use crate::args::Args;

pub(crate) use self::tui::TuiDebugger;


#[cfg_attr(windows, path = "dummy_tui.rs")]
mod tui;
mod simple;



/// Initializes the logging implementation.
///
/// If `debug_mode` is true, a nice TUI logger is used. If it's `false`, a
/// simple logger is used that just prints everything to stdout.
pub(crate) fn init_logger(args: &Args) {
    let default_log_level = if args.debug {
        log::LevelFilter::Trace
    } else {
        log::LevelFilter::Error
    };
    log::set_max_level(args.log_level.unwrap_or(default_log_level));

    if args.debug {
        tui::init_logger();
    } else {
        simple::init_logger();
    }
}


/// Returned from `TuiDebugger::update` to tell the main loop what to do.
#[must_use]
#[cfg_attr(windows, allow(dead_code))]
pub(crate) enum Action {
    /// Quit the application
    Quit,

    /// Pause execution
    Pause,

    /// Continue execeution
    Continue,

    /// Don't do anything special and keep running.
    Nothing,
}

pub(crate) struct WindowBuffer<'a>(pub(crate) &'a mut [u8]);

impl WindowBuffer<'_> {
    #[cfg_attr(windows, allow(dead_code))]
    fn paint_pink(&mut self) {
        for chunk in self.0.chunks_mut(4) {
            chunk[0] = 0xFF;
            chunk[1] = 0x69;
            chunk[2] = 0xB4;
        }
    }
}
