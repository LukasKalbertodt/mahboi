#![feature(const_fn)]
#![feature(const_vec_new)]

use std::fs;

use failure::{Error, ResultExt};
use structopt::StructOpt;

use mahboi::{
    Emulator, Disruption,
    cartridge::Cartridge,
    log::*,
};
use crate::{
    debug::{Action, TuiDebugger},
    env::NativeWindow,
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
        let debugger = TuiDebugger::new()?;
        for &bp in &args.breakpoints {
            debugger.breakpoints().add(bp);
        }

        Some(debugger)
    } else {
        None
    };

    // Load ROM
    let rom = fs::read(&args.path_to_rom)?;
    let cartridge = Cartridge::from_bytes(&rom);
    info!("Loaded: {:#?}", cartridge);

    // Create emulator
    let mut emulator = Emulator::new(cartridge);

    // Open window
    let mut window = NativeWindow::open(&args).context("failed to open window")?;
    info!("Opened window");

    let mut is_paused = args.debug;
    while !window.should_stop() {
        // Update window buffer and read input.
        window.update()?;

        // Run the emulator.
        if !is_paused {
            let res = emulator.execute_frame(&mut window, |machine| {
                // If we have a TUI debugger, we ask it when to pause.
                // Otherwise, we never stop.
                if let Some(debugger) = &mut tui_debugger {
                    debugger.should_pause(machine)
                } else {
                    false
                }
            });

            // React to abnormal disruptions
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
            let action = debugger.update(is_paused, emulator.machine())?;
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
