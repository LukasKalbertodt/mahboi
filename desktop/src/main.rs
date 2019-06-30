use std::{
    fs,
    mem,
    sync::{
        Arc, Mutex, MutexGuard,
        mpsc::{channel, Sender},
        atomic::{AtomicU8, Ordering},
    },
    thread,
};

use failure::{Error, ResultExt};
use glium::{
    Display, Program, VertexBuffer, Surface,
    implement_vertex, uniform,
    glutin::{
        ContextBuilder, EventsLoop, WindowBuilder, WindowedContext, NotCurrent,
    },
    index::NoIndices,
    program::ProgramCreationInput,
    texture::{
        UnsignedTexture2d, UncompressedUintFormat, MipmapsOption,
        pixel_buffer::PixelBuffer,
    },
};
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
    let keys = Arc::new(AtomicKeys::none());
    let gb_buffer = Arc::new(GbScreenBuffer::new());
    let (messages, incoming_messages) = channel();

    // Here we start the input thread. It's a bit awkward because the input
    // thread needs the `EventsLoop`, but this type cannot be sent to new
    // thread. So we have to already create it in the correct thread.
    // Furthermore, it is needed to create the `glium` display later, which
    // also cannot be transferred across threads. Luckily, we can build a
    // glutin context already and transfer it across threads.
    let context = {
        // Clone arcs and sender.
        let keys = keys.clone();
        let messages = messages.clone();

        // Even more awkward: to send the glutin context back to the main
        // thread, we cannot just return it, because the input thread runs
        // forever. Thus, we need a one-time channel here.
        let (tx, rx) = channel();

        thread::spawn(move || {
            // Create the main events loop, a window and a context.
            let events_loop = EventsLoop::new();
            let wb = WindowBuilder::new()
                .with_title(WINDOW_TITLE);
            let cb = ContextBuilder::new()
                .with_vsync(true);
            let context = cb.build_windowed(wb, &events_loop);
            info!("[desktop] Opened window");

            // The context is not needed in the input thread anymore, but it's
            // needed in the main thread. So send it back.
            tx.send(context).unwrap();

            // Start polling input events (forever)
            input_thread(events_loop, keys, messages);
        });

        // Receive the context from the thread.
        rx.recv().unwrap()?
    };


    // ----- Render Thread ---------------------------------------------------
    {
        // Clonse arc and sender.
        let gb_buffer = gb_buffer.clone();
        let messages = messages.clone();

        thread::spawn(move || {
            // There could actually go something wrong in the render thread. If
            // that's the case, we send an action to the main thread.
            let result = render_thread(context, gb_buffer);
            if let Err(e) = result {
                messages.send(Message::RenderError(e)).unwrap();
            }
        });
    }

    // ----- Emulator Thread -------------------------------------------------
    {
        // Clonse arcs and sender.
        let gb_buffer = gb_buffer.clone();
        let keys = keys.clone();
        let messages = messages.clone();

        thread::spawn(|| {
            emulator_thread(emulator, gb_buffer, keys, messages);
        });
    }




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
    keys: Arc<AtomicKeys>,
    messages: Sender<Message>,
) {
    use glium::glutin::{
        ControlFlow, ElementState as State, Event, KeyboardInput,
        VirtualKeyCode as Key, WindowEvent,
    };

    // Mini helper function
    let send_action = |action| {
        messages.send(action)
            .expect("failed to send input action: input thread will panic now");
    };

    events_loop.run_forever(move |event| {
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
    gb_buffer: Arc<GbScreenBuffer>,
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


    loop {
        // We map the pixel buffer and write directly to it.
        {
            let mut pixel_buffer = pixel_buffer.map_write();
            let front = gb_buffer.front.lock().expect("failed to lock front buffer");
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

        display.finish();
    }
}

/// Drives the emulation. The emulator writes into the `gb_buffer` back buffer.
/// Both of those buffers are swapped after each Gameboy frame. The emulator
/// additionally reads from `keys`. Lastly, if the emulator terminates in an
/// unusual fashion, a `Quit` message is send to the main thread.
fn emulator_thread(
    mut emulator: Emulator,
    gb_buffer: Arc<GbScreenBuffer>,
    keys: Arc<AtomicKeys>,
    messages: Sender<Message>,
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
    loop {
        let mut back = gb_buffer.back.lock().expect("[T-emu] failed to lock back buffer");

        // Swap both buffers
        {
            let mut front = gb_buffer.front.lock().expect("[T-emu] failed to lock front buffer");
            mem::swap(&mut *front, &mut *back);
        }

        // Run the emulator
        let mut peripherals = DesktopPeripherals {
            back_buffer: back,
            keys: &keys,
        };
        let res = emulator.execute_frame(&mut peripherals, |_| false);

        // React to abnormal disruptions
        match res {
            Err(Disruption::Terminated) => {
                messages.send(Message::Quit).unwrap();
                break;
            }
            _ => {}
        }
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
