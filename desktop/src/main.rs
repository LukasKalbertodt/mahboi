#![feature(rust_2018_preview)]
#![feature(const_fn)]
#![feature(const_vec_new)]

use std::fs;

use failure::{Error, ResultExt};
use minifb::{Key, WindowOptions, Window};
use structopt::StructOpt;

use mahboi::{
    SCREEN_WIDTH,
    SCREEN_HEIGHT,
    Emulator,
    cartridge::Cartridge,
    log::*,
};
use crate::{
    debug::{Action, tui::TuiDebugger},
    env::Peripherals,
    args::Args,
};


mod args;
mod debug;
mod env;


fn main() {
    if let Err(e) = run() {
        println!("ERROR: {}", e);

        for cause in e.iter_causes() {
            println!("  ... caused by: {}", cause);
        }
    }
}

fn run() -> Result<(), Error> {
    // Parse CLI arguments
    let args = Args::from_args();

    // Initialize global logger
    debug::init_logger(args.debug);
    log::set_max_level(log::LevelFilter::Trace);

    trace!("A super unimportant message");
    debug!("An unimportant message");
    info!("Here, have some information");
    warn!("You should probably fix that");
    error!("OMG EVERYTHING IS ON FIRE");

    // Create debugger
    let mut debugger = TuiDebugger::new()?;


    // Load ROM
    let rom = fs::read(&args.path_to_rom)?;

    // Create emulator
    let cartridge = Cartridge::from_bytes(&rom);
    debug!("Loaded: {:#?}", cartridge);
    let mut peripherals = Peripherals {};

    let mut emulator = Emulator::new(cartridge, &mut peripherals);

    let mut window = open_window(&args).context("failed to open window")?;
    info!("opened window");

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
            emulator.execute_frame();
        }

        let action = debugger.update(is_paused)?;
        match action {
            Action::Quit => break,
            Action::Pause => is_paused = true,
            Action::Continue => is_paused = false,
            Action::Nothing => {}
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
