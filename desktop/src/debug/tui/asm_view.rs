use std::{
    cmp,
    collections::BTreeMap,
    ops::Range,
};

use cursive::{
    Printer,
    direction::Direction,
    event::{AnyCb, Event, MouseButton, EventResult, MouseEvent},
    theme::{Color, BaseColor},
    view::{View, Selector},
    vec::Vec2,
};

use mahboi::{
    opcode,
    instr::Instr,
    machine::Machine,
    primitives::Word,
};
use super::{
    Breakpoints,
    util::{DecodedInstr, InstrArg},
};

/// How many bytes around PC should be showed in the view?
const CONTEXT_SIZE: u16 = 100;

/// When `update()` is called, how many instructions should be added to the
/// cache (starting from the current instruction).
const CACHE_LOOKAHEAD: u16 = 200;

#[derive(Clone, Debug)]
struct Line {
    current: bool,
    addr: Word,
    instr: DecodedInstr,
    comment: String,
}

pub struct AsmView {
    lines: Vec<Line>,
    instr_cache: BTreeMap<Word, DecodedInstr>,
    pc: Word,
    breakpoints: Breakpoints,
}

impl AsmView {
    /// Creates an empty AsmView.
    pub(crate) fn new(breakpoints: Breakpoints) -> Self {
        Self {
            lines: vec![],
            instr_cache: BTreeMap::new(),
            pc: Word::new(0),
            breakpoints,
        }
    }

    pub(crate) fn invalidate_cache(&mut self, range: Range<Word>) {
        let keys = self.instr_cache.range(range)
            .map(|(addr, _)| *addr)
            .collect::<Vec<_>>();

        for key in keys {
            self.instr_cache.remove(&key);
        }
    }

    fn start_of_instr_at(&self, addr: Word) -> Option<Word> {
        self.instr_cache
            .range(addr..addr + 3u8)
            .next()
            .map(|(addr, _)| *addr)
    }

    pub fn update(&mut self, machine: &Machine) {
        self.pc = machine.cpu.pc;

        // Add new instructions to cache
        let mut pos = machine.cpu.pc;
        for _ in 0..CACHE_LOOKAHEAD {
            let data = [
                machine.load_byte(pos),
                machine.load_byte(pos + 1u8),
                machine.load_byte(pos + 2u8),
            ];

            // We can unwrap: `data` is always long enough
            let instr = DecodedInstr::decode(&data).unwrap();

            // If we encounter an unencodable instruction, we stop.
            if instr.is_unknown() {
                break;
            }

            let addr = pos;
            pos += instr.len();

            self.instr_cache.insert(addr, instr);
        }

        // Construct the lines we want to show.
        self.lines.clear();
        let curr_range = self.get_current_range();
        let mut addr = curr_range.start;
        while addr < curr_range.end {
            // Print arrow to show where we are
            let current = self.pc == addr;

            let instr = self.instr_cache.get(&addr)
                .cloned()
                .unwrap_or(DecodedInstr::Unknown(machine.load_byte(addr)));

            let instr_len = instr.len();

            let line = Line {
                current,
                addr,
                comment: comment_for(&instr, addr),
                instr,
            };
            self.lines.push(line);

            addr += instr_len;
        }
    }

    pub(crate) fn get_active_line(&self) -> usize {
        self.lines.iter()
            .position(|l| l.current)
            .expect("internal asm_view error: no line is current")
    }

    fn get_current_range(&self) -> Range<Word> {
        // Determine the bounds in which we show instructions. The start
        // position is a bit tricky. It might be the case that it shows into
        // the middle of an cached instruction. If that's the case, we slightly
        // adjust the start value.
        let start = self.pc.map(|w| w.saturating_sub(CONTEXT_SIZE));
        let start = self.start_of_instr_at(start).unwrap_or(start);
        let end = self.pc.map(|w| w.saturating_add(CONTEXT_SIZE));

        start..end
    }
}

impl View for AsmView {
    fn draw(&self, printer: &Printer) {
        for (i, line) in self.lines.iter().enumerate() {
            // Print arrow to show where we are
            if line.current {
                printer.print((0, i), "PC ➤ ");
            }
            let breakpoint_offset = 5;

            if self.breakpoints.contains(line.addr) {
                printer.with_style(Color::Light(BaseColor::Red), |printer| {
                    printer.print((breakpoint_offset, i), "⯃ ");
                });
            } else {
                printer.print((breakpoint_offset, i), "  ");
            }
            let addr_offset = breakpoint_offset + 2;

            // Print address
            printer.with_style(Color::Light(BaseColor::Blue), |printer| {
                printer.print((addr_offset, i), &format!("{} │   ", line.addr));
            });
            let instr_offset = addr_offset + 11;

            // Print instruction
            line.instr.print(&printer.offset((instr_offset, i)));
            let comment_offset = instr_offset + 28;

            // If we have a comment, print it
            if !line.comment.is_empty() {
                printer.with_style(Color::Light(BaseColor::Black), |printer| {
                    printer.print((comment_offset, i), ";");
                    printer.print((comment_offset + 2, i), &line.comment);
                });
            }
        }
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        let width = cmp::max(constraint.x, 40);
        Vec2::new(width, self.lines.len())
    }

    fn on_event(&mut self, event: Event) -> EventResult {
        match event {
            Event::Mouse {
                event: MouseEvent::Press(MouseButton::Left),
                position,
                offset,
            } => {
                // If the click was over our view
                if let Some(rel_pos) = position.checked_sub(offset) {
                    // If the left side of the line was clicked
                    if rel_pos.x < 14 {
                        let addr = self.lines[rel_pos.y].addr;
                        if self.breakpoints.contains(addr) {
                            self.breakpoints.remove(addr);
                        } else {
                            self.breakpoints.add(addr);
                        }
                        return EventResult::Consumed(None);
                    }
                }
            }

            // All other events are ignored
            _ => {}
        }

        EventResult::Ignored
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        true
    }

    fn call_on_any<'a>(&mut self, _selector: &Selector, _cb: AnyCb<'a>) {
        // TODO
    }
}

/// Creates a comment string for the given instruction.
///
/// The comment can hold any potentially useful informtion.
fn comment_for(instr: &DecodedInstr, addr: Word) -> String {
    fn comment_sep(s: &mut String) {
        if !s.is_empty() {
            *s += ", ";
        }
    }

    fn comment_for_arg(s: &mut String, arg: &InstrArg) {
        if let InstrArg::Dyn { raw, label, .. } = arg {
            let addr = match *label {
                "(a8)" => Word::new(0xFF00) + raw[0],
                "(a16)" | "d16" => Word::from_bytes(raw[0], raw[1]),
                _ => return,
            };

            let comment = match addr.get() {
                0xFF00 => "input",
                0xFF01 => "serial transfer data",
                0xFF02 => "serial transfer control",
                0xFF04..=0xFF07 => "some timer register", // TODO
                0xFF0F => "IF interrupt flag",
                0xFF10..=0xFF3F => "probably some sound register", // TODO
                0xFF40 => "LCD control",
                0xFF41 => "LCD status",
                0xFF42 => "bg scroll y",
                0xFF43 => "bg scroll x",
                0xFF44 => "LY (current line)",
                0xFF45 => "LYC (line compare)",
                0xFF46 => "OAM DMA",
                0xFF47 => "background palette",
                0xFF48 => "sprite0 palette",
                0xFF49 => "sprite1 palette",
                0xFF4A => "window scroll y",
                0xFF4B => "window scroll x",
                0xFFFF => "IE interrupt enable",
                _ => "",
            };

            comment_sep(s);
            *s += comment;
        }
    }

    let mut out = String::new();
    match instr {
        DecodedInstr::OneArg { arg, .. } => comment_for_arg(&mut out, arg),
        DecodedInstr::TwoArgs { arg0, arg1, .. } => {
            comment_for_arg(&mut out, arg0);
            comment_for_arg(&mut out, arg1);
        }
        _ => {}
    };

    if let Some(Instr { opcode, .. }) = instr.instr() {
        match opcode.get() {
            // Show destination address
            opcode!("JR r8")
            | opcode!("JR NZ, r8")
            | opcode!("JR NC, r8")
            | opcode!("JR Z, r8")
            | opcode!("JR C, r8") if !instr.prefixed() => {
                let arg = if opcode.get() == opcode!("JR r8") {
                    instr.arg0().unwrap()
                } else {
                    instr.arg1().unwrap()
                };
                let raw = arg.raw_data().unwrap();
                let r8 = raw[0].get() as i8;

                let dst = addr + r8 + 2u8;
                out.push_str(&format!("jumps to {}", dst));
            }

            _ => {}
        }
    }

    out
}
