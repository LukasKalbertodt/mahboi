use std::{
    cmp,
};

use cursive::{
    Printer,
    direction::Direction,
    event::{AnyCb, Event, EventResult},
    // theme::{ColorStyle, Color, ColorType, BaseColor},
    view::{View, Selector},
    // views::TextView,
    vec::Vec2,
};

use mahboi::{
    machine::{
        Machine,
        instr::{INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
    },
    primitives::{Byte, Word},
};


const CONTEXT_SIZE: u16 = 40;
// const LONGEST_STR_LEN: u16 = 4;

#[derive(Clone)]
enum LineKind {
    Asm {
        s: &'static str,
    },
    Byte(Byte),
}

#[derive(Clone)]
struct Line {
    addr: Word,
    kind: LineKind,
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
        let default_line = Line {
            addr: Word::new(0),
            kind: LineKind::Byte(Byte::new(0)),
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
            let opcode = machine.load_byte(pos);
            let addr = pos;

            let instr = if opcode.get() == 0xCB {
                let opcode = machine.load_byte(pos + 1u16);
                PREFIXED_INSTRUCTIONS[opcode.get() as usize]
            } else {
                INSTRUCTIONS[opcode.get() as usize]
            };

            let kind = match instr {
                Some(instr) if no_unknown_yet => {
                    pos = pos + instr.len as u16;
                    LineKind::Asm {
                        s: instr.mnemonic,
                    }
                }
                _ => {
                    no_unknown_yet = false;
                    pos += 1u16;
                    LineKind::Byte(opcode)
                }
            };

            self.lines.push(Line {
                addr,
                kind,
            });
        }
    }
}

impl View for AsmView {
    fn draw(&self, printer: &Printer) {
        for (i, line) in self.lines.iter().enumerate() {
            printer.print((0, i), &format!("{}   ", line.addr));
            let offset = 9;
            match line.kind {
                LineKind::Asm { s } => {
                    printer.print((offset, i), s);
                }
                LineKind::Byte(b) => {
                    let s = format!("{}", b);
                    printer.print((offset, i), &s);
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
