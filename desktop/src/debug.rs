use std::{
    cell::RefCell,
    io::{self, Write},
    panic,
    sync::{
        Mutex, Arc,
        mpsc::{channel, Receiver},
    },
    rc::Rc,
    thread,
};

use failure::{Error, ResultExt};
use termion::{
    event::{Event, Key},
    input::{TermRead, MouseTerminal},
    screen::AlternateScreen,
    raw::{RawTerminal, IntoRawMode},
};
use tui::{
    Terminal,
    backend::TermionBackend,
    layout::{Group, Size, Direction, Rect},
    style::{Alignment, Color, Style, Modifier},
    widgets::{Item, List, Widget, Block, Borders, Paragraph, Tabs},
};

use mahboi::env::{Debugger, EventLevel};


#[must_use]
pub(crate) enum Action {
    /// Quit the application
    Quit,

    /// Pause execution
    Pause,

    /// Don't do anything special and keep running.
    Nothing,
}

const NUM_TABS: u8 = 2;

type Backend = TermionBackend<AlternateScreen<MouseTerminal<RawTerminal<io::Stdout>>>>;

pub(crate) struct TuiDebugger {
    term: Arc<Mutex<Option<Terminal<Backend>>>>,
    size: Rect,
    input_events: Receiver<Result<Event, io::Error>>,

    selected_tab: u8,

    shared: SharedTuiDebugger,
}

#[derive(Clone)]
pub(crate) struct SharedTuiDebugger {
    event_log: Rc<RefCell<Vec<(String, Style)>>>,

}

impl TuiDebugger {
    pub(crate) fn new() -> Result<Self, Error> {
        // Prepare the thread that will be listening for terminal events. This
        // thread will run the whole time in the background. It's usually only
        // stopped if the main thread stops.
        let (event_sender, input_events) = channel();
        thread::spawn(move || {
            let stdin = io::stdin();
            for e in stdin.events() {
                let res = event_sender.send(e);
                if res.is_err() {
                    break;
                }
            }
        });

        // Create a handle to the terminal (with the correct backend).
        let mut term = Terminal::new(
            TermionBackend::with_stdout(
                AlternateScreen::from(
                    MouseTerminal::from(
                        io::stdout()
                            .into_raw_mode()
                            .context("failed to switch to terminal raw mode")?
                    )
                )
            )
        )?;
        term.hide_cursor()?;
        let size = term.size()?;

        let term = Arc::new(Mutex::new(Some(term)));

        // Setup own panic hook to be notified when the thread panics
        let previous_hook = panic::take_hook();
        let term_copy = term.clone();
        panic::set_hook(Box::new(move |info| {
            let mut term = term_copy.try_lock().unwrap().take();
            term.unwrap().show_cursor().expect("failed to show cursor");
            io::stdout().flush().expect("failed to flush stdout");
            previous_hook(info)

        }));

        let shared = SharedTuiDebugger {
            event_log: Rc::new(RefCell::new(vec![])),
        };
        let mut out = Self {
            term,
            size,
            input_events,
            shared,
            selected_tab: 0,
        };

        // Already draw once
        out.draw()?;

        Ok(out)
    }

    pub(crate) fn shared(&self) -> SharedTuiDebugger {
        self.shared.clone()
    }

    /// Updates the debugger view and handles events. Should be called
    /// regularly.
    pub(crate) fn update(&mut self) -> Result<Action, Error> {
        // Handle any terminal events that might have occured.
        while let Ok(event) = self.input_events.try_recv() {
            self.post_event(EventLevel::Trace, format!("{:?}", event));
            match event? {
                Event::Key(Key::Char('q')) => return Ok(Action::Quit),
                Event::Key(Key::Char('p')) => return Ok(Action::Pause),
                Event::Key(Key::PageUp) => {
                    if self.selected_tab > 0 {
                        self.selected_tab -= 1;
                    }
                }
                Event::Key(Key::PageDown) => {
                    if self.selected_tab < NUM_TABS - 1 {
                        self.selected_tab += 1;
                    }
                }
                _ => {},
            }
        }

        // Resize terminal if necessary
        {
            let mut term_guard = self.term.lock().unwrap();
            let term = term_guard.as_mut().unwrap();

            let new_size = term.size()?;
            if new_size != self.size {
                term.resize(new_size)?;
                self.size = new_size;
            }
        }

        // Draw the UI.
        self.draw()?;

        Ok(Action::Nothing)
    }

    /// Draws the complete UI to the terminal.
    fn draw(&mut self) -> Result<(), Error> {
        let mut term_guard = self.term.lock().unwrap();
        let term = term_guard.as_mut().unwrap();

        let main_title = "Mahboi Debugger (running)";

        let event_log = self.shared.event_log.borrow();
        let events = event_log.iter().map(|(msg, style)| {
            Item::StyledData(msg, style)
        });
        let selected_tab = self.selected_tab;

        Group::default()
            .direction(Direction::Vertical)
            .sizes(&[Size::Fixed(2), Size::Fixed(1), Size::Fixed(1), Size::Percent(100)])
            .render(term, &self.size, |t, chunks| {
                let top_style = Style::default().bg(Color::Rgb(20, 20, 20));

                // Render main title
                Paragraph::default()
                    .text(main_title)
                    .style(top_style.clone().fg(Color::Green).modifier(Modifier::Bold))
                    .alignment(Alignment::Center)
                    .render(t, &chunks[0]);

                // Render tab bar
                Tabs::default()
                    .titles(&["Event Log", "Debugging"])
                    .select(selected_tab.into())
                    .style(top_style.fg(Color::White))
                    .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::Bold))
                    .render(t, &chunks[1]);

                // Render body
                match selected_tab {
                    0 => {
                        List::new(events)
                            .render(t, &chunks[3])
                    }
                    1 => {
                        Paragraph::default()
                            .text("Debugging only possible when emulator is paused")
                            .alignment(Alignment::Center)
                            .render(t, &chunks[3])
                    }
                    _ => panic!("internal error: invalid tab selected"),
                }
            });

        term.draw().context("failed to draw terminal")?;

        Ok(())
    }
}

impl Drop for TuiDebugger {
    fn drop(&mut self) {
        self.term.lock().unwrap().as_mut().map(|t|
            t.show_cursor().expect("failed to show cursor")
        );
    }
}

impl Debugger for TuiDebugger {
    fn post_event(&self, level: EventLevel, msg: String) {
        self.shared.post_event(level, msg)
    }
}

impl Debugger for SharedTuiDebugger {
    fn post_event(&self, level: EventLevel, msg: String) {
        let (level_name, color) = match level {
            EventLevel::Info => ("INFO: ", Color::Blue),
            EventLevel::Debug => ("DEBUG:", Color::White),
            EventLevel::Trace => ("TRACE:", Color::Gray),
        };

        self.event_log.borrow_mut().push((
            format!("{} {}", level_name, msg),
            Style::default().fg(color),
        ));
    }
}
