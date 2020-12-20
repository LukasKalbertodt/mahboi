use std::{
    fs,
    panic::{self, AssertUnwindSafe},
    sync::{Arc, Mutex},
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
    args::Args,
    debug::{Action, TuiDebugger, WindowBuffer},
    timer::LoopTimer,
};


mod args;
mod debug;
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
