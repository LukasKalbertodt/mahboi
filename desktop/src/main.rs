use std::{
    fs,
    sync::{
        Arc, Mutex,
        mpsc::{channel, Sender},
        atomic::{AtomicBool, AtomicU8, Ordering},
    },
    time::{Duration, Instant},
    thread,
};

use failure::{Error, ResultExt};
use glium::{
    glutin::{
        ContextBuilder, EventsLoop, WindowBuilder,
        dpi::{LogicalSize, PhysicalSize},
    },
};
use structopt::StructOpt;

use mahboi::{
    Emulator, SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
    cartridge::Cartridge,
    machine::input::{Keys, JoypadKey},
};
use crate::{
    args::Args,
    emu::emulator_thread,
    input::input_thread,
    render::render_thread,
};


mod args;
mod emu;
mod input;
mod render;
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
    builder.default_format_timestamp_nanos(true);
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
            gb_frame: Mutex::new(GbFrame::new()),
            emulation_rate: Mutex::new(TARGET_FPS),
            turbo_mode: AtomicBool::new(false),

            // Dummy values that are overwritten later
            window_dpi_factor: Mutex::new(1.0),
            window_size: Mutex::new(LogicalSize::new(1.0, 1.0)),

            // It's fine to use an instant that is "earlier" than a real value
            // would be.
            render_timing: Mutex::new(RenderTiming {
                last_host_frame_start: Instant::now(),
                draw_delay: Duration::from_secs(0),
            }),
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
            let size = size.to_logical(dpi_factor);
            *shared.state.window_dpi_factor.lock().unwrap() = dpi_factor;
            *shared.state.window_size.lock().unwrap() = size;

            let wb = WindowBuilder::new()
                .with_dimensions(size)
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

/// A Gameboy frame generated by the emulator.
///
/// The emulation thread constantly renders into its own back buffer and swaps
/// that buffer with this one each Gameboy-frame. The render thread reads the
/// this buffer whenever the host system can render a new frame.
struct GbFrame {
    /// The buffer the render thread reads from.
    buffer: Vec<(u8, u8, u8)>,

    /// The instant the emulation creating this buffer was started.
    timestamp: Instant,
}

impl GbFrame {
    /// A black frame.
    fn new() -> Self {
        Self {
            buffer: vec![(0, 0, 0); SCREEN_WIDTH * SCREEN_HEIGHT],
            timestamp: Instant::now(),
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

    /// Front buffer for the gameboy screen (has nothing to do with OpenGL).
    gb_frame: Mutex<GbFrame>,

    /// The current rate of emulation in FPS. Should be `TARGET_FPS` or at
    /// least very close to it.
    emulation_rate: Mutex<f64>,

    /// Whether we are currently in turbo mode.
    turbo_mode: AtomicBool,

    /// The DPI factor of the window. This value is updated by the input
    /// thread.
    window_dpi_factor: Mutex<f64>,

    /// The current logical size of the window. This value is updated by the
    /// input thread.
    window_size: Mutex<LogicalSize>,

    /// This is written by the render thread each frame. It is mostly used by
    /// the emulator thread to synchronize sleeping.
    render_timing: Mutex<RenderTiming>,
}

/// Information about the timing of the rendering thread.
#[derive(Debug, Clone, Copy)]
struct RenderTiming {
    /// The instant the last host (OpenGL) frame was started. Started means
    /// directly becore the `draw_delay` sleep.
    last_host_frame_start: Instant,

    /// How much the render thread currently sleeps before starting to draw
    /// anything.
    draw_delay: Duration,
}

trait DurationExt {
    fn saturating_sub(self, rhs: Self) -> Self;
}

impl DurationExt for Duration {
    fn saturating_sub(self, rhs: Self) -> Self {
        if self > rhs {
            self - rhs
        } else {
            Duration::from_millis(0)
        }
    }
}
