use std::{fs, panic::{self, AssertUnwindSafe}, time::{Duration, Instant}};

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
    let mut timer = LoopTimer::new(&args);

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
            // Events to close the window.
            if input.quit() || (input.key_pressed(VirtualKeyCode::Q) && input.held_control()) {
                *control_flow = ControlFlow::Exit;
                return;
            }

            timer.set_turbo_mode(input.key_held(VirtualKeyCode::Q));

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

                timer.drive_emulation(|| {
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
                });
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
            if let Some(fps) = timer.report_fps() {
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

/// How often the FPS are reported. Longer times lead to more delay and more
/// smoothing.
const REPORT_INTERVAL: Duration = Duration::from_millis(250);

/// Check `drive_emulation` for more details.
const SLACK_MULTIPLIER: f32 = 1.3;

struct LoopTimer {
    /// The time an emulated frame should last. (This stays constant.)
    ideal_frame_time: Duration,

    /// The factor by which the `ideal_frame_time` is divided when the turbo
    /// mode is enabled. (This stays constant.)
    turbo_mode_factor: f64,

    /// The amount the emulation is behind of the ideal time.
    behind: Duration,

    /// The point in time when `should_emulate_frame` was last called. That
    /// method should be called once every frame on the host machine.
    last_host_frame: Option<Instant>,

    /// Whether the turbo mode is enabled.
    turbo: bool,

    // For FPS reporting
    last_report: Instant,
    frames_since_last_report: u32,
    behind_at_last_report: Duration,
}

impl LoopTimer {
    fn new(args: &Args) -> Self {
        let ideal_frame_time = Duration::from_secs(1).div_f64(args.fps);

        // This arbitrary 1.5 factor makes sure that in a typical 59.73
        // emulation FPS and 60 host FPS setting, the first "host frame no
        // emulation frame" doesn't happen at the very start. But it also makes
        // sure that there aren't two emulation frames in one host frame (which
        // would be factor 2).
        let behind = ideal_frame_time.mul_f32(1.5);

        Self {
            ideal_frame_time,
            turbo_mode_factor: args.turbo_mode_factor,
            turbo: false,
            last_host_frame: None,
            behind,
            last_report: Instant::now(),
            frames_since_last_report: 0,
            behind_at_last_report: behind,
        }
    }

    fn set_turbo_mode(&mut self, turbo: bool) {
        self.turbo = turbo;
    }

    /// Call once per host frame and pass a closure that emulates one frame of
    /// the gameboy. This method will make sure that `emulate_frame` is called
    /// an appropriate number of times to keep the target frame rate.
    fn drive_emulation(&mut self, mut emulate_frame: impl FnMut()) {
        let now = Instant::now();
        if let Some(last_host_frame) = self.last_host_frame {
            self.behind += now - last_host_frame;
            // println!("{:.1?} -> {:.1?}", now - last_host_frame, self.behind);
        }
        self.last_host_frame = Some(now);

        // Obtain actual target frame time by applying turbo mode.
        let target_frame_time = self.target_frame_time();

        // If one emulation frame fits into the `behind` time, we emulate a
        // frame. After the first iteration we set `slack` to 1.3. This is done
        // to have some transition time/slack. Otherwise the following
        // problematic situation might arise quite often:
        // - The `behind` value gets smaller than `target_frame_time` (which it
        //   does regularly when using the original gameboy speed on a 60hz
        //   monitor).
        // - One host frame, there is no frame emulated to make up for that.
        // - The next host frame takes a bit longer, so that the previous
        //   `behind` value plus this slightly longer host frame result in a
        //   value slightly larger than two times `target_frame_time`.
        // - That results in two frames being emulated. But the next host frame
        //   is likely a bit shorter again, meaning that THEN no gameboy frame
        //   is emulated again.
        //
        // This can destabilize the game loop and lead to some juttery motion.
        let mut slack = 1.0;
        while self.behind > target_frame_time.mul_f32(slack) {
            self.behind -= target_frame_time;
            emulate_frame();
            slack = SLACK_MULTIPLIER;
            self.frames_since_last_report += 1;
        }
    }

    fn target_frame_time(&self) -> Duration {
        if self.turbo {
            self.ideal_frame_time.div_f64(self.turbo_mode_factor)
        } else {
            self.ideal_frame_time
        }
    }

    /// Returns `Some(fps)` every `REPORT_INTERVAL`.
    fn report_fps(&mut self) -> Option<f64> {
        let elapsed = self.last_report.elapsed();
        if elapsed >= REPORT_INTERVAL {
            // The calculation is a bit more involved to avoid the reported FPS
            // fluctuating all over the place. That's because in the case of
            // original gameboy speed and 60hz monitor, every roughly 2s, a
            // gameboy frame is skipped. With a naive calculation of
            // `frames_since_last_report / relapsed`, the reported FPS would be
            // 60 most of the time and dip down to like 55 or so for a single
            // report. While this is "more correct", technically, I think it's
            // more useful for users to have the saved time in `behind` be
            // included in the report.
            //
            // So we check the difference between `behind` and the `behind`
            // value when the last report was made. That way we know whether we
            // "spent" or gained saved time compared to the last report.
            let saved_time = self.behind.as_secs_f64() - self.behind_at_last_report.as_secs_f64();
            let saved_frames = saved_time / self.target_frame_time().as_secs_f64();
            let fps = (self.frames_since_last_report as f64 + saved_frames)
                / elapsed.as_secs_f64();

            // Reset stuff
            self.behind_at_last_report = self.behind;
            self.last_report = Instant::now();
            self.frames_since_last_report = 0;

            Some(fps)
        } else {
            None
        }
    }
}
