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
    views::TextView,
};
use failure::{Error, ResultExt};


mod tab_view;


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

// impl Debugger for SomeDebugger {
//     fn post_event(&self, level: EventLevel, msg: String) {
//         match self {
//             SomeDebugger::Simple(d) => d.post_event(level, msg),
//             SomeDebugger::Tui(d) => d.post_event(level, msg),
//         }
//     }
// }

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

        self.siv.step();

        Ok(Action::Nothing)
    }
}

fn setup_tui(siv: &mut Cursive) {
    use cursive::{
        direction::Orientation,
        theme::Effect,
        view::Boxable,
        views::{LinearLayout, ListView, SelectView},
    };

    // We always want to be able to quit the application via `q`.
    siv.add_global_callback('q', |s| s.quit());

    let list = ListView::new()
        .child("foo", TextView::new("whatsaup"))
        .child("bararoo", TextView::new("grüne tomaten"))
        .full_screen();

    let main_title = TextView::new("Mahboi Debugger")
        .effect(Effect::Bold)
        .center()
        .no_wrap();

    let tabs = tab_view::TabView::new(vec!["Event Log", "Debugger", "Test", "Foobar"]);

    let main_layout = LinearLayout::vertical()
        .child(main_title)
        .child(tabs)
        .child(list);

    siv.add_fullscreen_layer(main_layout);
    // siv.add_layer(TextView::new("Hello World!\nPress q to quit."));
}

// impl TuiDebuggerInner {
//     /// See `TuiDebugger::update`.
//     fn update(&mut self, is_paused: bool) -> Result<Action, Error> {
//         // // Handle any terminal events that might have occured.
//         // while let Ok(event) = self.input_events.try_recv() {
//         //     let event = event?;
//         //     self.post_event(EventLevel::Trace, format!("{:?}", event));

//         //     // Global key bindings
//         //     match event {
//         //         Event::Key(Key::Char('q')) => return Ok(Action::Quit),
//         //         Event::Key(Key::PageUp) => {
//         //             if self.selected_tab > 0 {
//         //                 self.selected_tab -= 1;
//         //             }
//         //         }
//         //         Event::Key(Key::PageDown) => {
//         //             if self.selected_tab < NUM_TABS - 1 {
//         //                 self.selected_tab += 1;
//         //             }
//         //         }
//         //         _ => {},
//         //     }

//         //     // Key bindings for debug tab
//         //     if self.selected_tab == DEBUG_TAB {
//         //         match event {
//         //             Event::Key(Key::Char('p')) => return Ok(Action::Pause),
//         //             Event::Key(Key::Char('r')) => return Ok(Action::Continue),
//         //             _ => {}
//         //         }
//         //     }
//         // }

//         // // Resize terminal if necessary
//         // let new_size = self.term.size()?;
//         // if new_size != self.size {
//         //     self.term.resize(new_size)?;
//         //     self.size = new_size;
//         // }

//         // // If the emulator was just paused, we switch the the debugger tab
//         // if self.is_paused != is_paused {
//         //     self.selected_tab = 1;
//         // }
//         // self.is_paused = is_paused;

//         // // Draw the UI.
//         // self.draw()?;

//         Ok(Action::Nothing)
//     }

//     /// Draws the complete UI to the terminal.
//     fn draw(&mut self) -> Result<(), Error> {
//         // let main_title = "Mahboi Debugger (running)";

//         // let selected_tab = self.selected_tab;
//         // let events = self.event_log.iter().map(|(msg, style)| {
//         //     Item::StyledData(msg, style)
//         // });

//         // let keymap_string = self.keymap_string();

//         // let body_height = self.size.height - 2 - 1 - 1 - 1 - 2;
//         // Group::default()
//         //     .direction(Direction::Vertical)
//         //     .sizes(&[
//         //         Size::Fixed(2),     // Title
//         //         Size::Fixed(1),     // Tab bar
//         //         Size::Fixed(1),     // Empty space
//         //         Size::Fixed(body_height), // Body
//         //         Size::Fixed(1),     // Empty space
//         //         Size::Fixed(2),     // Keymap
//         //     ])
//         //     .render(&mut self.term, &self.size, |t, chunks| {
//         //         let top_style = Style::default().bg(Color::Rgb(20, 20, 20));

//         //         // Render main title
//         //         Paragraph::default()
//         //             .text(main_title)
//         //             .style(top_style.clone().fg(Color::Green).modifier(Modifier::Bold))
//         //             .alignment(Alignment::Center)
//         //             .render(t, &chunks[0]);

//         //         // Render tab bar
//         //         Tabs::default()
//         //             .titles(&["Event Log", "Debugging"])
//         //             .select(selected_tab.into())
//         //             .style(top_style.fg(Color::White))
//         //             .highlight_style(Style::default().fg(Color::Yellow).modifier(Modifier::Bold))
//         //             .render(t, &chunks[1]);

//         //         // Render body
//         //         match selected_tab {
//         //             0 => {
//         //                 List::new(events)
//         //                     .render(t, &chunks[3])
//         //             }
//         //             1 => {
//         //                 Paragraph::default()
//         //                     .text("Debugging only possible when emulator is paused")
//         //                     .alignment(Alignment::Center)
//         //                     .render(t, &chunks[3])
//         //             }
//         //             _ => panic!("internal error: invalid tab selected"),
//         //         }

//         //         // Render keymap
//         //         Paragraph::default()
//         //             .text(&keymap_string)
//         //             .block(Block::default().title("Controls").borders(Borders::TOP))
//         //             .render(t, &chunks[5]);
//         //     });

//         // self.term.draw().context("failed to draw terminal")?;

//         Ok(())
//     }

//     // /// Actual implementation of `Debugger:post_event`.
//     // fn post_event(&mut self, level: EventLevel, msg: String) {
//     //     // let (level_name, color) = match level {
//     //     //     EventLevel::Info => ("INFO: ", Color::Blue),
//     //     //     EventLevel::Debug => ("DEBUG:", Color::White),
//     //     //     EventLevel::Trace => ("TRACE:", Color::Gray),
//     //     // };

//     //     // let mut iter = msg.split('\n');

//     //     // // Push first line (the iterator always contains one element)
//     //     // self.event_log.push((
//     //     //     format!("{} {}", level_name, iter.next().unwrap()),
//     //     //     Style::default().fg(color),
//     //     // ));

//     //     // // Push all remaining lines with `|` at the start
//     //     // for line in iter {
//     //     //     self.event_log.push((
//     //     //         format!("     | {}", line),
//     //     //         Style::default().fg(color),
//     //     //     ));
//     //     // }
//     // }

//     // fn keymap_string(&self) -> String {
//     //     // Global key map
//     //     let mut keys = vec![
//     //         ('q', "Quit"),
//     //     ];

//     //     if self.selected_tab == DEBUG_TAB {
//     //         keys.extend_from_slice(&[
//     //             ('p', "Pause execution"),
//     //             ('r', "Continue execution"),
//     //         ]);
//     //     }

//     //     let mut out = String::new();
//     //     for (key, description) in keys {
//     //         out.push_str("{bg=red  ");
//     //         out.push(key);
//     //         out.push_str(" } ");
//     //         out.push_str(description);
//     //         out.push_str("    ");
//     //     }

//     //     out
//     // }
// }

// impl Drop for TuiDebugger {
//     fn drop(&mut self) {
//         // Show cursor again
//         print!("{}", termion::cursor::Show);
//         let _ = io::stdout().flush();

//         drop_inner(&self.inner);
//     }
// }

// fn drop_inner(inner: &Mutex<Option<TuiDebuggerInner>>) {
//     // We have to be careful here. We don't want to have a dead lock in the
//     // panic hook or in `drop()`. That would be bad, presumably.
//     match inner.try_lock() {
//         // No one holds the lock right now.
//         Ok(mut guard) => {
//             // We explicitly drop the value to reset the terminal.
//             drop(guard.take());
//         }

//         // The thread holding the lock panicked. This means that
//         // our `inner` can be in a semantically invalid state. We
//         // don't care about that though, so we can access the
//         // value.
//         Err(TryLockError::Poisoned(e)) => {
//             // We explicitly drop the value to reset the terminal.
//             drop(e.into_inner().take());
//         }

//         // In this case, another thread holds the lock and we cannot access the
//         // terminal. So we have to switch to the main screen manually. This
//         // only switches the screen but doesn't reset certain terminal states.
//         // So this is suboptimal.
//         Err(TryLockError::WouldBlock) => {
//             print!("{}", termion::screen::ToMainScreen);
//         }
//     }
//     // We ignore the error here to avoid panicking in a panic hook.
//     let _ = io::stdout().flush();
// }

// impl Debugger for TuiDebugger {
//     fn post_event(&self, level: EventLevel, msg: String) {
//         self.with_inner(|inner| {
//             inner.post_event(level, msg);
//             Ok(())
//         }).expect("couldn't aquire lock to debugger");
//     }
// }
