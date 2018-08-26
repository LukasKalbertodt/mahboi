use std::{
    cmp,
    collections::BTreeMap,
    ops::Range,
};

use cursive::{
    Printer,
    direction::Direction,
    event::{AnyCb, Event, EventResult},
    theme::{Color, BaseColor},
    view::{View, Selector},
    // views::TextView,
    vec::Vec2,
};

use mahboi::{
    log::*,
    machine::Machine,
    primitives::Word,
};
use super::{
    util::DecodedInstr,
};

/// How many bytes around PC should be showed in the view?
const CONTEXT_SIZE: u16 = 50;

/// When `update()` is called, how many instructions should be added to the
/// cache (starting from the current instruction).
const CACHE_LOOKAHEAD: u16 = 100;

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
    boot_rom_disabled: bool,
}

impl AsmView {
    /// Creates an empty AsmView.
    pub fn new() -> Self {
        Self {
            lines: vec![],
            instr_cache: BTreeMap::new(),
            pc: Word::new(0),
            boot_rom_disabled: false,
        }
    }

    fn invalidate_cache(&mut self, range: Range<Word>) {
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
        // Special case: check the boot ROM will be disabled. TODO: this is
        // actually not good because `update()` is probably not called with pc
        // == 100
        if machine.cpu.pc == 0x100 && !self.boot_rom_disabled {
            self.invalidate_cache(Word::new(0)..Word::new(0x100));
        }

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
        debug!("range in update: {:?}", curr_range);
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
            let addr_offset = 5;

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

    fn on_event(&mut self, _: Event) -> EventResult {
        // TODO
        EventResult::Ignored
    }

    fn take_focus(&mut self, _: Direction) -> bool {
        true
    }

    fn call_on_any<'a>(&mut self, _selector: &Selector, _cb: AnyCb<'a>) {
        // TODO
    }
}
