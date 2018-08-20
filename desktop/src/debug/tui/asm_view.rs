use std::{
    cmp,
    collections::HashMap,
};

use cursive::{
    Printer,
    direction::Direction,
    event::{AnyCb, Event, EventResult},
    theme::{ColorStyle, Color, ColorType, BaseColor},
    view::{View, Selector},
    views::TextView,
    vec::Vec2,
};

use mahboi::{
    machine::{
        Machine,
        instr::{Instr, INSTRUCTIONS},
    },
    primitives::{Byte},
};


const CONTEXT_SIZE: u16 = 40;
const LONGEST_STR_LEN: u16 = 4;

#[derive(Clone)]
enum Line {
    Asm {
        s: &'static str,
    },
    Byte(Byte),
}

// struct InstrCache {
//     map: HashMap<u16, Instr>,
// }

// impl InstrCache {
//     fn new() -> Self {
//         Self {
//             map: HashMap::new(),
//         }
//     }

//     fn get_at(&self, addr: u16) -> Option<Instr> {
//         let start_addrs = (0..LONGEST_STR_LEN)
//             .rev()
//             .filter_map(|offset| addr.checked_sub(offset))
//             .map(|addr| self.map.get(addr))


//     }
// }

pub struct AsmView {
    lines: Vec<Line>,
}

impl AsmView {
    /// Creates an empty AsmView.
    pub fn new() -> Self {
        Self {
            lines: vec![Line::Byte(Byte::new(0)); CONTEXT_SIZE as usize],
        }
    }

    pub fn update(&mut self, machine: &Machine) {
        self.lines.clear();

        let mut pos = machine.cpu.pc;
        let mut no_unknown_yet = true;
        for _ in 0..CONTEXT_SIZE {
            let opcode = machine.load_byte(pos);
            let line = match INSTRUCTIONS[opcode.get() as usize] {
                Some(instr) if no_unknown_yet => {
                    pos = pos + instr.len as u16;
                    Line::Asm {
                        s: instr.mnemonic,
                    }
                }
                _ => {
                    no_unknown_yet = false;
                    pos = pos + 1;
                    Line::Byte(opcode)
                }
            };

            self.lines.push(line);
        }
    }
}

impl View for AsmView {
    fn draw(&self, printer: &Printer) {
        for (i, line) in self.lines.iter().enumerate() {
            match line {
                Line::Asm { s } => {
                    printer.print((0, i), s);
                }
                Line::Byte(b) => {
                    let s = format!("{}", b);
                    printer.print((0, i), &s);
                }
            }
        }
    }

    fn required_size(&mut self, constraint: Vec2) -> Vec2 {
        let height = cmp::max(constraint.y, self.lines.len());
        Vec2::new(constraint.x, height)
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
