#![feature(const_fn)]
#![feature(const_vec_new)]

use std::fs;

use failure::{Error, ResultExt};
use minifb::{Key, WindowOptions, Window};
use structopt::StructOpt;

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT, Emulator, Disruption,
    cartridge::Cartridge,
    log::*,
};
use crate::{
    debug::{Action, TuiDebugger},
    env::Peripherals,
    args::Args,
};


mod args;
mod debug;
mod env;


fn main() {
    // We just catch potential errors here and pretty print them.
    if let Err(e) = run() {
        println!("ERROR: {}", e);

        for cause in e.iter_causes() {
            println!("  ... caused by: {}", cause);
        }
    }
}

/// The actual main function.
fn run() -> Result<(), Error> {
    // Parse CLI arguments
    let args = Args::from_args();

    // Initialize global logger. The logger kind depends on the `--debug` flag.
    debug::init_logger(args.debug);
    log::set_max_level(log::LevelFilter::Trace);

    // Create the TUI debugger if we're in debug mode.
    let mut tui_debugger = if args.debug {
        Some(TuiDebugger::new()?)
    } else {
        None
    };

    // Load ROM
    let rom = fs::read(&args.path_to_rom)?;

    // Create emulator
    let cartridge = Cartridge::from_bytes(&rom);
    info!("Loaded: {:#?}", cartridge);
    let mut peripherals = Peripherals {};

    let mut emulator = Emulator::new(cartridge, &mut peripherals);

    let mut window = open_window(&args).context("failed to open window")?;
    info!("Opened window");

    let mut buffer: Vec<u32> = vec![0; SCREEN_WIDTH * SCREEN_HEIGHT];
    let mut color = 0;
    let mut is_paused = args.debug;
    while window.is_open() && !window.is_key_down(Key::Escape) {
        for i in buffer.iter_mut() {
            *i = color;
        }
        color += 1;

        window.update_with_buffer(&buffer).unwrap();

        // Run the emulator.
        if !is_paused {
            let res = emulator.execute_frame();
            match res {
                Ok(_) => {},
                Err(Disruption::Paused) => is_paused = true,
                Err(Disruption::Terminated) => {
                    // If we are not in debug mode, we stop the program, as it
                    // doesn't make much sense to keep running. In debug mode,
                    // we just pause execution.
                    warn!("Emulator was terminated");
                    if args.debug {
                        is_paused = true;
                    } else {
                        break;
                    }
                }
            }
        }

        // If we're in debug mode (and have a TUI debugger), let's update it.
        if let Some(debugger) = &mut tui_debugger {
            let action = debugger.update(is_paused)?;
            match action {
                Action::Quit => break,
                Action::Pause => is_paused = true,
                Action::Continue => is_paused = false,
                Action::Nothing => {}
            }
        }
    }

    Ok(())
}

/// Opens a `minifb` window configured by `args`.
fn open_window(args: &Args) -> Result<Window, Error> {
    const TITLE: &str = "Mahboi";

    let options = WindowOptions {
        borderless: false,
        title: true,
        resize: false,
        scale: args.scale,
    };

    Window::new(TITLE, SCREEN_WIDTH, SCREEN_HEIGHT, options)
        .map_err(|e| e.into())
}
