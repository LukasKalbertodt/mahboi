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
    // views::TextView,
    vec::Vec2,
};

use mahboi::{
    machine::Machine,
    primitives::Word,
};
use super::{
    Breakpoints,
    util::DecodedInstr,
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

            let instr = DecodedInstr::decode(&data);

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
            self.lines.push(Line { current, addr, instr });
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
