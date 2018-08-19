
/// A simple debugger that simply prints all events to the terminal and cannot
/// do anything else. Used in non `--debug` mode.
pub(crate) struct SimpleDebugger;

// impl Debugger for SimpleDebugger {
//     fn post_event(&self, level: EventLevel, msg: String) {
//         // TODO: Maybe add colors
//         let level_name = match level {
//             EventLevel::Info => "INFO: ",
//             EventLevel::Debug => "DEBUG:",
//             EventLevel::Trace => "TRACE:",
//         };

//         println!("{} {}", level_name, msg);
//     }
// }
