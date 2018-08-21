use std::{
    cell::RefCell,
    collections::BTreeSet,
    panic,
    rc::Rc,
    sync::{
        Mutex,
        mpsc::{channel, Receiver, Sender},
    },
};

use cursive::{
    Cursive,
    theme::{Theme, BorderStyle, Effect, Color, BaseColor, Palette, PaletteColor, Style},
    view::{Boxable, Identifiable},
    views::{
        OnEventView, ListView, BoxView, EditView, DummyView, Button, TextView,
        LinearLayout, Dialog
    },
    utils::markup::StyledString,
};
use failure::Error;
use lazy_static::lazy_static;
use log::{Log, Record, Level, Metadata};

use mahboi::{
    log::*,
    machine::{Cpu, Machine},
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


// ============================================================================
// ===== Debugger
// ============================================================================

/// A debugger that uses a terminal user interface. Used in `--debug` mode.
pub(crate) struct TuiDebugger {
    /// Handle to the special TUI terminal
    siv: Cursive,

    /// Is the emulator in pause mode?
    ///
    /// In pause mode, we (pretty much) always return `true` from
    /// `should_pause` to always also pause the emulator. There are many events
    /// which set the debugger into pause mode, but it's not happening
    /// directly. Instead, pausing happens by returning `true` from
    /// `should_pause` or `Action::Pause` from `update()`. We don't immediately
    /// switch into pause mode, but wait for the main loop/the emulator to
    /// switch state. Once `update()` gets called with `is_paused = true`, we
    /// switch into pause mode as well.
    ///
    /// The other way around works differently: the emulator can't tell us to
    /// exit pause mode. Instead, we exit pause mode ourselves on certain
    /// events.
    pause_mode: bool,

    // ===== Asynchronous event handling ======================================
    /// Events that cannot be handled immediately and are stored here to be
    /// handled in `update`.
    pending_events: Receiver<char>,

    /// A clonable sender for events to be handled in `update()`. This is just
    /// passed to Cursive event handlers.
    event_sink: Sender<char>,

    // ===== Data to control when to stop execution ===========================
    /// This is an exception to the normal pause-rules. If this is
    /// `Some(addr)`, we will not pause execution for an instruction at `addr`.
    /// It's reset to `None` after this exception "has been used".
    step_over: Option<Word>,

    /// A set of addresses at which we will pause execution
    breakpoints: Breakpoints,
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

        let (event_sink, pending_events) = channel();

        let mut out = Self {
            siv,
            pause_mode: false,
            pending_events,
            event_sink,
            step_over: None,
            breakpoints: Breakpoints::new(),
        };

        // Build the TUI view
        out.setup_tui();

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

        // Check if the emulator got paused.
        if is_paused && !self.pause_mode {
            // Switch the debugger into pause mode.
            self.pause();
        }

        // If we're in pause mode, update elements in the debugging tab
        if is_paused {
            self.siv.find_id::<AsmView>("asm_view").unwrap().update(machine);
            self.update_cpu_data(&machine.cpu);
        }

        // Append all log messages that were pushed to the global buffer into
        // the corresponding log view.
        self.siv.call_on_id("log_list", |list: &mut LogView| {
            for log in LOG_MESSAGES.lock().unwrap().drain(..) {
                list.add_row(log.level, log.msg);
            }
        });

        // React to any events that might have happend
        while let Ok(c) = self.pending_events.try_recv() {
            match c {
                'p' => {
                    if !self.pause_mode {
                        return Ok(Action::Pause);
                    }
                }
                'r' => {
                    if self.pause_mode {
                        // We will continue execution. To make sure we won't
                        // immediately pause again because we paused on a
                        // breakpoint, we set this exception.
                        self.step_over = Some(machine.cpu.pc);
                        self.resume();
                        return Ok(Action::Continue);
                    }
                }
                's' => {
                    if self.pause_mode {
                        // We tell the emulator to continue execution, while we
                        // stay in pause mode. This would mean that we would
                        // return `true` from `should_pause` right away. To
                        // avoid that, we also set the `step_over` exception to
                        // exectute one instruction.
                        self.step_over = Some(machine.cpu.pc);
                        return Ok(Action::Continue);
                    }
                }
                _ => panic!("internal error: unexpected event"),
            }
        }

        // Receive events and update view.
        self.siv.step();

        Ok(Action::Nothing)
    }

    /// Switch to pause mode.
    fn pause(&mut self) {
        trace!("[debugger] enter pause mode");

        self.pause_mode = true;

        // Execution just got paused => select the debugging tab
        self.siv.find_id::<TabView>("tab_view")
            .unwrap()
            .set_selected(1);

        // Update the title
        self.siv.find_id::<TextView>("main_title")
            .unwrap()
            .set_content(Self::make_main_title("Mahboi Debugger (paused)"));
    }

    /// Exit pause mode (continue execution)
    fn resume(&mut self) {
        trace!("[debugger] continue execution (exit pause mode)");

        self.pause_mode = false;

        // Update the title
        self.siv.find_id::<TextView>("main_title")
            .unwrap()
            .set_content(Self::make_main_title("Mahboi Debugger (running)"));
    }

    pub(crate) fn should_pause(&mut self, machine: &Machine) -> bool {
        if let Some(addr) = self.step_over {
            // If we are at the address we should step over, we will ignore the
            // rest of this method and just *not* pause. But we will also reset
            // the `step_over` value, to pause the next time.
            if addr == machine.cpu.pc {
                self.step_over = None;
                return false;
            }
        }

        // If we're in paused mode, the emulator should always pause.
        if self.pause_mode {
            return true;
        }

        // We the current instruction is one of our breakpoints, we also pause.
        if self.breakpoints.contains(machine.cpu.pc) {
            trace!("[debugger] paused at breakpoint {}", machine.cpu.pc);
            return true;
        }

        false
    }

    /// Prepare s the `Cursive` instance by registering event handler and
    /// setting up the view.
    fn setup_tui(&mut self) {
        // We always want to be able to quit the application via `q`.
        self.siv.add_global_callback('q', |s| s.quit());

        // Other global events are just forwarded to be handled in the next
        // `update()` call.
        for &c in &['p', 'r', 's'] {
            let tx = self.event_sink.clone();
            self.siv.add_global_callback(c, move |_| tx.send(c).unwrap());
        }

        // Create and set our theme.
        let mut palette = Palette::default();
        palette[PaletteColor::View] = Color::TerminalDefault;
        palette[PaletteColor::Primary] = Color::TerminalDefault;
        palette[PaletteColor::Secondary] = Color::TerminalDefault;
        palette[PaletteColor::Tertiary] = Color::TerminalDefault;
        palette[PaletteColor::TitlePrimary] = Color::Light(BaseColor::Green);
        palette[PaletteColor::TitleSecondary] = Color::TerminalDefault;
        palette[PaletteColor::Highlight] = Color::Dark(BaseColor::Red);
        palette[PaletteColor::HighlightInactive] = Color::TerminalDefault;
        let theme = Theme {
            shadow: false,
            borders: BorderStyle::Simple,
            palette,
        };
        self.siv.set_theme(theme);

        // Create view for log messages
        let log_list = LogView::new().with_id("log_list");


        let main_title = TextView::new(Self::make_main_title("Mahboi Debugger"))
            // .effect(Effect::Bold)
            .center()
            .no_wrap()
            .with_id("main_title");

        let tabs = TabView::new()
            .tab("Event Log", log_list)
            .tab("Debugger", self.debug_tab())
            .with_id("tab_view");

        let main_layout = LinearLayout::vertical()
            .child(main_title)
            .child(tabs);

        self.siv.add_fullscreen_layer(main_layout);
    }

    fn make_main_title(title: &str) -> StyledString {
        StyledString::styled(
            title,
            Style::from(Color::Dark(BaseColor::Green)).combine(Effect::Bold),
        )
    }

    fn update_cpu_data(&mut self, cpu: &Cpu) {
        let reg_style = Color::Light(BaseColor::Magenta);

        let mut body = StyledString::new();

        // A F
        body.append_plain("A: ");
        body.append_styled(cpu.a.to_string(), reg_style);
        body.append_plain("    ");
        body.append_plain("F: ");
        body.append_styled(cpu.f.to_string(), reg_style);

        // B C
        body.append_plain("\n");
        body.append_plain("B: ");
        body.append_styled(cpu.b.to_string(), reg_style);
        body.append_plain("    ");
        body.append_plain("C: ");
        body.append_styled(cpu.c.to_string(), reg_style);

        // D E
        body.append_plain("\n");
        body.append_plain("D: ");
        body.append_styled(cpu.d.to_string(), reg_style);
        body.append_plain("    ");
        body.append_plain("E: ");
        body.append_styled(cpu.e.to_string(), reg_style);

        // H L
        body.append_plain("\n");
        body.append_plain("H: ");
        body.append_styled(cpu.h.to_string(), reg_style);
        body.append_plain("    ");
        body.append_plain("L: ");
        body.append_styled(cpu.l.to_string(), reg_style);

        // SP and PC
        body.append_plain("\n\n");
        body.append_plain("SP: ");
        body.append_styled(cpu.sp.to_string(), reg_style);
        body.append_plain("\n");
        body.append_plain("PC: ");
        body.append_styled(cpu.pc.to_string(), reg_style);

        // The four flags from the F registers in nicer
        body.append_plain("\n\n");
        body.append_plain("Z: ");
        body.append_styled((cpu.zero() as u8).to_string(), reg_style);
        body.append_plain("  N: ");
        body.append_styled((cpu.substract() as u8).to_string(), reg_style);
        body.append_plain("  H: ");
        body.append_styled((cpu.half_carry() as u8).to_string(), reg_style);
        body.append_plain("  C: ");
        body.append_styled((cpu.carry() as u8).to_string(), reg_style);

        self.siv.find_id::<TextView>("cpu_data").unwrap().set_content(body);
    }

    /// Create the body of the debugging tab.
    fn debug_tab(&self) -> OnEventView<BoxView<LinearLayout>> {
        // Main body (left)
        let asm_view = AsmView::new()
            .with_id("asm_view")
            .full_screen();

        // Right panel
        let cpu_body = TextView::new("no data yet").center().with_id("cpu_data");
        let cpu_view = Dialog::around(cpu_body).title("CPU registers");


        // Setup Buttons
        let button_breakpoints = {
            let breakpoints = self.breakpoints.clone(); // clone for closure
            Button::new("Manage Breakpoints [b]", move |s| {
                Self::open_breakpoints_dialog(s, &breakpoints)
            })
        };

        // Buttons for the 'r' and 's' actions
        let tx = self.event_sink.clone();
        let run_button = Button::new("Continue [r]", move |_| tx.send('r').unwrap());
        let tx = self.event_sink.clone();
        let step_button = Button::new("Single step [s]", move |_| tx.send('s').unwrap());

        // Wrap all buttons
        let debug_buttons = LinearLayout::vertical()
            .child(button_breakpoints)
            .child(run_button)
            .child(step_button);
        let debug_buttons = Dialog::around(debug_buttons).title("Actions");

        // Build the complete right side
        let right_panel = LinearLayout::vertical()
            .child(cpu_view)
            .child(DummyView)
            .child(debug_buttons)
            .fixed_width(30);

        // Combine
        let view = LinearLayout::horizontal()
            .child(asm_view).weight(5)
            .child(right_panel).weight(1)
            .full_screen();

        // Add shortcuts for debug tab
        let breakpoints = self.breakpoints.clone();
        OnEventView::new(view)
            .on_event('b', move |s| Self::open_breakpoints_dialog(s, &breakpoints))
    }

    /// Gets executed when the "Manage breakpoints" action button is pressed.
    fn open_breakpoints_dialog(siv: &mut Cursive, breakpoints: &Breakpoints) {
        // Setup list showing all breakpoints
        let bp_list = Self::create_breakpoint_list(breakpoints)
            .with_id("breakpoint_list");

        // Setup the field to add a breakpoint
        let breakpoints = breakpoints.clone(); // clone for closure
        let add_breakpoint_edit = EditView::new()
            .max_content_width(4)
            .on_submit(move |s, input| {
                // Try to parse the input as hex value
                match u16::from_str_radix(&input, 16) {
                    Ok(addr) => {
                        // Add it to the breakpoints collection and update the
                        // list view.
                        breakpoints.add(Word::new(addr));
                        s.call_on_id("breakpoint_list", |list: &mut ListView| {
                            *list = Self::create_breakpoint_list(&breakpoints);
                        });
                    },
                    Err(e) => {
                        let msg = format!("invalid addr: {}", e);
                        s.add_layer(Dialog::info(msg));
                    }
                }
            })
            .fixed_width(7);

        let add_breakpoint = LinearLayout::horizontal()
            .child(TextView::new("Add breakpoint:  "))
            .child(add_breakpoint_edit);


        // Combine all elements
        let body = LinearLayout::vertical()
            .child(bp_list)
            .child(DummyView)
            .child(add_breakpoint);

        // Put into `Dialog` and show dialog
        let dialog = Dialog::around(body)
            .title("Breakpoints")
            .button("Ok", |s| { s.pop_layer(); });

        siv.add_layer(dialog);
    }

    /// Creates a list of all breakpoints in the given collection. For each
    /// breakpoint, there is a button to remove the breakpoint. This function
    /// assumes that the returned view is added to the Cursive instance with
    /// the id "breakpoint_list"!
    fn create_breakpoint_list(breakpoints: &Breakpoints) -> ListView {
        let mut out = ListView::new();

        for bp in breakpoints.as_sorted_list() {
            let breakpoints = breakpoints.clone();
            let remove_button = Button::new("Remove", move |s| {
                breakpoints.remove(bp);
                s.call_on_id("breakpoint_list", |list: &mut ListView| {
                    *list = Self::create_breakpoint_list(&breakpoints);
                });
            });

            out.add_child(&bp.to_string(), remove_button);
        }

        out
    }
}


/// A collection of breakpoints.
///
/// This type uses reference counted pointer and interior mutability to be
/// easily usable from everywhere. Just `clone()` this to get another owned
/// reference.
#[derive(Clone)]
struct Breakpoints(Rc<RefCell<BTreeSet<Word>>>);

impl Breakpoints {
    fn new() -> Self {
        Breakpoints(Rc::new(RefCell::new(BTreeSet::new())))
    }

    /// Add a breakpoint to the collection. If it's already inside, nothing
    /// happens.
    fn add(&self, addr: Word) {
        self.0.borrow_mut().insert(addr);
    }

    /// Remove a breakpoint. If it's not present in the collection, nothing
    /// happens.
    fn remove(&self, addr: Word) {
        self.0.borrow_mut().remove(&addr);
    }

    fn contains(&self, addr: Word) -> bool {
        self.0.borrow().contains(&addr)
    }

    fn as_sorted_list(&self) -> Vec<Word> {
        self.0.borrow().iter().cloned().collect()
    }
}
