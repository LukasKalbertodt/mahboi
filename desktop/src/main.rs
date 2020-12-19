use std::{fs, panic::{self, AssertUnwindSafe}, time::Duration};

use failure::{Error, ResultExt};
use pixels::{Pixels, SurfaceTexture};
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
    env::Peripherals,
    primitives::PixelColor,
    machine::input::{Keys, JoypadKey},
    log::*,
};
use crate::{
    debug::{Action, TuiDebugger},
    args::Args,
};


mod args;
mod debug;


const TARGET_FPS: f64 = 59.73;
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

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(SCREEN_WIDTH as u32, SCREEN_HEIGHT as u32, surface_texture)?
    };

    // Setup loop timing.
    let mut emulation_period = Duration::from_millis(0);
    let mut loop_helper = spin_sleep::LoopHelper::builder()
        .report_interval_s(0.2)
        .build_with_target_rate(TARGET_FPS);

    // Start everything and run until the window is closed.
    event_loop.run(move |event, _, control_flow| {
        // Draw the current frame.
        if let Event::RedrawRequested(_) = event {
            if let Err(e) = pixels.render() {
                eprintln!("pixels.render() failed: {}", e);
                *control_flow = ControlFlow::Exit;
                return;
            }
        }

        // Application logic.
        if input.update(&event) {
            emulation_period += loop_helper.loop_start();

            // Events to close the window.
            if input.quit() || (input.key_pressed(VirtualKeyCode::Q) && input.held_control()) {
                *control_flow = ControlFlow::Exit;
                return;
            }

            // Resize the window.
            if let Some(size) = input.window_resized() {
                pixels.resize(size.width, size.height);
            }

            // Run the emulator.
            if !is_paused {
                let keys = Keys::none()
                    .set_key(JoypadKey::Up, input.key_held(VirtualKeyCode::W))
                    .set_key(JoypadKey::Left, input.key_held(VirtualKeyCode::A))
                    .set_key(JoypadKey::Down, input.key_held(VirtualKeyCode::S))
                    .set_key(JoypadKey::Right, input.key_held(VirtualKeyCode::D))
                    .set_key(JoypadKey::A, input.key_held(VirtualKeyCode::J))
                    .set_key(JoypadKey::B, input.key_held(VirtualKeyCode::K))
                    .set_key(JoypadKey::Select, input.key_held(VirtualKeyCode::N))
                    .set_key(JoypadKey::Start, input.key_held(VirtualKeyCode::M));
                let mut env = Env { keys, buffer: pixels.get_frame() };

                let res = panic::catch_unwind(AssertUnwindSafe(|| {
                    emulator.execute_frame(&mut env, |machine| {
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
                                    *control_flow = ControlFlow::Exit;
                                    return;
                                }
                            }
                        }
                    }
                }
            }

            // If we're in debug mode (and have a TUI debugger), let's update it.
            if let Some(debugger) = &mut debugger {
                let action = debugger.update(is_paused, emulator.machine());
                match action {
                    Action::Quit => {
                        *control_flow = ControlFlow::Exit;
                        return;
                    }
                    Action::Pause => is_paused = true,
                    Action::Continue => is_paused = false,
                    Action::Nothing => {}
                }
            }

            // Write FPS into window title
            if let Some(fps) = loop_helper.report_rate() {
                window.set_title(&format!("{} - {:.1} FPS", WINDOW_TITLE, fps));
            }

            window.request_redraw();
        }
    });
}

/// The environment of the Gameboy. Implements `Peripherals`.
struct Env<'a> {
    keys: Keys,
    buffer: &'a mut [u8],
}

impl Peripherals for Env<'_> {
    fn get_pressed_keys(&self) -> Keys {
        self.keys
    }

    fn write_lcd_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]) {
        let offset = line_idx as usize * SCREEN_WIDTH * 4;
        for col in 0..SCREEN_WIDTH {
            let [r, g, b] = pixels[col].to_srgb();

            self.buffer[offset + 4 * col + 0] = r;
            self.buffer[offset + 4 * col + 1] = g;
            self.buffer[offset + 4 * col + 2] = b;
        }
    }
}
