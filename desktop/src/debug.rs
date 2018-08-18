use std::{
    io::{self, Write},
    panic,
    sync::{
        Mutex, Arc, TryLockError,
        mpsc::{channel, Receiver},
    },
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
    widgets::{Item, List, Widget, Paragraph, Tabs},
};

use mahboi::env::{Debugger, EventLevel};


/// Returned from `TuiDebugger::update` to tell the main loop what to do.
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

/// A debugger that uses a terminal user interface.
pub(crate) struct TuiDebugger {
    inner: Arc<Mutex<Option<TuiDebuggerInner>>>,
}

/// Actual TUI debugger data.
///
/// For multiple reasons, we need interior mutability in the `TuiDebugger`. The
/// main reason is that we want to drop the terminal early when a panic is
/// triggered. The panic hook requires `'static` access to this from
/// potentially other threads. Thus, we need to use `Arc<Mutex<>>`.
///
/// But this interior mutability has some advantages as well. Now every method
/// can be called with an immutable reference, so the `TuiDebugger` can be
/// passed around easily.
struct TuiDebuggerInner {
    /// Handle to the special TUI terminal
    term: Terminal<Backend>,

    /// Current size of the terminal
    size: Rect,

    /// Events from the terminal that haven't been handled yet.
    input_events: Receiver<Result<Event, io::Error>>,

    /// List of all events received via `post_event`.
    event_log: Vec<(String, Style)>,

    /// View: the index of the selected tab.
    selected_tab: u8,
}


impl TuiDebugger {
    pub(crate) fn new() -> Result<Self, Error> {
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

        // Create the inner debugger
        let inner = TuiDebuggerInner {
            term,
            size,
            input_events,
            selected_tab: 0,
            event_log: vec![],
        };
        let inner = Arc::new(Mutex::new(Some(inner)));


        // Setup own panic hook.
        //
        // Unfortunately, the nice TUI has a disadvantage: panic messages are
        // written into the alternate screen and then that screen is destroyed
        // because the application unwinds. That means that the panic error is
        // basically lost.
        //
        // To avoid this, we install a panic hook that drops the terminal and
        // returns to the main screen, before the message is printed.
        {
            let previous_hook = panic::take_hook();
            let inner = inner.clone();
            panic::set_hook(Box::new(move |info| {
                println!("IN HOOOK");
                // We have to be careful here. We don't want to have a dead
                // lock in the panic hook. That would be bad, presumably.
                match inner.try_lock() {
                    // No one holds the lock right now, so we can access the
                    // value.
                    Ok(mut inner) => {
                        // We explicitly drop the value to reset the terminal.
                        drop(inner.take());
                    }

                    // The thread holding the lock panicked. This means that
                    // our `inner` can be in a semantically invalid state. We
                    // don't care about that though, so we can access the
                    // value.
                    Err(TryLockError::Poisoned(e)) => {
                        // We explicitly drop the value to reset the terminal.
                        drop(e.into_inner());
                    }

                    // In this case, another thread (than the currently
                    // panicking one) holds the lock. In that case we cannot
                    // access the terminal. So we have to switch to the main
                    // screen manually. This only switches the screen but
                    // doesn't reset certain terminal states. So this is
                    // suboptimal.
                    Err(TryLockError::WouldBlock) => {
                        print!("{}", termion::screen::ToMainScreen);
                    }
                }
                // We ignore the error here to avoid panicking in a panic hook.
                let _ = io::stdout().flush();

                // Execute previous hook.
                previous_hook(info)
            }));
        }

        let out = Self { inner };

        // Already draw once
        out.with_inner(|inner| inner.draw())?;

        Ok(out)
    }

    /// Updates the debugger view and handles events. Should be called
    /// regularly.
    ///
    /// Returns a requested action.
    pub(crate) fn update(&self) -> Result<Action, Error> {
        self.with_inner(|inner| inner.update())
    }

    /// Helper method to do something with the locked `inner` value.
    fn with_inner<F, T>(&self, fun: F) -> Result<T, Error>
    where
        F: Send + FnOnce(&mut TuiDebuggerInner) -> Result<T, Error>,
    {
        let mut guard = self.inner.lock()
            .map_err(|_| failure::err_msg("failed to aquire debugger lock"))?;

        let inner = guard.as_mut()
            .ok_or(failure::err_msg("access to dropped deubgger"))?;

        Ok(fun(inner)?)
    }
}

impl TuiDebuggerInner {
    /// See `TuiDebugger::update`.
    fn update(&mut self) -> Result<Action, Error> {
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
        let new_size = self.term.size()?;
        if new_size != self.size {
            self.term.resize(new_size)?;
            self.size = new_size;
        }

        // Draw the UI.
        self.draw()?;

        Ok(Action::Nothing)
    }

    /// Draws the complete UI to the terminal.
    fn draw(&mut self) -> Result<(), Error> {
        let main_title = "Mahboi Debugger (running)";

        let event_log = &self.event_log;
        let events = event_log.iter().map(|(msg, style)| {
            Item::StyledData(msg, style)
        });
        let selected_tab = self.selected_tab;

        Group::default()
            .direction(Direction::Vertical)
            .sizes(&[Size::Fixed(2), Size::Fixed(1), Size::Fixed(1), Size::Percent(100)])
            .render(&mut self.term, &self.size, |t, chunks| {
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

        self.term.draw().context("failed to draw terminal")?;

        Ok(())
    }

    /// Actual implementation of `Debugger:post_event`.
    fn post_event(&mut self, level: EventLevel, msg: String) {
        let (level_name, color) = match level {
            EventLevel::Info => ("INFO: ", Color::Blue),
            EventLevel::Debug => ("DEBUG:", Color::White),
            EventLevel::Trace => ("TRACE:", Color::Gray),
        };

        let mut iter = msg.split('\n');

        // Push first line (the iterator always contains one element)
        self.event_log.push((
            format!("{} {}", level_name, iter.next().unwrap()),
            Style::default().fg(color),
        ));

        // Push all remaining lines with `|` at the start
        for line in iter {
            self.event_log.push((
                format!("     | {}", line),
                Style::default().fg(color),
            ));
        }
    }
}

impl Drop for TuiDebugger {
    fn drop(&mut self) {
        // Show cursor again
        print!("{}", termion::cursor::Show);
        let _ = io::stdout().flush();

        // We don't use `with_inner` here, because we want to avoid a dead
        // lock, thus we use `try_lock()`.
        if let Ok(mut guard) = self.inner.try_lock() {
            // Explicitly drop the terminal to reset state.
            drop(guard.take());
        }
    }
}

impl Debugger for TuiDebugger {
    fn post_event(&self, level: EventLevel, msg: String) {
        self.with_inner(|inner| {
            inner.post_event(level, msg);
            Ok(())
        }).expect("couldn't aquire lock to debugger");
    }
}
