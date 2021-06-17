use std::{
    fs,
    panic::{self, AssertUnwindSafe},
};

use failure::{Error, ResultExt};
use structopt::StructOpt;
use winit::{
    dpi::PhysicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

use mahboi::{
    SCREEN_WIDTH, SCREEN_HEIGHT, Emulator, Disruption,
    cartridge::Cartridge,
    log::*,
};
use crate::{
    args::Args,
    debug::{Action, TuiDebugger, WindowBuffer},
    env::Env,
    timer::LoopTimer,
};


mod args;
mod debug;
mod env;
mod timer;


const WINDOW_TITLE: &str = "Mahboi";

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

    // Initialize Debugger.
    let mut is_paused = args.debug && !args.instant_start;
    let mut debugger = {
        // Initialize global logger.
        debug::init_logger(&args);

        // Create the TUI debugger if we're in debug mode.
        if args.debug {
            Some(TuiDebugger::new(&args)?)
        } else {
            None
        }
    };

    // Load the ROM from disk and create the emulator.
    let mut emulator = {
        // Load ROM
        let rom = fs::read(&args.path_to_rom).context("failed to load ROM file")?;
        let cartridge = Cartridge::from_bytes(&rom);
        info!("[desktop] Loaded: {:#?}", cartridge);

        // Create emulator
        Emulator::new(cartridge, args.bios)
    };

    // Initialize the events loop, the window and the pixels buffer.
    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();
    let window = {
        let factor = args.scale as u32;
        let initial_size = PhysicalSize::new(
            SCREEN_WIDTH as u32 * factor,
            SCREEN_HEIGHT as u32* factor,
        );
        WindowBuilder::new()
            .with_title(WINDOW_TITLE)
            .with_inner_size(initial_size)
            .build(&event_loop)?
    };

    let mut env = Env::new(&args, &window)?;

    // ============================================================================================
    // ===== Main loop
    // ============================================================================================
    // Setup loop timing.
    let mut timer = LoopTimer::new(&args);

    // Start everything and run until the window is closed.
    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame.
        if let Event::RedrawRequested(_) = event {
            if let Err(e) = env.pixels.render() {
                eprintln!("pixels.render() failed: {}", e);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Application logic.
        if input.update(&event) {
            // Events to close the window.
            if input.quit() || (input.key_pressed(VirtualKeyCode::Q) && input.held_control()) {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Handle other non-Gameboy input events.
            timer.set_turbo_mode(input.key_held(VirtualKeyCode::Q));
            if let Some(size) = input.window_resized() {
                env.pixels.resize_surface(size.width, size.height);
            }

            // Run the emulator.
            if !is_paused {
                env.update_keys(&input);

                // Actually emulate!
                let outcome = timer.drive_emulation(|| {
                    emulate_frame(&mut emulator, &mut env, debugger.as_mut())
                });

                match outcome {
                    Outcome::Continue => {}
                    Outcome::Pause => is_paused = true,
                    Outcome::Terminate => {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                }
            }

            // If we're in debug mode (and have a TUI debugger), let's update it.
            if let Some(debugger) = &mut debugger {
                let action = debugger.update(
                    is_paused,
                    emulator.machine(),
                    WindowBuffer(env.pixels.get_frame()),
                );
                match action {
                    Action::Quit => {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    Action::Pause => is_paused = true,
                    Action::Continue => {
                        is_paused = false;
                        timer.unpause();
                    }
                    Action::Nothing => {}
                }
            }

            // Write FPS into window title
            if let Some(fps) = timer.report_fps() {
                window.set_title(&format!("{} - {:.1} FPS", WINDOW_TITLE, fps));
            }

            window.request_redraw();
        }
    });
}


#[derive(Debug, Clone, Copy, PartialEq)]
enum Outcome {
    Continue,
    Pause,
    Terminate,
}

// Emulates one frame of the emulator and correctly handles the debugger and the
// result of the emulation.
fn emulate_frame(
    emulator: &mut Emulator,
    env: &mut Env,
    mut debugger: Option<&mut TuiDebugger>,
) -> Outcome {
    let res = panic::catch_unwind(AssertUnwindSafe(|| {
        emulator.execute_frame(env, |machine| {
            // If we have a TUI debugger, we ask it when to pause.
            // Otherwise, we never stop.
            if let Some(debugger) = &mut debugger {
                debugger.should_pause(machine)
            } else {
                false
            }
        })
    }));

    match res {
        Err(e) => {
            if let Some(s) = e.downcast_ref::<&str>() {
                warn!("Emulator panicked: {}", s);
            } else {
                warn!("Emulator panicked!");
            };

            if debugger.is_none() {
                panic::resume_unwind(e);
            }

            Outcome::Pause
        }
        Ok(disruption) => {
            // React to abnormal disruptions
            match disruption {
                Ok(_) => Outcome::Continue,
                Err(Disruption::Paused) => Outcome::Pause,
                Err(Disruption::Terminated) => {
                    // If we are not in debug mode, we stop the program, as it
                    // doesn't make much sense to keep running. In debug mode,
                    // we just pause execution.
                    warn!("[desktop] Emulator was terminated");
                    if debugger.is_some() {
                        Outcome::Pause
                    } else {
                        Outcome::Terminate
                    }
                }
            }
        }
    }
}
