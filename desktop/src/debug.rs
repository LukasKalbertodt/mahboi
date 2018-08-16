use mahboi::env::{Debugger, EventLevel};

pub(crate) struct CliDebugger {}

impl Debugger for CliDebugger {
    fn post_event(&mut self, level: EventLevel, msg: String) {
        println!("{}", msg);
    }
}
