use cursive::{
    Printer,
    theme::{Color, BaseColor, Style, Effect},
    utils::markup::StyledString,
};

use mahboi::{
    instr::{Instr, INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
    primitives::{Byte, Word},
};


/// An argument of an instruction.
#[derive(Clone, Debug)]
pub(crate) enum InstrArg {
    /// This means that its a logical argument (seen in the mnemonic), but it's
    /// not actually stored. E.g. `BIT 2, C`: both arguments are static.
    Static(&'static str),

    /// An argument that stores a dynamic value. E.g. `LD B, d8`: the second
    /// argument is a dynamic one.
    Dyn {
        label: &'static str,
        display: String,
        raw: Vec<Byte>,
    },
}

impl InstrArg {
    /// Creates a new `InstrArg` from the argument label (from the mnemonic) and
    /// the argument bytes. The `data` slice can have length 0 for static
    /// arguments.
    pub(crate) fn new(label: &'static str, data: &[Byte]) -> Option<Self> {
        let s = match label {
            "d8" => format!("{}", data.get(0)?),
            "d16" => format!("{}", Word::from_bytes(*data.get(0)?, *data.get(1)?)),
            "(a8)" => format!("(0xFF00+{})", data.get(0)?),
            "a16" => format!("{}", Word::from_bytes(*data.get(0)?, *data.get(1)?)),
            "(a16)" => format!("({})", Word::from_bytes(*data.get(0)?, *data.get(1)?)),
            "r8" => {
                let i = data[0].get() as i8;
                if i < 0 {
                    format!("PC-0x{:02x}", -(i as i16))
                } else {
                    format!("PC+0x{:02x}", i)
                }
            }
            _ => return Some(InstrArg::Static(label)),
        };

        Some(InstrArg::Dyn {
            label,
            display: s,
            raw:data.to_vec(),
        })
    }

    pub(crate) fn raw_data(&self) -> Option<Vec<Byte>> {
        match self {
            InstrArg::Static(..) => None,
            InstrArg::Dyn { raw, .. } => Some(raw.clone()),
        }
    }
}

/// A decoded instruction
#[derive(Clone, Debug)]
pub(crate) enum DecodedInstr {
    NoArgs {
        name: &'static str,
        instr: Instr,
        prefixed: bool,
    },
    OneArg {
        name: &'static str,
        arg: InstrArg,
        instr: Instr,
        prefixed: bool,
    },
    TwoArgs {
        name: &'static str,
        arg0: InstrArg,
        arg1: InstrArg,
        instr: Instr,
        prefixed: bool,
    },
    Unknown(Byte),
}

impl DecodedInstr {
    /// Decodes the given bytes into an instruction. The given byte slice has
    /// to be at least 1 byte long. If the slice is too short for the
    /// instruction to be decoded, `None` is returned.
    pub(crate) fn decode(bytes: &[Byte]) -> Option<Self> {
        let opcode = bytes[0];

        // Fetch the correct instruction data
        let (instr, arg_start, prefixed) = if opcode.get() == 0xCB {
            if bytes.len() == 1 {
                return None;
            }

            (Some(PREFIXED_INSTRUCTIONS[bytes[1]]), 2, true)
        } else {
            (INSTRUCTIONS[opcode], 1, false)
        };

        match instr {
            Some(instr) => {
                // Prepare array of argument data
                let arg_data = &bytes[arg_start..];

                // Interpret the mnemonic string
                let parts = instr.mnemonic.split_whitespace().collect::<Vec<_>>();
                let out = match *parts {
                    [name] => DecodedInstr::NoArgs {
                        name,
                        instr,
                        prefixed,
                    },
                    [name, arg0] => DecodedInstr::OneArg {
                        name,
                        arg: InstrArg::new(arg0, arg_data)?,
                        instr,
                        prefixed,
                    },
                    [name, arg0, arg1] => DecodedInstr::TwoArgs {
                        name,
                        arg0: InstrArg::new(&arg0[..arg0.len() - 1], arg_data)?,
                        arg1: InstrArg::new(arg1, arg_data)?,
                        instr,
                        prefixed,
                    },
                    _ => panic!("internal error: instructions with more than 2 args"),
                };

                Some(out)
            }
            _ => Some(DecodedInstr::Unknown(opcode)),
        }
    }

    pub(crate) fn len(&self) -> u8 {
        match self {
            DecodedInstr::NoArgs { instr, .. } => instr.len,
            DecodedInstr::OneArg { instr, .. } => instr.len,
            DecodedInstr::TwoArgs { instr, .. } => instr.len,
            DecodedInstr::Unknown(_) => 1,
        }
    }

    pub(crate) fn is_unknown(&self) -> bool {
        match self {
            DecodedInstr::Unknown(_) => true,
            _ => false,
        }
    }

    pub(crate) fn prefixed(&self) -> bool {
        match *self {
            DecodedInstr::NoArgs { prefixed, .. } => prefixed,
            DecodedInstr::OneArg { prefixed, .. } => prefixed,
            DecodedInstr::TwoArgs { prefixed, .. } => prefixed,
            DecodedInstr::Unknown(_) => true,
        }
    }

    pub(crate) fn instr(&self) -> Option<Instr> {
        match self {
            DecodedInstr::NoArgs { instr, .. } => Some(*instr),
            DecodedInstr::OneArg { instr, .. } => Some(*instr),
            DecodedInstr::TwoArgs { instr, .. } => Some(*instr),
            DecodedInstr::Unknown(_) => None,
        }
    }

    pub(crate) fn arg0(&self) -> Option<&InstrArg> {
        match self {
            DecodedInstr::OneArg { arg, .. } => Some(arg),
            DecodedInstr::TwoArgs { arg0, .. } => Some(arg0),
            DecodedInstr::NoArgs { .. } | DecodedInstr::Unknown(_) => None,
        }
    }

    pub(crate) fn arg1(&self) -> Option<&InstrArg> {
        match self {
            DecodedInstr::TwoArgs { arg1, .. } => Some(arg1),
            DecodedInstr::OneArg { .. }
            | DecodedInstr::NoArgs { .. }
            | DecodedInstr::Unknown(_) => None,
        }
    }

    /// Creates a styled string representing this instruction.
    pub(crate) fn to_styled_string(&self) -> StyledString {
        fn append_arg(arg: &InstrArg, styled_string: &mut StyledString) {
            let (s, color) = match arg {
                InstrArg::Static(s) => (*s, Color::Light(BaseColor::White)),
                InstrArg::Dyn { display, .. } => (&**display, Color::Dark(BaseColor::Yellow)),
            };

            styled_string.append_styled(s, color);
        }

        let name_style = Style::from(Color::Light(BaseColor::White))
            .combine(Effect::Bold);

        let mut out = StyledString::new();

        match self {
            DecodedInstr::NoArgs { name, .. } => out.append_styled(*name, name_style),
            DecodedInstr::OneArg { name, arg, .. } => {
                out.append_styled(format!("{:5}", name), name_style);
                append_arg(arg, &mut out);
            }
            DecodedInstr::TwoArgs { name, arg0, arg1, .. } => {
                out.append_styled(format!("{:5}", name), name_style);
                append_arg(arg0, &mut out);
                out.append_plain(", ");
                append_arg(arg1, &mut out);
            }
            DecodedInstr::Unknown(byte) => out.append_plain(byte.to_string()),
        }

        out
    }

    /// Prints this instruction into the given printer (with the same
    /// formatting as `to_styled_string()` uses).
    pub(crate) fn print(&self, printer: &Printer) {
        print_styled_string(printer, &self.to_styled_string());
    }
}

/// Takes a styled string and prints it to the given printer.
pub(crate) fn print_styled_string(printer: &Printer, ss: &StyledString) {
    let mut offset = 0;
    for span in ss.spans() {
        printer.with_style(*span.attr, |printer| {
            printer.print((offset, 0), span.content);
        });
        offset += span.content.len();
    }
}
