use std::{
    panic,
    sync::{
        Mutex,
        mpsc::{channel, Receiver},
    },
};

use cursive::{
    Cursive,
    theme::{Theme, BorderStyle, Effect, Color, BaseColor, Palette, PaletteColor},
    view::{Identifiable},
    views::{TextView, LinearLayout},
};
use failure::Error;
use lazy_static::lazy_static;
use log::{Log, Record, Level, Metadata};

use mahboi::{
    machine::Machine,
    primitives::Word,
};
use super::{Action};
use self::{
    asm_view::AsmView,
    log_view::LogView,
    tab_view::TabView,
};

mod asm_view;
mod tab_view;
mod log_view;


// ============================================================================
// ===== Logger
// ============================================================================
// So the logger should show the log messages in the TUI. Sadly, we can't
// directly to the views since log messages could come from all threads. So
// instead, we have a global buffer of log messages. New messages are inserted
// there and the TUI interface regularly checks for new messages and shows them
// in the TUI.

/// Initializes the logger that works in tandem with the TUI debugger.
pub(crate) fn init_logger() {
    log::set_logger(&TuiLogger)
        .expect("called init(), but a logger is already set!");
}

lazy_static! {
    static ref LOG_MESSAGES: Mutex<Vec<LogMessage>> = Mutex::new(Vec::new());
}

#[derive(Debug)]
struct LogMessage {
    level: Level,
    msg: String,
}

struct TuiLogger;

impl Log for TuiLogger {
    fn enabled(&self, _: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if record.module_path().map(|p| p.starts_with("mahboi")).unwrap_or(false) {
            // Just push them into the global list.
            LOG_MESSAGES.lock().unwrap().push(LogMessage {
                level: record.level(),
                msg: record.args().to_string(),
            });
        }
    }

    fn flush(&self) {}
}



/// A debugger that uses a terminal user interface. Used in `--debug` mode.
pub(crate) struct TuiDebugger {
    /// Handle to the special TUI terminal
    siv: Cursive,

    /// Paused state of the last `update()` call.
    is_paused: bool,

    /// Events that cannot be handled immediately and are stored here to be
    /// handled in `update`.
    pending_events: Receiver<char>,

    step_over: Option<Word>,
}

impl TuiDebugger {
    pub(crate) fn new() -> Result<Self, Error> {
        // Create a handle to the terminal (with the correct backend).
        let mut siv = Cursive::ncurses();

        // To handle events, we use `Cursive::step`. Sadly, this function
        // blocks to wait on an event before it returns. This isn't good. We
        // can force the `step()` method to return after one "TUI frame". By
        // setting this to 1000, we assure that `step()` waits for at most 1ms.
        // Still not perfect, but ok.
        siv.set_fps(1000);

        // Setup own panic hook.
        //
        // Unfortunately, the nice TUI has a disadvantage: panic messages are
        // written into the alternate screen and then that screen is destroyed
        // because the application unwinds. That means that the panic message
        // is basically lost.
        //
        // To avoid this, we install a panic hook that returns to the main
        // screen, before the message is printed.
        let previous_hook = panic::take_hook();
        panic::set_hook(Box::new(move |info| {
            // So this is not entirely cool. These two lines are executed
            // in the `drop()` function of `Cursive`. I have no idea if
            // it's bad to call these twice. But so far, nothing bad has
            // happend...
            println!("\x1B[?1002l");
            ncurses::endwin();

            // Execute previous hook.
            previous_hook(info)
        }));

        // Build the TUI view
        let events = setup_tui(&mut siv);

        let out = Self {
            siv,
            is_paused: false,
            pending_events: events,
            step_over: None,
        };

        Ok(out)
    }

    /// Updates the debugger view and handles events. Should be called
    /// regularly.
    ///
    /// Returns a requested action.
    pub(crate) fn update(
        &mut self,
        is_paused: bool,
        machine: &Machine,
    ) -> Result<Action, Error> {
        if !self.siv.is_running() {
            return Ok(Action::Quit);
        }

        // Check if the paused state has changed.
        if is_paused != self.is_paused {
            self.is_paused = is_paused;

            // Update the title (which contains the paused state)
            let state = if is_paused { "paused" } else { "running" };
            self.siv.call_on_id("main_title", |text: &mut TextView| {
                text.set_content(format!("Mahboi Debugger ({})", state));
            });
        }

        if is_paused {
            self.step_over = None;

            self.siv.call_on_id("asm_view", |asm: &mut AsmView| {
                asm.update(machine);
            });
        }

        // Append all log messages that were pushed to the global buffer into
        // the corresponding log view.
        self.siv.call_on_id("log_list", |list: &mut LogView| {
            for log in LOG_MESSAGES.lock().unwrap().drain(..) {
                list.add_row(log.level, log.msg);
            }
        });

        // Handle events and update view
        self.siv.step();

        // React to any events that might have happend
        while let Ok(c) = self.pending_events.try_recv() {
            match c {
                'p' => return Ok(Action::Pause),
                'r' => return Ok(Action::Continue),
                's' => {
                    if self.is_paused {
                        self.step_over = Some(machine.cpu.pc);
                        return Ok(Action::Continue);
                    }
                }
                _ => panic!("internal error: unexpected event"),
            }
        }

        Ok(Action::Nothing)
    }

    pub(crate) fn should_pause(&self, machine: &Machine) -> bool {
        if let Some(addr) = self.step_over {
            return addr != machine.cpu.pc;
        }

        false
    }
}

/// Prepares the `Cursive` instance by registering event handler and setting up
/// the view.
fn setup_tui(siv: &mut Cursive) -> Receiver<char> {
    // We always want to be able to quit the application via `q`.
    siv.add_global_callback('q', |s| s.quit());
    let (tx, receiver) = channel();

    for &c in &['p', 'r', 's'] {
        let tx = tx.clone();
        siv.add_global_callback(c, move |_| tx.send(c).unwrap());
    }

    // Create and set our theme.
    let mut palette = Palette::default();
    palette[PaletteColor::View] = Color::TerminalDefault;
    palette[PaletteColor::Primary] = Color::TerminalDefault;
    palette[PaletteColor::Secondary] = Color::TerminalDefault;
    palette[PaletteColor::Tertiary] = Color::TerminalDefault;
    palette[PaletteColor::TitlePrimary] = Color::TerminalDefault;
    palette[PaletteColor::TitleSecondary] = Color::TerminalDefault;
    palette[PaletteColor::Highlight] = Color::Dark(BaseColor::Red);
    palette[PaletteColor::HighlightInactive] = Color::TerminalDefault;
    let theme = Theme {
        shadow: false,
        borders: BorderStyle::Simple,
        palette,
    };
    siv.set_theme(theme);

    // Create view for log messages
    let log_list = LogView::new().with_id("log_list");

    let asm_view = AsmView::new().with_id("asm_view");

    let main_title = TextView::new("Mahboi Debugger")
        .effect(Effect::Bold)
        .center()
        .no_wrap()
        .with_id("main_title");

    let tabs = TabView::new()
        .tab("Event Log", log_list)
        .tab("Debugger", asm_view);

    let main_layout = LinearLayout::vertical()
        .child(main_title)
        .child(tabs);

    siv.add_fullscreen_layer(main_layout);

    receiver
}
