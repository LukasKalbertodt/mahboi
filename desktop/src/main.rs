use std::{
    fs,
    mem,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{channel, Sender},
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    thread,
};

use failure::{Error, ResultExt};
use glium::{
    Display, Program, VertexBuffer, Surface,
    implement_vertex, uniform,
    glutin::{
        ContextBuilder, EventsLoop, WindowBuilder, WindowedContext, NotCurrent,
        dpi::PhysicalSize,
    },
    index::NoIndices,
    program::ProgramCreationInput,
    texture::{
        UnsignedTexture2d, UncompressedUintFormat, MipmapsOption,
        pixel_buffer::PixelBuffer,
    },
};
use spin_sleep::LoopHelper;
use structopt::StructOpt;

use mahboi::{
    Emulator, Disruption, SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
    env::Peripherals,
    cartridge::Cartridge,
    primitives::PixelColor,
    machine::input::{Keys, JoypadKey},
};
use crate::{
    args::Args,
};


mod args;
// mod debug;


const TARGET_FPS: f64 = 59.7275;
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

    // Initialize logger
    let mut builder = env_logger::Builder::from_default_env();
    builder.filter_module("mahboi", args.log_level);
    builder.init();



    // =======================================================================
    // ===== Initialization ==================================================
    // =======================================================================

    // ----- Core Emulator ---------------------------------------------------
    let emulator = {
        let rom = fs::read(&args.path_to_rom).context("failed to load ROM file")?;
        let cartridge = Cartridge::from_bytes(&rom);
        info!("Loaded: {:#?}", cartridge);

        // Create emulator
        Emulator::new(cartridge, args.bios)
    };


    // ----- Input Thread ----------------------------------------------------
    // These three instances are shared across all threads.
    let (messages, incoming_messages) = channel();
    let shared = Shared {
        messages,
        state: Arc::new(SharedState {
            args,
            keys: AtomicKeys::none(),
            gb_screen: GbScreenBuffer::new(),
            emulation_rate: Mutex::new(TARGET_FPS),
            turbo_mode: AtomicBool::new(false),
        }),
    };

    // Here we start the input thread. It's a bit awkward because the input
    // thread needs the `EventsLoop`, but this type cannot be sent to new
    // thread. So we have to already create it in the correct thread.
    // Furthermore, it is needed to create the `glium` display later, which
    // also cannot be transferred across threads. Luckily, we can build a
    // glutin context already and transfer it across threads.
    let context = {
        // Create a new handle to the shared values.
        let shared = shared.clone();

        // Even more awkward: to send the glutin context back to the main
        // thread, we cannot just return it, because the input thread runs
        // forever. Thus, we need a one-time channel here.
        let (tx, rx) = channel();

        thread::spawn(move || {
            // Create the main events loop, a window and a context.
            let events_loop = EventsLoop::new();

            // Configure window
            //
            // TODO: this might be wrong when the window is not created on the
            // primary monitor. No idea if that can happen.
            let dpi_factor = events_loop.get_primary_monitor().get_hidpi_factor();
            let size = PhysicalSize::new(
                SCREEN_WIDTH as f64 * shared.state.args.scale,
                SCREEN_HEIGHT as f64 * shared.state.args.scale,
            );
            let wb = WindowBuilder::new()
                .with_dimensions(size.to_logical(dpi_factor))
                .with_resizable(true)
                .with_title(WINDOW_TITLE);

            // Configure and GL context
            let cb = ContextBuilder::new()
                .with_vsync(true);
            let context = cb.build_windowed(wb, &events_loop);
            info!("[desktop] Opened window");

            // The context is not needed in the input thread anymore, but it's
            // needed in the main thread. So send it back.
            tx.send(context).unwrap();

            // Start polling input events (forever)
            input_thread(events_loop, shared);
        });

        // Receive the context from the thread.
        rx.recv().unwrap()?
    };


    // ----- Render Thread ---------------------------------------------------
    {
        // Create a new handle to the shared values.
        let shared = shared.clone();

        thread::spawn(move || {
            // There could actually go something wrong in the render thread. If
            // that's the case, we send an action to the main thread.
            let result = render_thread(context, shared.clone());
            if let Err(e) = result {
                shared.messages.send(Message::RenderError(e)).unwrap();
            }
        });
    }

    // ----- Emulator Thread -------------------------------------------------
    thread::spawn(move || emulator_thread(emulator, shared));



    // =======================================================================
    // ===== Let everything run ==============================================
    // =======================================================================

    // All the real work is done in threads. We just listen to messages that
    // come from the thread. The main thread will just wait almost all of the
    // time.
    for msg in incoming_messages {
        match msg {
            Message::Quit => break,
            Message::RenderError(e) => return Err(e.context("error in render thread"))?,
        }
    }

    Ok(())
}


// ===============================================================================================
// ===== The worker threads ======================================================================
// ===============================================================================================

/// Listens for input events and handles them by either updating `keys` or
/// sending messages to the main thread.
fn input_thread(
    mut events_loop: EventsLoop,
    shared: Shared,
) {
    use glium::glutin::{
        ControlFlow, ElementState as State, Event, KeyboardInput,
        VirtualKeyCode as Key, WindowEvent,
    };

    events_loop.run_forever(move |event| {
        // Mini helper function
        let send_action = |action| {
            shared.messages.send(action)
                .expect("failed to send input action: input thread will panic now");
        };


        // First, we extract the inner window event as that's what we are
        // interested in.
        let event = match event {
            // That's what we want!
            Event::WindowEvent { event, .. } => event,

            // When the main thread wakes us up, we just stop this thread.
            Event::Awakened => return ControlFlow::Break,

            // We ignore all other events (device events).
            _ => return ControlFlow::Continue,
        };

        // Now handle window events.
        match event {
            WindowEvent::CloseRequested | WindowEvent::Destroyed => send_action(Message::Quit),

            // A key input that has a virtual keycode attached
            WindowEvent::KeyboardInput {
                input: KeyboardInput { virtual_keycode: Some(key), state, modifiers, .. },
                ..
            } => {
                let keys = &shared.state.keys;

                match key {
                    // Button keys
                    Key::M if state == State::Pressed => keys.set_key(JoypadKey::Start),
                    Key::M if state == State::Released => keys.unset_key(JoypadKey::Start),
                    Key::N if state == State::Pressed => keys.set_key(JoypadKey::Select),
                    Key::N if state == State::Released => keys.unset_key(JoypadKey::Select),
                    Key::J if state == State::Pressed => keys.set_key(JoypadKey::A),
                    Key::J if state == State::Released => keys.unset_key(JoypadKey::A),
                    Key::K if state == State::Pressed => keys.set_key(JoypadKey::B),
                    Key::K if state == State::Released => keys.unset_key(JoypadKey::B),

                    // Direction keys
                    Key::W if state == State::Pressed => keys.set_key(JoypadKey::Up),
                    Key::W if state == State::Released => keys.unset_key(JoypadKey::Up),
                    Key::A if state == State::Pressed => keys.set_key(JoypadKey::Left),
                    Key::A if state == State::Released => keys.unset_key(JoypadKey::Left),
                    Key::S if state == State::Pressed => keys.set_key(JoypadKey::Down),
                    Key::S if state == State::Released => keys.unset_key(JoypadKey::Down),
                    Key::D if state == State::Pressed => keys.set_key(JoypadKey::Right),
                    Key::D if state == State::Released => keys.unset_key(JoypadKey::Right),

                    // Other non-Gameboy related functions
                    Key::Q if state == State::Pressed && modifiers.ctrl
                        => send_action(Message::Quit),

                    Key::LShift => {
                        shared.state.turbo_mode.store(state == State::Pressed, Ordering::SeqCst);
                    }

                    _ => {}
                }
            }
            _ => {}
        }

        ControlFlow::Continue
    });

    debug!("Input thread shutting down");
}

/// Renders the front buffer of `gb_buffer` to the host screen at the host
/// refresh rate.
fn render_thread(
    context: WindowedContext<NotCurrent>,
    shared: Shared,
) -> Result<(), Error> {
    let display = Display::from_gl_window(context)?;

    // Create the pixel buffer and initialize all pixels with black.
    let mut pixel_buffer = PixelBuffer::new_empty(&display, SCREEN_WIDTH * SCREEN_HEIGHT);
    pixel_buffer.write(&vec![(0, 0, 0); SCREEN_WIDTH * SCREEN_HEIGHT]);

    // Create an empty, uninitialized texture
    let texture = UnsignedTexture2d::empty_with_format(
        &display,
        UncompressedUintFormat::U8U8U8,
        MipmapsOption::NoMipmap,
        SCREEN_WIDTH as u32,
        SCREEN_HEIGHT as u32,
    )?;


    #[derive(Copy, Clone)]
    struct Vertex {
        position: [f32; 2],
        tex_coords: [f32; 2],
    }

    implement_vertex!(Vertex, position, tex_coords);

    // Create the full screen quad
    let shape = vec![
        Vertex { position: [-1.0, -1.0], tex_coords: [0.0, 1.0] },
        Vertex { position: [-1.0,  1.0], tex_coords: [0.0, 0.0] },
        Vertex { position: [ 1.0, -1.0], tex_coords: [1.0, 1.0] },
        Vertex { position: [ 1.0,  1.0], tex_coords: [1.0, 0.0] },
    ];

    let vertex_buffer = VertexBuffer::new(&display, &shape)?;
    let indices = NoIndices(glium::index::PrimitiveType::TriangleStrip);


    // Compile program. We have to do it via `ProgramCreationInput` to set
    // `outputs_srgb` to `true`. This is an ugly workaround for a bug
    // somewhere in the window creation stack. The framebuffer is
    // incorrectly created as sRGB and glium then automatically converts
    // all values returned by the fragment shader into sRGB. We don't want
    // a conversion, so we just tell glium we already output sRGB (which we
    // don't).
    let program = Program::new(
        &display,
        ProgramCreationInput::SourceCode {
            vertex_shader: include_str!("shader/simple.vert"),
            tessellation_control_shader: None,
            tessellation_evaluation_shader: None,
            geometry_shader: None,
            fragment_shader: include_str!("shader/simple.frag"),
            transform_feedback_varyings: None,
            outputs_srgb: true,
            uses_point_size: false,
        }
    )?;

    let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.25)
        .build_without_target_rate();

    loop {
        loop_helper.loop_start();

        // We map the pixel buffer and write directly to it.
        {
            let mut pixel_buffer = pixel_buffer.map_write();
            let front = shared.state.gb_screen.front.lock()
                .expect("failed to lock front buffer");
            for (i, &PixelColor { r, g, b }) in front.iter().enumerate() {
                pixel_buffer.set(i, (r, g, b));
            }
        }

        // We update the texture data by uploading our pixel buffer.
        texture.main_level().raw_upload_from_pixel_buffer(
            pixel_buffer.as_slice(),
            0..SCREEN_WIDTH as u32,
            0..SCREEN_HEIGHT as u32,
            0..1,
        );

        // Draw the fullscreenquad to the framebuffer
        let mut target = display.draw();
        target.draw(
            &vertex_buffer,
            &indices,
            &program,
            &uniform! { tex: &texture },
            &Default::default(),
        )?;
        target.finish()?;

        // We try really hard to make OpenGL sync here. We want to avoid OpenGL
        // rendering several frames in advance as this drives up the input lag.
        let fence = glium::SyncFence::new(&display).unwrap();
        fence.wait();
        display.finish();

        // Potentially update the window title to show the current speed.
        if let Some(ogl_fps) = loop_helper.report_rate() {
            let emu_fps = *shared.state.emulation_rate.lock().unwrap();
            let emu_percent = (emu_fps / TARGET_FPS) * 100.0;
            let title = format!(
                "{} (emulator: {:.1} FPS / {:3}%, openGL: {:.1} FPS)",
                WINDOW_TITLE,
                emu_fps,
                emu_percent.round(),
                ogl_fps,
            );
            display.gl_window().window().set_title(&title);
        }
    }
}

/// Drives the emulation. The emulator writes into the `gb_buffer` back buffer.
/// Both of those buffers are swapped after each Gameboy frame. The emulator
/// additionally reads from `keys`. Lastly, if the emulator terminates in an
/// unusual fashion, a `Quit` message is send to the main thread.
fn emulator_thread(
    mut emulator: Emulator,
    shared: Shared,
) {
    /// This is what we pass to the emulator.
    struct DesktopPeripherals<'a> {
        back_buffer: MutexGuard<'a, Vec<PixelColor>>,
        keys: &'a AtomicKeys,
    }

    impl Peripherals for DesktopPeripherals<'_> {
        fn get_pressed_keys(&self) -> Keys {
            self.keys.as_keys()
        }

        fn write_lcd_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]) {
            let start = line_idx as usize * SCREEN_WIDTH;
            let end = start + SCREEN_WIDTH;
            self.back_buffer[start..end].copy_from_slice(pixels);
        }
    }

    // Run forever, until an error occurs.
    let mut loop_helper = LoopHelper::builder()
        .report_interval_s(0.25)
        .build_with_target_rate(TARGET_FPS);

    loop {
        let target_rate = if shared.state.turbo_mode.load(Ordering::SeqCst) {
            shared.state.args.turbo_mode_factor * TARGET_FPS
        } else {
            TARGET_FPS
        };
        loop_helper.set_target_rate(target_rate);

        loop_helper.loop_start();

        // Lock the buffer for the whole emulation step.
        let back = shared.state.gb_screen.back.lock()
            .expect("[T-emu] failed to lock back buffer");

        // Run the emulator
        let mut peripherals = DesktopPeripherals {
            back_buffer: back,
            keys: &shared.state.keys,
        };
        let res = emulator.execute_frame(&mut peripherals, |_| false);

        // React to abnormal disruptions
        match res {
            Err(Disruption::Terminated) => {
                shared.messages.send(Message::Quit).unwrap();
                break;
            }

            // This means that the emulator reached V-Blank and we want to
            // present the buffer we just wrote to the actual display.
            Ok(true) => {
                // Swap both buffers
                {
                    let mut front = shared.state.gb_screen.front.lock()
                        .expect("[T-emu] failed to lock front buffer");
                    mem::swap(&mut *front, &mut *peripherals.back_buffer);
                }
            }
            _ => {}
        }

        // Release the lock as soon as possible.
        drop(peripherals.back_buffer);

        if let Some(fps) = loop_helper.report_rate() {
            *shared.state.emulation_rate.lock().unwrap() = fps;
        }

        loop_helper.loop_sleep();
    }
}



// ===============================================================================================
// ===== Helper types ============================================================================
// ===============================================================================================

/// All Gameboy key states stored in one atomic `u8`.
///
/// This is used to update the state from the input thread while reading it on
/// the emulation thread.
#[derive(Debug)]
struct AtomicKeys(AtomicU8);

impl AtomicKeys {
    /// Returns a new instance with no key pressed.
    fn none() -> Self {
        let byte = Keys::none().0;
        Self(AtomicU8::new(byte))
    }

    /// Returns the non-atomic keys by loading the atomic value.
    fn as_keys(&self) -> Keys {
        let byte = self.0.load(Ordering::SeqCst);
        Keys(byte)
    }

    /// Modify the atomic value to set the `key` as pressed.
    fn set_key(&self, key: JoypadKey) {
        let mut keys = self.as_keys();
        keys.set_key(key);
        self.0.store(keys.0, Ordering::SeqCst);
    }

    /// Modify the atomic value to set the `key` as unpressed.
    fn unset_key(&self, key: JoypadKey) {
        let mut keys = self.as_keys();
        keys.unset_key(key);
        self.0.store(keys.0, Ordering::SeqCst);
    }
}

/// Messages that the worker threads can generate for the main thread.
enum Message {
    /// The application should exit.
    Quit,

    /// An error occured in the rendering thread. This will also exit the
    /// application.
    RenderError(Error),
}

/// Two buffer holding a gameboy screen.
///
/// The emulation thread constantly renders into the `back` buffer and swaps
/// the buffers each Gameboy-frame. The render thread reads the front buffer
/// whenever the host system can render a new frame.
struct GbScreenBuffer {
    /// The buffer the render thread reads from.
    front: Mutex<Vec<PixelColor>>,

    /// The buffer the emulation thread writes into.
    back: Mutex<Vec<PixelColor>>,
}

impl GbScreenBuffer {
    /// Two black buffers.
    fn new() -> Self {
        let buf = vec![PixelColor::black(); SCREEN_WIDTH * SCREEN_HEIGHT];
        Self {
            front: Mutex::new(buf.clone()),
            back: Mutex::new(buf),
        }
    }
}



#[derive(Clone)]
struct Shared {
    /// A channel to send messages to the main thread.
    messages: Sender<Message>,

    /// Several different things.
    state: Arc<SharedState>,
}

struct SharedState {
    /// The command line arguments.
    args: Args,

    /// The Gameboy keys currently being pressed.
    keys: AtomicKeys,

    /// Front and back buffer for the gameboy screen (has nothing to do with
    /// OpenGL).
    gb_screen: GbScreenBuffer,

    /// The current rate of emulation in FPS. Should be `TARGET_FPS` or at
    /// least very close to it.
    emulation_rate: Mutex<f64>,

    /// Whether we are currently in turbo mode.
    turbo_mode: AtomicBool,
}
