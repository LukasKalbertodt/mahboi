#![feature(const_fn)]
#![feature(const_vec_new)]

use std::{
    fs,
    panic::{self, AssertUnwindSafe},
};

use failure::{Error, ResultExt};
use structopt::StructOpt;

use mahboi::{
    Emulator, Disruption,
    cartridge::Cartridge,
    log::*,
    primitives::TARGET_FPS,
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

        std::process::exit(1);
    }
}

/// The actual main function.
fn run() -> Result<(), Error> {
    // Parse CLI arguments
    let args = Args::from_args();

    // Prepare everything
    let mut tui_debugger = init_debugger(&args)?;
    let mut emulator = init_emulator(&args)?;
    let mut window = NativeWindow::open(&args).context("failed to open window")?;


    // ===== MAIN LOOP ========================================================
    let mut is_paused = args.debug && !args.instant_start;
    let mut loop_helper = spin_sleep::LoopHelper::builder()
        .report_interval_s(0.2)
        .build_with_target_rate(TARGET_FPS);

    while !window.should_stop() {
        loop_helper.loop_start();

        // Update window buffer and read input.
        window.update()?;

        // Run the emulator if we're not in pause mode.
        if !is_paused {
            let res = panic::catch_unwind(AssertUnwindSafe(|| {
                emulator.execute_frame(&mut window, |machine| {
                    // If we have a TUI debugger, we ask it when to pause.
                    // Otherwise, we never stop.
                    if let Some(debugger) = &mut tui_debugger {
                        debugger.should_pause(machine)
                    } else {
                        false
                    }
                })}
            ));

            // React to abnormal disruptions
            match res {
                Err(e) => {
                    if let Some(s) = e.downcast_ref::<&str>() {
                        warn!("Emulator panicked: {}", s);
                    } else {
                        warn!("Emulator panicked!");
                    };

                    if !args.debug {
                        panic::resume_unwind(e);
                    }

                    is_paused = true;
                }
                Ok(disruption) => {
                    // React to abnormal disruptions
                    match disruption {
                        Ok(_) => {},
                        Err(Disruption::Paused) => is_paused = true,
                        Err(Disruption::Terminated) => {
                            // If we are not in debug mode, we stop the program, as it
                            // doesn't make much sense to keep running. In debug mode,
                            // we just pause execution.
                            warn!("[desktop] Emulator was terminated");
                            if args.debug {
                                is_paused = true;
                            } else {
                                break;
                            }
                        }
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

        // Write FPS into window title
        if let Some(fps) = loop_helper.report_rate() {
            window.set_title_postfix(&format!("{:.1} FPS", fps));
        }

        // Sleep for a while to reach our target FPS
        if window.in_turbo_mode() {
            loop_helper.set_target_rate(TARGET_FPS * args.turbo_mode_factor);
        } else {
            loop_helper.set_target_rate(TARGET_FPS);
        }
        loop_helper.loop_sleep();
    }

    Ok(())
}

/// Initializes the global logger implementation and returns the TUI debugger,
/// if we are in debugging mode.
fn init_debugger(args: &Args) -> Result<Option<TuiDebugger>, Error> {
    // Initialize global logger.
    debug::init_logger(args);

    // Create the TUI debugger if we're in debug mode.
    if args.debug {
        Ok(Some(TuiDebugger::new(&args)?))
    } else {
        Ok(None)
    }
}

/// Loads the ROM and initializes the emulator.
fn init_emulator(args: &Args) -> Result<Emulator, Error> {
    // Load ROM
    let rom = fs::read(&args.path_to_rom).context("failed to load ROM file")?;
    let cartridge = Cartridge::from_bytes(&rom);
    info!("[desktop] Loaded: {:#?}", cartridge);

    // Create emulator
    Ok(Emulator::new(cartridge))
}
