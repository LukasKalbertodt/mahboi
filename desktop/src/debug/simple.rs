use log::{Log, Record, Metadata};


/// Initializes a simple logging implementation.
pub(crate) fn init_logger() {
    log::set_logger(&SimpleLogger)
        .expect("called init(), but a logger is already set!");
}

/// A simple logger that simply prints all events to the terminal. Used in non
/// `--debug` mode.
struct SimpleLogger;

impl Log for SimpleLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if record.module_path().map(|p| p.starts_with("mahboi")).unwrap_or(false) {
            if self.enabled(record.metadata()) {
                println!("{:5}: {}", record.level(), record.args());
            }
        }
    }

    fn flush(&self) {}
}
