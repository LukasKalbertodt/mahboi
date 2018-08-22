use std::{
    cmp,
};

use cursive::{
    Printer,
    direction::Direction,
    event::{AnyCb, Event, EventResult},
    theme::{Color, BaseColor, Style, Effect},
    view::{View, Selector},
    // views::TextView,
    vec::Vec2,
};
use unicode_width::UnicodeWidthStr;

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
enum Arg {
    Static(&'static str),
    Dyn(String),
}

impl Arg {
    fn new(name: &'static str, data: &[Byte]) -> Self {
        let s = match name {
            "d8" => format!("{}", data[0]),
            "d16" => format!("{}", Word::from_bytes(data[0], data[1])),
            "a8" => format!("(0xFF00+{})", data[0]),
            "a16" => format!("({})", Word::from_bytes(data[0], data[1])),
            "r8" => format!("PC+0x{:02x}", data[0].get() as i8),
            _ => return Arg::Static(name),
        };

        Arg::Dyn(s)
    }
}

#[derive(Clone)]
enum LineKind {
    NoArgs(&'static str),
    OneArg {
        name: &'static str,
        arg: Arg,
    },
    TwoArgs {
        name: &'static str,
        arg0: Arg,
        arg1: Arg,
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

            // Fetch the correct instruction data
            let (instr, arg_start) = if opcode.get() == 0xCB {
                let opcode = machine.load_byte(pos + 1u16);
                (PREFIXED_INSTRUCTIONS[opcode], pos + 2u16)
            } else {
                (INSTRUCTIONS[opcode], pos + 1u16)
            };

            let (kind, len) = match instr {
                Some(instr) if no_unknown_yet => {
                    // Prepare array of argument data
                    let data = [
                        machine.load_byte(arg_start + 1u16),
                        machine.load_byte(arg_start + 2u16),
                    ];
                    let data = &data[..instr.len as usize - 1];

                    // Interpret the mnemonic string
                    let parts = instr.mnemonic.split_whitespace().collect::<Vec<_>>();
                    let kind = match &*parts {
                        &[name] => LineKind::NoArgs(name),
                        &[name, arg0] => LineKind::OneArg {
                            name,
                            arg: Arg::new(arg0, data),
                        },
                        &[name, arg0, arg1] => LineKind::TwoArgs {
                            name,
                            arg0: Arg::new(&arg0[..arg0.len() - 1], data),
                            arg1: Arg::new(arg1, data),
                        },
                        _ => panic!("internal error: instructions with more than 2 args"),
                    };

                    (kind, instr.len)
                }
                _ => {
                    no_unknown_yet = false;
                    (LineKind::Byte(opcode), 1)
                }
            };

            self.lines.push(Line {
                addr: pos,
                kind,
            });

            pos += len as u16;
        }
    }
}

impl View for AsmView {
    fn draw(&self, printer: &Printer) {
        fn print_arg(arg: &Arg, printer: &Printer) -> usize {
            let (s, color) = match arg {
                Arg::Static(s) => {
                    (*s, Color::Light(BaseColor::White))
                }
                Arg::Dyn(s) => {
                    (&**s, Color::Dark(BaseColor::Yellow))
                }
            };

            printer.with_style(color, |printer| {
                printer.print((0, 0), &s);
            });
            s.width()
        }

        // Styles for certain parts of the instructions.
        let addr_style = Style::from(Color::Light(BaseColor::Blue));
        let name_style = Style::from(Color::Light(BaseColor::White)).combine(Effect::Bold);


        for (i, line) in self.lines.iter().enumerate() {
            // Print address
            printer.with_style(addr_style, |printer| {
                printer.print((0, i), &format!("{} │   ", line.addr));
            });
            let instr_offset = 11;
            let arg_offset = instr_offset + 6;


            // Print instruction
            match &line.kind {
                LineKind::NoArgs(name) => {
                    printer.with_style(name_style, |printer| {
                        printer.print((instr_offset, i), name);
                    });
                }
                LineKind::OneArg { name, arg } => {
                    printer.with_style(name_style, |printer| {
                        printer.print((instr_offset, i), name);
                    });
                    print_arg(&arg, &printer.offset((arg_offset, i)));
                }
                LineKind::TwoArgs { name, arg0, arg1 } => {
                    printer.with_style(name_style, |printer| {
                        printer.print((instr_offset, i), name);
                    });
                    let used = print_arg(&arg0, &printer.offset((arg_offset, i)));
                    let arg1_offset = arg_offset + used;
                    printer.print((arg1_offset, i), ", ");
                    print_arg(&arg1, &printer.offset((arg1_offset + 2, i)));
                }
                LineKind::Byte(b) => {
                    let s = format!("{}", b);
                    printer.print((instr_offset, i), &s);
                }
            }
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
