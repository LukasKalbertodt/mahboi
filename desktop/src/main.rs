use std::{
    fs,
    mem,
    panic::{self, AssertUnwindSafe},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use cpal::traits::{DeviceTrait, HostTrait};
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
    debug::{Action, TuiDebugger, WindowBuffer},
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

    // Initialize sound stream
    let device = cpal::default_host()
        .default_output_device()
        .ok_or(failure::format_err!("failed to find a default output device"))?;
    let supported_config = device.default_output_config()
        .context("no default sound output config")?;
    let audio_buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let mut cycles_till_next_sample = 0.0;
    let stream = {
        let audio_buffer = audio_buffer.clone();
        // let config: cpal::StreamConfig = supported_config.clone().into();
        println!("{:?}", supported_config.sample_rate());
        let config = cpal::StreamConfig {
            channels: 2, // TODO
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Fixed(735), // TODO
        };
        let channels = config.channels;
        let mut collected_enough = false;
        device.build_output_stream_raw(
            &config,
            supported_config.sample_format(),
            move |data: &mut cpal::Data, _: &cpal::OutputCallbackInfo| {

                let mut buffer = audio_buffer.lock().unwrap();
                if buffer.len() > 4000 {
                    collected_enough = true;
                } else if buffer.len() < 1500 {
                    collected_enough = false;
                }

                // for v in &buffer {
                //     println!("{}", v);
                // }
                // println!("----------------------------");

                match data.sample_format() {
                    cpal::SampleFormat::I16 => todo!(),
                    cpal::SampleFormat::U16 => todo!(),
                    cpal::SampleFormat::F32 => {
                        let out = data.as_slice_mut::<f32>().unwrap();
                        println!("out {} <-> src {}", out.len() / 2, buffer.len());

                        if !collected_enough {
                            for out in out {
                                *out = 0.0;
                            }
                            return;
                        }

                        let num_samples = std::cmp::min(out.len() / 2, buffer.len());
                        for (dst, src) in out.chunks_mut(channels as usize).zip(buffer.drain(..num_samples)) {
                            // println!("{}", src);
                            for channel in dst {
                                *channel = src;
                            }
                        }
                        // println!("-------------------");

                        if buffer.len() < out.len() / channels as usize {
                            println!("!!! Provided audio data shorter than the cpal buffer");

                            for out in &mut out[buffer.len() * channels as usize..] {
                                *out = 0.0;
                            }
                        }
                    }
                }
            },
            |e| eprintln!("audio error: {}", e),
        )
    };


    // ============================================================================================
    // ===== Main loop
    // ============================================================================================
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

            // Handle other non-Gameboy input events.
            timer.set_turbo_mode(input.key_held(VirtualKeyCode::Q));
            if let Some(size) = input.window_resized() {
                pixels.resize(size.width, size.height);
            }

            // Run the emulator.
            if !is_paused {
                let mut env = Env::new(&input, pixels.get_frame(), &audio_buffer, &mut cycles_till_next_sample);

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
                    WindowBuffer(pixels.get_frame()),
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

/// The environment of the Gameboy. Implements `Peripherals`.
struct Env<'a> {
    keys: Keys,
    buffer: &'a mut [u8],
    audio_buffer: &'a Mutex<Vec<f32>>,
    cycles_till_next_sample: &'a mut f32,
}

impl<'a> Env<'a> {
    fn new(input: &WinitInputHelper, buffer: &'a mut [u8], audio_buffer: &'a Mutex<Vec<f32>>, cycles_till_next_sample: &'a mut f32) -> Self {
        let keys = Keys::none()
            .set_key(JoypadKey::Up, input.key_held(VirtualKeyCode::W))
            .set_key(JoypadKey::Left, input.key_held(VirtualKeyCode::A))
            .set_key(JoypadKey::Down, input.key_held(VirtualKeyCode::S))
            .set_key(JoypadKey::Right, input.key_held(VirtualKeyCode::D))
            .set_key(JoypadKey::A, input.key_held(VirtualKeyCode::J))
            .set_key(JoypadKey::B, input.key_held(VirtualKeyCode::K))
            .set_key(JoypadKey::Select, input.key_held(VirtualKeyCode::N))
            .set_key(JoypadKey::Start, input.key_held(VirtualKeyCode::M));

        Self { keys, buffer, audio_buffer, cycles_till_next_sample }
    }
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

    fn offer_sound_sample(&mut self, f: impl FnOnce() -> f32) {
        if *self.cycles_till_next_sample <= 0.0 {
            self.audio_buffer.lock().unwrap().push(f());
            *self.cycles_till_next_sample += 23.777; // TODO
        }
        *self.cycles_till_next_sample -= 1.0;
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

    // Tells the timer the emulation has just been unpaused. This will reset a
    // few values in the timer so that the timer doesn't think we just
    // experienced a huge lag.
    fn unpause(&mut self) {
        self.behind = self.ideal_frame_time.mul_f32(1.5);
        self.last_host_frame = None;
    }

    /// Call once per host frame and pass a closure that emulates one frame of
    /// the gameboy. This method will make sure that `emulate_frame` is called
    /// an appropriate number of times to keep the target frame rate.
    fn drive_emulation(&mut self, mut emulate_frame: impl FnMut() -> Outcome) -> Outcome {
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
            let outcome = emulate_frame();
            if outcome != Outcome::Continue {
                return outcome;
            }

            slack = SLACK_MULTIPLIER;
            self.frames_since_last_report += 1;
        }

        Outcome::Continue
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
