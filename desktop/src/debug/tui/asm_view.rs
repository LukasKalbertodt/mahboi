use std::{
    cmp,
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
    machine::Machine,
    primitives::{Byte, Word},
};
use super::{
    util::DecodedInstr,
};

const CONTEXT_SIZE: u16 = 40;

#[derive(Clone)]
struct Line {
    addr: Word,
    instr: DecodedInstr,
}

pub struct AsmView {
    lines: Vec<Line>,
}

impl AsmView {
    /// Creates an empty AsmView.
    pub fn new() -> Self {
        let default_line = Line {
            addr: Word::new(0),
            instr: DecodedInstr::Unknown(Byte::new(0)),
        };
        Self {
            lines: vec![default_line; CONTEXT_SIZE as usize],
        }
    }

    pub fn update(&mut self, machine: &Machine) {
        self.lines.clear();

        let mut pos = machine.cpu.pc;
        let mut no_unknown_yet = true;
        for _ in 0..CONTEXT_SIZE {
            let data = [
                machine.load_byte(pos),
                machine.load_byte(pos + 1u8),
                machine.load_byte(pos + 2u8),
            ];

            let instr = if no_unknown_yet {
                let instr = DecodedInstr::decode(&data);
                if instr.is_unknown() {
                    no_unknown_yet = false;
                }
                instr
            } else {
                DecodedInstr::Unknown(data[0])
            };

            let addr = pos;
            pos += instr.len() as u16;

            self.lines.push(Line { addr, instr });
        }
    }
}

impl View for AsmView {
    fn draw(&self, printer: &Printer) {
        for (i, line) in self.lines.iter().enumerate() {
            // Print address
            printer.with_style(Color::Light(BaseColor::Blue), |printer| {
                printer.print((0, i), &format!("{} │   ", line.addr));
            });

            // Print instruction
            line.instr.print(&printer.offset((11, i)));
        }
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        let height = cmp::max(constraint.y, self.lines.len());
        let width = cmp::max(constraint.x, 40);
        Vec2::new(width, height)
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
