use std::{
    io::{self, Write},
    panic,
    sync::{
        Mutex, Arc, TryLockError,
        mpsc::{channel, Receiver},
    },
    thread,
};

use cursive::{
    Cursive,
    direction::Orientation,
    theme::{Theme, BorderStyle, ColorStyle, Effect, Color, ColorType, BaseColor, Palette, PaletteColor},
    view::{Boxable, Identifiable},
    views::{TextView, LinearLayout, ListView, SelectView},
};
use failure::{Error, ResultExt};
use lazy_static::lazy_static;
use log::{Log, Record, Level, Metadata};

use super::{Action};
use self::{
    log_view::LogView,
    tab_view::TabView,
};

mod tab_view;
mod log_view;




lazy_static! {
    pub static ref LOG_MESSAGES: Mutex<Vec<LogMessage>> = Mutex::new(Vec::new());
}

#[derive(Debug)]
pub struct LogMessage {
    level: Level,
    msg: String,
}

pub(crate) struct TuiLogger;

impl TuiLogger {
    pub(crate) fn init() {
        log::set_logger(&TuiLogger)
            .expect("called init(), but a logger is already set!");
    }
}

impl Log for TuiLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if record.module_path().map(|p| p.starts_with("mahboi")).unwrap_or(false) {
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
        setup_tui(&mut siv);

        let out = Self {
            siv,
            is_paused: false,
        };

        // Already draw once
        // out.draw()?;

        Ok(out)
    }

    /// Updates the debugger view and handles events. Should be called
    /// regularly.
    ///
    /// Returns a requested action.
    pub(crate) fn update(&mut self, is_paused: bool) -> Result<Action, Error> {
        if !self.siv.is_running() {
            return Ok(Action::Quit);
        }

        self.siv.call_on_id("log_list", |list: &mut LogView| {
            for log in LOG_MESSAGES.lock().unwrap().drain(..) {
                list.add_row(log.level, log.msg);
            }
        });

        self.siv.step();

        Ok(Action::Nothing)
    }
}

fn setup_tui(siv: &mut Cursive) {
    // We always want to be able to quit the application via `q`.
    siv.add_global_callback('q', |s| s.quit());

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

    let log_list = LogView::new()
        .with_id("log_list")
        .full_screen();

    let main_title = TextView::new("Mahboi Debugger")
        .effect(Effect::Bold)
        .center()
        .no_wrap();

    let tabs = TabView::new()
        .tab("Event Log", log_list)
        .tab("Debugger", TextView::new("Hello in the debugger tab!"));

    let main_layout = LinearLayout::vertical()
        .child(main_title)
        .child(tabs);

    siv.add_fullscreen_layer(main_layout);
}
