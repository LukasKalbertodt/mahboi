use mahboi::env::{Debugger, EventLevel};

pub(crate) struct CliDebugger {}

impl Debugger for CliDebugger {
    fn post_event(&mut self, _level: EventLevel, msg: String) {
        println!("{}", msg);
        // TODO implement level check (get level from cli args)
    }
}
