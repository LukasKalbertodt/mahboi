use crate::args::Args;

pub(crate) use self::tui::TuiDebugger;

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
