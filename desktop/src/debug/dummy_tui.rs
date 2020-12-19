//! Dummy versions of everything within `tui/mod.rs` for windows. That way we
//! don't have to use `cfg` attributes all over `main()`.
use failure::{bail, Error};

use mahboi::machine::Machine;
use crate::args::Args;
use super::{Action, WindowBuffer};


pub(crate) enum TuiDebugger {}

impl TuiDebugger {
    pub(crate) fn new(_: &Args) -> Result<Self, Error> {
        bail!("Debugging mode not usable on Windows!");
    }

    pub(crate) fn update(
        &mut self,
        _: bool,
        _: &Machine,
        _: WindowBuffer,
    ) -> Action {
        unreachable!()
    }
    pub(crate) fn should_pause(&mut self, _: &Machine) -> bool {
        unreachable!()
    }
}

pub(crate) fn init_logger() {
    panic!("Debugging mode not usable on Windows!");
}
