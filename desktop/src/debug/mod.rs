use failure::Error;

use self::{
    simple::SimpleDebugger,
    tui::{TuiDebugger},
};

pub mod tui;
mod simple;


pub(crate) enum SomeDebugger {
    Simple(SimpleDebugger),
    Tui(TuiDebugger),
}

impl SomeDebugger {
    pub(crate) fn from_flag(debug_mode: bool) -> Result<Self, Error> {
        if debug_mode {
            Ok(SomeDebugger::Tui(TuiDebugger::new()?))
        } else {
            Ok(SomeDebugger::Simple(SimpleDebugger))
        }
    }
}

pub(crate) fn init_logger(_debug_mode: bool) {
    tui::TuiLogger::init();
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
