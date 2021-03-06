use std::{
    cell::RefCell,
    collections::BTreeSet,
    panic,
    rc::Rc,
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{channel, Receiver, Sender},
    },
};

use cursive::{
    Cursive, CursiveExt,
    theme::{Theme, BorderStyle, Effect, Color, BaseColor, Palette, PaletteColor, Style},
    view::{Boxable, Identifiable, Scrollable},
    views::{
        OnEventView, ListView, ResizedView, EditView, DummyView, Button, TextView,
        LinearLayout, Dialog, ScrollView, NamedView,
    },
    utils::markup::StyledString,
};
use failure::Error;
use lazy_static::lazy_static;
use log::{Log, Record, Level, Metadata};

use mahboi::{
    opcode,
    log::*,
    machine::{
        Machine,
        cpu::Cpu,
        ppu::{Mode, Ppu},
    },
    primitives::{Byte, Word},
};
use crate::{
    args::Args,
};
use super::{Action, WindowBuffer};
use self::{
    asm_view::AsmView,
    log_view::LogView,
    mem_view::MemView,
    tab_view::TabView,
};

mod asm_view;
mod log_view;
mod mem_view;
mod tab_view;
mod util;


// ============================================================================
// ===== Logger
// ============================================================================
// So the logger should show the log messages in the TUI. Sadly, we can't
// directly to the views since log messages could come from all threads. So
// instead, we have a global buffer of log messages. New messages are inserted
// there and the TUI interface regularly checks for new messages and shows them
// in the TUI.

/// The global logger.
static LOGGER: TuiLogger = TuiLogger {
    discard_trace: AtomicBool::new(true),
};

/// Initializes the logger that works in tandem with the TUI debugger.
pub(crate) fn init_logger() {
    log::set_logger(&LOGGER)
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

struct TuiLogger {
    discard_trace: AtomicBool,
}

impl Log for TuiLogger {
    fn enabled(&self, meta: &Metadata) -> bool {
        !(self.discard_trace.load(Ordering::SeqCst) && meta.level() == Level::Trace)
    }

    fn log(&self, record: &Record) {
        let enabled = self.enabled(record.metadata())
            && record.module_path().map(|p| p.starts_with("mahboi")).unwrap_or(false);
        if enabled {
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

// To handle events, we use `Cursive::step`. Sadly, this function
// blocks to wait on an event before it returns. This isn't good. We
// can force the `step()` method to return after one "TUI frame". By
// setting this to 1000, we assure that `step()` waits for at most 1ms.
// Still not perfect, but ok.
const FPS_RUNNING: u32 = 1000;

/// When the debugger is paused, the outer main loop doesn't need to run very
/// often. We can lower the FPS to be nice to the CPU. Remember that TUI inputs
/// interrupt Cursive waiting and can result in a higher FPS than this. This is
/// just for changes in the TUI that are not input triggered.
const FPS_PAUSED: u32 = 2;

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

    /// Flag that is set when the user requested to run until the next RET
    /// instruction.
    pause_on_ret: bool,

    /// This is set whenever the user runs the emulator until a new line or new
    /// frame is reached.
    pause_in_line: Option<u8>,

    /// This is a flag used for "jump to next frame". A value of `true` means
    /// that `pause_in_line` won't trigger. This flag is reset once we reach
    /// V-Blank.
    waiting_for_vblank: bool,

    /// To avoid updating all elements every frame, we track whether an update
    /// is necessary. This flag is set to `true` whenever `should_pause()` is
    /// called and reset whenever all views are updated.
    update_needed: bool,

    /// Was the boot ROM already disabled? This is used to do cache management.
    boot_rom_disabled: bool,

    /// Sometimes the ASM view has to be scrolled to a specific position. This
    /// has to be done after `siv.step()`. That's why its stored here.
    scroll_asm_view: Option<usize>,

    /// A simple counter which counts up every `update()` step. Used to call
    /// `siv.step()` only every Nth time `update()` is called.
    update_counter: u32,
}

impl TuiDebugger {
    pub(crate) fn new(args: &Args) -> Result<Self, Error> {
        // Create a handle to the terminal (with the correct backend).
        let mut siv = Cursive::ncurses()?;

        // To handle events, we use `Cursive::step`. Sadly, this function
        // blocks to wait on an event before it returns. This isn't good. We
        // can force the `step()` method to return after one "TUI frame". By
        // setting this to 1000, we assure that `step()` waits for at most 1ms.
        // Still not perfect, but ok.
        siv.set_fps(FPS_RUNNING);

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
            pause_on_ret: false,
            pause_in_line: None,
            waiting_for_vblank: false,
            boot_rom_disabled: false,
            update_needed: true,
            scroll_asm_view: None,
            update_counter: 0,
        };

        // Add all breakpoints specified by CLI
        for &bp in &args.breakpoints {
            out.breakpoints.add(bp);
        }

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
        mut window: WindowBuffer,
    ) -> Action {
        if !self.siv.is_running() {
            return Action::Quit;
        }

        // Check if the emulator got paused.
        if is_paused && !self.pause_mode {
            // Switch the debugger into pause mode.
            self.pause();
        }

        if self.update_needed {
            // We only update the ASM view if the emulator is paused
            if is_paused {
                let mut asm_view = self.siv.find_name::<AsmView>("asm_view").unwrap();
                asm_view.update(machine);
                let line = asm_view.get_active_line();
                self.scroll_asm_view = Some(line.saturating_sub(10));
            }

            self.update_cpu_data(&machine.cpu);
            self.update_stack_data(machine);
            self.update_ppu_data(&machine.ppu);
            self.update_interrupt_data(machine);

            self.update_needed = false;
        }

        // If we're in pause mode, update elements in the debugging tab
        if is_paused {
            // If the memory dialog is opened, update it
            if let Some(mut mem_view) = self.siv.find_name::<MemView>("mem_view") {
                mem_view.update(machine, self.update_needed);
            }
        }

        // Append all log messages that were pushed to the global buffer into
        // the corresponding log view.
        self.siv.find_name::<LogView>("log_list").unwrap().update();
        let discard = self.siv.find_name::<LogView>("log_list")
            .unwrap()
            .ignore_trace_logs(&mut self.siv)
            && !self.pause_mode;
        LOGGER.discard_trace.store(discard, Ordering::SeqCst);

        // React to any events that might have happend
        while let Ok(c) = self.pending_events.try_recv() {
            match c {
                'p' => {
                    if !self.pause_mode {
                        return Action::Pause;
                    }
                }
                'r' => {
                    if self.pause_mode {
                        // We will continue execution. To make sure we won't
                        // immediately pause again because we paused on a
                        // breakpoint, we set this exception.
                        self.step_over = Some(machine.cpu.pc);
                        self.resume();
                        return Action::Continue;
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
                        return Action::Continue;
                    }
                }
                'f' => {
                    if self.pause_mode {
                        self.step_over = Some(machine.cpu.pc);
                        self.pause_on_ret = true;
                        self.resume();
                        return Action::Continue;
                    }
                }
                'l' => {
                    if self.pause_mode {
                        let next_line = (machine.ppu.regs().current_line.get() + 1) % 144;
                        self.pause_in_line = Some(next_line);
                        self.resume();
                        return Action::Continue;
                    }
                }
                'k' => {
                    if self.pause_mode {
                        self.waiting_for_vblank = true;
                        self.pause_in_line = Some(0);
                        self.resume();
                        return Action::Continue;
                    }
                }
                'c' => {
                    window.paint_pink();
                }
                _ => panic!("internal error: unexpected event"),
            }
        }

        // Receive events and update view.
        self.update_counter += 1;
        if self.update_counter == 4 {
            self.update_counter = 0;
            self.siv.step();
        }

        // Perform certain steps after the TUI has been drawn (re-layouted)
        if let Some(pos) = self.scroll_asm_view {
            self.siv.find_name::<ScrollView<NamedView<AsmView>>>("asm_view_scroll")
                .unwrap()
                .set_offset((0, pos));
            self.scroll_asm_view = None;
        }

        Action::Nothing
    }

    /// Switch to pause mode.
    fn pause(&mut self) {
        debug!("[debugger] enter pause mode");

        self.pause_mode = true;

        LOGGER.discard_trace.store(false, Ordering::SeqCst);

        // Execution just got paused => select the debugging tab
        self.siv.find_name::<TabView>("tab_view")
            .unwrap()
            .set_selected(1);

        // Update the title
        self.siv.find_name::<TextView>("main_title")
            .unwrap()
            .set_content(Self::make_main_title("Mahboi Debugger (paused)"));

        self.siv.set_fps(FPS_PAUSED);
    }

    /// Exit pause mode (continue execution)
    fn resume(&mut self) {
        debug!("[debugger] continue execution (exit pause mode)");

        self.pause_mode = false;

        let discard = self.siv.find_name::<LogView>("log_list")
            .unwrap()
            .ignore_trace_logs(&mut self.siv);
        LOGGER.discard_trace.store(discard, Ordering::SeqCst);

        // Update the title
        self.siv.find_name::<TextView>("main_title")
            .unwrap()
            .set_content(Self::make_main_title("Mahboi Debugger (running)"));

        self.siv.set_fps(FPS_RUNNING);
    }

    pub(crate) fn should_pause(&mut self, machine: &Machine) -> bool {
        // Do internal updating unrelated to determining if the emulator should
        // stop.
        self.update_needed = true;
        if machine.cpu.pc == 0x100 && !self.boot_rom_disabled {
            self.boot_rom_disabled = true;

            // The ASM view caches instructions and assumes the data in
            // 0..0x4000 never changes... which is almost true. But if the boot
            // ROM is disabled, we have to invalidate the cache for 0..0x100.
            self.siv.find_name::<AsmView>("asm_view")
                .unwrap()
                .invalidate_cache(Word::new(0)..Word::new(0x100));

        }

        if let Some(line) = self.pause_in_line {
            // If we are supposed to wait for V-Blank, we just check if we are
            // in V-Blank. Otherwise, we check if we are in the line we want to
            // stop at.
            if self.waiting_for_vblank {
                if machine.ppu.regs().mode() == Mode::VBlank {
                    self.waiting_for_vblank = false;
                }
            } else {
                let stop = machine.ppu.regs().current_line == line
                    && machine.ppu.regs().mode() == Mode::OamSearch;
                if stop {
                    debug!("[debugger] paused in line {}", line);
                    self.pause_in_line = None;
                    return true;
                }
            }
        }

        // If we are at the address we should step over, we will ignore the
        // rest of this method and just *not* pause. But we will also reset the
        // `step_over` value, to pause the next time.
        if let Some(addr) = self.step_over {
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
            debug!("[debugger] paused at breakpoint {}", machine.cpu.pc);
            return true;
        }

        // If we are supposed to pause on a RET instruction...
        if self.pause_on_ret {
            // ... check if the next instruction is an RET-like instruction
            let opcode = machine.load_byte(machine.cpu.pc);
            match opcode.get() {
                opcode!("RET")
                | opcode!("RETI")
                | opcode!("RET NZ")
                | opcode!("RET NC")
                | opcode!("RET Z")
                | opcode!("RET C") => {
                    // Reset the flag
                    self.pause_on_ret = false;
                    return true;
                }
                _ => {}
            }
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
        for &c in &['p', 'r', 's', 'f', 'l', 'k', 'c'] {
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
        let log_tab = LogView::new();

        let main_title = TextView::new(Self::make_main_title("Mahboi Debugger"))
            // .effect(Effect::Bold)
            .center()
            .no_wrap()
            .with_name("main_title");

        let tabs = TabView::new()
            .tab("Event Log", log_tab)
            .tab("Debugger", self.debug_tab())
            .with_name("tab_view");

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

    fn update_stack_data(&mut self, machine: &Machine) {
        let mut body = StyledString::new();

        let start = machine.cpu.sp.get();
        let end = start.saturating_add(20);

        for addr in start..end {
            let addr = Word::new(addr);
            body.append_styled(addr.to_string(), Color::Light(BaseColor::Blue));
            body.append_styled(" │   ", Color::Light(BaseColor::Blue));
            body.append_styled(
                machine.load_byte(addr).to_string(),
                Color::Dark(BaseColor::Yellow),
            );

            if addr == start {
                body.append_plain("   ← SP");
            }

            body.append_plain("\n");
        }

        self.siv.find_name::<TextView>("stack_view").unwrap().set_content(body);
    }

    fn update_ppu_data(&mut self, ppu: &Ppu) {
        // TODO:
        // - FF40 bit 0
        // - FF41 bit 2-6

        let reg_style = Color::Light(BaseColor::Magenta);

        let mut body = StyledString::new();

        let regs = ppu.regs();

        // Phase/Status
        if regs.is_lcd_enabled() {
            body.append_plain("==> Phase: ");
            body.append_plain(regs.mode().to_string());
            body.append_plain("\n");
        } else {
            body.append_plain("  --- LCD disabled! ---\n");
        }
        body.append_plain("\n");

        // Tile data memory range for BG and window
        body.append_plain("BG tile data: ");
        body.append_styled(regs.bg_window_tile_data_address().to_string(), reg_style);
        body.append_plain("\n");

        // FF44 current line
        body.append_plain("Current line: ");
        body.append_styled(regs.current_line.get().to_string(), reg_style);
        body.append_plain("\n");

        // FF45 line compare
        body.append_plain("Line compare: ");
        body.append_styled(regs.lyc.get().to_string(), reg_style);
        body.append_plain("\n");

        body.append_plain("\n");


        // ===== Palette information =====
        fn format_palette(b: Byte) -> String {
            let b = b.get();

            format!(
                "{:02b} {:02b} {:02b} {:02b}\n",
                (b >> 6) & 0b11,
                (b >> 4) & 0b11,
                (b >> 2) & 0b11,
                (b >> 0) & 0b11,
            )
        }
        body.append_plain("## Palettes: \n");

        body.append_plain("- BG: ");
        body.append_styled(format_palette(regs.background_palette), reg_style);
        body.append_plain("- S0: ");
        body.append_styled(format_palette(regs.sprite_palette_0), reg_style);
        body.append_plain("- S1: ");
        body.append_styled(format_palette(regs.sprite_palette_1), reg_style);

        body.append_plain("\n");


        // ===== Background information =====
        body.append_plain("## Background: \n");

        // Tile map memory region
        body.append_plain("- tile map: ");
        body.append_styled(regs.bg_tile_map_address().to_string(), reg_style);
        body.append_plain("\n");

        // Scroll
        body.append_plain("- X: ");
        body.append_styled(format!("{: >3}", regs.scroll_bg_x.get()), reg_style);
        body.append_plain(",  Y: ");
        body.append_styled(format!("{: >3}", regs.scroll_bg_y.get()), reg_style);
        body.append_plain("\n");

        body.append_plain("\n");


        // ===== Window information =====
        body.append_plain("## Window: \n");

        // Enabled?
        body.append_plain("- ");
        let win_status = if regs.is_window_enabled() { "enabled" } else { "disabled" };
        body.append_styled(win_status, reg_style);
        body.append_plain("\n");

        // Tile map memory region
        body.append_plain("- tile map: ");
        body.append_styled(regs.window_tile_map_address().to_string(), reg_style);
        body.append_plain("\n");

        // Scroll position
        body.append_plain("- X: ");
        body.append_styled(format!("{: >3}", regs.scroll_win_x.get()), reg_style);
        body.append_plain(",  Y: ");
        body.append_styled(format!("{: >3}", regs.scroll_win_y.get()), reg_style);
        body.append_plain("\n");

        body.append_plain("\n");


        // ===== Sprite information =====
        body.append_plain("## Sprites: \n");

        // Enabled?
        body.append_plain("- ");
        let sprite_status = if regs.are_sprites_enabled() { "enabled" } else { "disabled" };
        body.append_styled(sprite_status, reg_style);
        body.append_plain("\n");

        // Size
        body.append_plain("- Size: ");
        let sprite_size = if regs.large_sprites_enabled() { "8x16" } else { "8x8" };
        body.append_styled(sprite_size, reg_style);
        body.append_plain("\n");


        self.siv.find_name::<TextView>("ppu_data").unwrap().set_content(body);
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
        body.append_styled((cpu.subtract() as u8).to_string(), reg_style);
        body.append_plain("  H: ");
        body.append_styled((cpu.half_carry() as u8).to_string(), reg_style);
        body.append_plain("  C: ");
        body.append_styled((cpu.carry() as u8).to_string(), reg_style);

        self.siv.find_name::<TextView>("cpu_data").unwrap().set_content(body);
    }

    fn update_interrupt_data(&mut self, machine: &Machine) {
        let reg_style = Color::Light(BaseColor::Magenta);

        let mut body = StyledString::new();
        let ints = machine.interrupt_controller();

        // IME
        body.append_plain("IME: ");
        body.append_styled((ints.ime as u8).to_string(), reg_style);
        body.append_plain("\n");
        body.append_plain("\n");


        // IF and IE
        fn bit_string(byte: u8) -> String {
            format!(
                "{}  {}  {}  {}  {}",
                (byte >> 4) & 0b1,
                (byte >> 3) & 0b1,
                (byte >> 2) & 0b1,
                (byte >> 1) & 0b1,
                (byte >> 0) & 0b1,
            )
        }

        body.append_plain("      J  S  T  L  V\n");

        body.append_plain("IE:   ");
        body.append_styled(bit_string(ints.interrupt_enable.get()), reg_style);
        body.append_plain("\n");

        body.append_plain("IF:   ");
        body.append_styled(bit_string(ints.interrupt_flag.get()), reg_style);
        body.append_plain("\n");
        body.append_plain("\n");


        // LCD stat interrupts
        body.append_plain("            =  O  V  H\n");
        body.append_plain("LCD stat:   ");
        body.append_styled(
            (machine.ppu.regs().coincidence_interrupt() as u8).to_string(),
            reg_style,
        );
        body.append_plain("  ");
        body.append_styled(
            (machine.ppu.regs().oam_search_interrupt() as u8).to_string(),
            reg_style,
        );
        body.append_plain("  ");
        body.append_styled(
            (machine.ppu.regs().vblank_interrupt() as u8).to_string(),
            reg_style,
        );
        body.append_plain("  ");
        body.append_styled(
            (machine.ppu.regs().hblank_interrupt() as u8).to_string(),
            reg_style,
        );
        body.append_plain("\n");


        self.siv.find_name::<TextView>("interrupt_view").unwrap().set_content(body);
    }

    /// Create the body of the debugging tab.
    fn debug_tab(&self) -> OnEventView<ResizedView<LinearLayout>> {
        // Main body (left)
        let asm_view = AsmView::new(self.breakpoints.clone())
            .with_name("asm_view")
            .scrollable()
            .with_name("asm_view_scroll");

        // First right column
        let cpu_body = TextView::new("no data yet").center().with_name("cpu_data");
        let cpu_view = Dialog::around(cpu_body).title("CPU registers");

        let stack_body = TextView::new("no data yet")
            .with_name("stack_view")
            .scrollable()
            .fixed_height(8);
        let stack_view = Dialog::around(stack_body).title("Stack");

        let interrupt_body = TextView::new("no data yet")
            .with_name("interrupt_view");
        let interrupt_view = Dialog::around(interrupt_body).title("Interrupts");

        let first_right_panel = LinearLayout::vertical()
            .child(cpu_view)
            .child(DummyView)
            .child(stack_view)
            .child(DummyView)
            .child(interrupt_view)
            .fixed_width(30);

        // Second right column
        let ppu_body = TextView::new("not implemented yet").with_name("ppu_data");
        let ppu_view = Dialog::around(ppu_body).title("PPU");

        // Setup Buttons
        let button_breakpoints = {
            let breakpoints = self.breakpoints.clone(); // clone for closure
            Button::new("Manage Breakpoints [b]", move |s| {
                Self::open_breakpoints_dialog(s, &breakpoints)
            })
        };

        let mem_button = Button::new("View memory [m]", |s| {
            Self::open_memory_dialog(s)
        });

        // Buttons for the 'r', 's' and 'f' actions
        let tx = self.event_sink.clone();
        let run_button = Button::new("Continue [r]", move |_| tx.send('r').unwrap());
        let tx = self.event_sink.clone();
        let step_button = Button::new("Single step [s]", move |_| tx.send('s').unwrap());
        let tx = self.event_sink.clone();
        let fun_end_button = Button::new("Run to RET-like [f]", move |_| tx.send('f').unwrap());
        let tx = self.event_sink.clone();
        let line_button = Button::new("Run to next line [l]", move |_| tx.send('l').unwrap());
        let tx = self.event_sink.clone();
        let frame_button = Button::new("Run to next frame [k]", move |_| tx.send('k').unwrap());

        // Wrap all buttons
        let debug_buttons = LinearLayout::vertical()
            .child(button_breakpoints)
            .child(mem_button)
            .child(run_button)
            .child(step_button)
            .child(fun_end_button)
            .child(line_button)
            .child(frame_button);
        let debug_buttons = Dialog::around(debug_buttons).title("Actions");

        // Build the complete right side
        let second_right_panel = LinearLayout::vertical()
            .child(ppu_view)
            .child(DummyView)
            .child(debug_buttons)
            .fixed_width(30);

        // Combine
        let view = LinearLayout::horizontal()
            .child(asm_view)
            .child(first_right_panel)
            .child(DummyView)
            .child(second_right_panel)
            .full_screen();

        // Add shortcuts for debug tab
        let breakpoints = self.breakpoints.clone();
        OnEventView::new(view)
            .on_event('b', move |s| Self::open_breakpoints_dialog(s, &breakpoints))
            .on_event('m', |s| Self::open_memory_dialog(s))
    }

    /// Gets executed when the "Manage breakpoints" action button is pressed.
    fn open_breakpoints_dialog(siv: &mut Cursive, breakpoints: &Breakpoints) {
        // Setup list showing all breakpoints
        let bp_list = Self::create_breakpoint_list(breakpoints)
            .with_name("breakpoint_list");

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
                        s.call_on_name("breakpoint_list", |list: &mut ListView| {
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
                s.call_on_name("breakpoint_list", |list: &mut ListView| {
                    *list = Self::create_breakpoint_list(&breakpoints);
                });
            });

            out.add_child(&bp.to_string(), remove_button);
        }

        out
    }

    /// Gets executed when the "View memory" action button is pressed.
    fn open_memory_dialog(siv: &mut Cursive) {
        let jump_to_edit = EditView::new()
            .max_content_width(4)
            .on_submit(move |s, input| {
                // Try to parse the input as hex value
                match u16::from_str_radix(&input, 16) {
                    Ok(addr) => {
                        // Set cursor
                        let mut mem_view = s.find_name::<MemView>("mem_view").unwrap();
                        mem_view.cursor = Word::new(addr);
                    },
                    Err(e) => {
                        let msg = format!("invalid addr: {}", e);
                        s.add_layer(Dialog::info(msg));
                    }
                }
            })
            .fixed_width(7);

        let jump_to = LinearLayout::horizontal()
            .child(TextView::new("Jump to:  "))
            .child(jump_to_edit);

        let mem_view = MemView::new()
            .with_name("mem_view");

        // Combine all elements
        let body = LinearLayout::vertical()
            .child(mem_view)
            .child(DummyView)
            .child(jump_to);

        // Put into `Dialog` and show dialog
        let dialog = Dialog::around(body)
            .title("Memory")
            .button("Ok", |s| { s.pop_layer(); });

        siv.add_layer(dialog);
    }
}


/// A collection of breakpoints.
///
/// This type uses reference counted pointer and interior mutability to be
/// easily usable from everywhere. Just `clone()` this to get another owned
/// reference.
#[derive(Clone)]
pub(crate) struct Breakpoints(Rc<RefCell<BTreeSet<Word>>>);

impl Breakpoints {
    fn new() -> Self {
        Breakpoints(Rc::new(RefCell::new(BTreeSet::new())))
    }

    /// Add a breakpoint to the collection. If it's already inside, nothing
    /// happens.
    pub(crate) fn add(&self, addr: Word) {
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
