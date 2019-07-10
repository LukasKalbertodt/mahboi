use std::{
    fs,
    panic::{AssertUnwindSafe, catch_unwind, resume_unwind},
    sync::{
        Arc, Condvar, Mutex,
        atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
    },
    time::{Duration, Instant},
    thread,
};

use failure::{Error, ResultExt};
use structopt::StructOpt;
use winit::{
    EventsLoop, EventsLoopProxy,
    dpi::{LogicalSize, PhysicalSize},
};

use mahboi::{
    Emulator, SCREEN_WIDTH, SCREEN_HEIGHT,
    log::*,
    cartridge::Cartridge,
    machine::input::{Keys, JoypadKey},
};
use crate::{
    args::Args,
    emu::emulator_thread,
    input::handle_event,
    render::{create_context, render_thread},
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
        println!("ERROR: {} ({:?}", e, e);

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


    // ----- Window ----------------------------------------------------------
    // The events loop is the core interface to the OS regarding events.
    let mut events_loop = EventsLoop::new();

    // Configure window
    //
    // TODO: this might be wrong when the window is not created on the
    // primary monitor. No idea if that can happen.
    let window_dpi_factor = events_loop.get_primary_monitor().get_hidpi_factor();
    let window_size = PhysicalSize::new(
        SCREEN_WIDTH as f64 * args.scale,
        SCREEN_HEIGHT as f64 * args.scale,
    );
    let window_size = window_size.to_logical(window_dpi_factor);

    // Create Vulkan "context"
    let context = create_context(&args, &events_loop, &window_size)?;

    // Create values that are shared across all threads.
    let shared = Arc::new(Shared {
        args,

        event_thread: events_loop.create_proxy(),
        should_quit: AtomicBool::new(false),

        keys: AtomicKeys::none(),
        gb_frame: Mutex::new(GbFrame::new()),
        frame_finished_event: Condvar::new(),
        emulation_rate: Mutex::new(TARGET_FPS),
        turbo_mode: AtomicBool::new(false),

        window_dpi_factor: Mutex::new(window_dpi_factor),
        window_size: Mutex::new(window_size),
        window_title: Mutex::new(WINDOW_TITLE.into()),

        // It's fine to use an instant that is "earlier" than a real value
        // would be. The duration also doesn't need to be exact.
        render_timing: Mutex::new(RenderTiming {
            next_draw_start: Instant::now(),
            frame_time: Duration::from_millis(16),
        }),
        dropped_frames: AtomicU64::new(0),
    });



    // ----- Render Thread ---------------------------------------------------
    let render_thread = {
        // Create a new handle to the shared values.
        let shared = shared.clone();
        let context = context.clone();

        thread::spawn(move || {
            let res = catch_unwind(AssertUnwindSafe(|| render_thread(context, &shared)));
            shared.request_quit();
            res.unwrap_or_else(|e| resume_unwind(e))
        })
    };

    // ----- Emulator Thread -------------------------------------------------
    let emulator_thread = {
        // Create a new handle to the shared values.
        let shared = shared.clone();

        thread::spawn(move || {
            let res = catch_unwind(AssertUnwindSafe(|| emulator_thread(emulator, &shared)));
            shared.request_quit();
            res.unwrap_or_else(|e| resume_unwind(e))
        })
    };



    // =======================================================================
    // ===== Handle events ===================================================
    // =======================================================================

    events_loop.run_forever(|event| {
        handle_event(&event, &shared, context.window())
    });

    // When we reached this point, the `run_forever` call returned because
    // `handle_event` returned `ControlFlow::Break`. This only happens if some
    // part of this application requests a "quit". This is stored in
    // `shared.should_quit`.
    //
    // The other threads will end themselves, so we just need to wait for them.
    // We actually have to since the render thread could return an error, which
    // we want to print. We also want to check if any thread panicked.
    debug!("Application shutting down: waiting for threads to finish");
    emulator_thread.join().map_err(|_| failure::err_msg("emulator thread panicked"))?;
    render_thread.join().map_err(|_| failure::err_msg("render thread panicked"))??;

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

    /// Incremented by the emulator thread each time it has finished a frame.
    /// Reset to 0 by the render thread when it receives and displays a frame.
    num_finished: u64,
}

impl GbFrame {
    /// A black frame.
    fn new() -> Self {
        Self {
            buffer: vec![(0, 0, 0); SCREEN_WIDTH * SCREEN_HEIGHT],
            timestamp: Instant::now(),
            num_finished: 0,
        }
    }
}


struct Shared {
    /// The command line arguments.
    args: Args,

    /// A handle to send a message to the main thread to wake up.
    event_thread: EventsLoopProxy,

    /// Whether the application should quit.
    should_quit: AtomicBool,

    /// The Gameboy keys currently being pressed.
    keys: AtomicKeys,

    /// Front buffer for the gameboy screen (has nothing to do with OpenGL).
    gb_frame: Mutex<GbFrame>,

    /// A condition variable that is notified by the emulator thread whenever a
    /// frame is finished.
    frame_finished_event: Condvar,

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

    /// The title of the window. Usually set by the render thread and read by
    /// the main thread. The render thread cannot directly set the title
    /// because all window interaction on MacOS has to be in the main thread.
    window_title: Mutex<String>,

    /// This is written by the render thread each frame. It is mostly used by
    /// the emulator thread to synchronize sleeping.
    render_timing: Mutex<RenderTiming>,

    /// We count the number of frames that were created by the emulator but not
    /// shown by the render thread.
    dropped_frames: AtomicU64,
}

impl Shared {
    fn request_quit(&self) {
        self.should_quit.store(true, Ordering::SeqCst);
        self.event_thread.wakeup()
            .expect("event thread unexpectedly already finished");
        self.frame_finished_event.notify_one();
    }
}

/// Information about the timing of the rendering thread.
#[derive(Debug, Clone, Copy)]
struct RenderTiming {
    /// Approximately when the render thread will start drawing next.
    next_draw_start: Instant,

    /// The duration of one host frame.
    frame_time: Duration,
}

trait DurationExt {
    type Out;
    fn saturating_sub(self, rhs: Self) -> Self::Out;
}

impl DurationExt for Duration {
    type Out = Self;
    fn saturating_sub(self, rhs: Self) -> Self::Out {
        if self > rhs {
            self - rhs
        } else {
            Duration::from_millis(0)
        }
    }
}

impl DurationExt for Instant {
    type Out = Duration;
    fn saturating_sub(self, rhs: Self) -> Self::Out {
        if self > rhs {
            self - rhs
        } else {
            Duration::from_millis(0)
        }
    }
}
