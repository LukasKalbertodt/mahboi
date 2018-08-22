use std::fmt;

use crate::{
    machine::{
        Machine,
        instr::{Instr, INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
    },
    primitives::{Byte, Word},
};




#[derive(Clone, Copy)]
pub struct InstrWithArg {
    kind: Instr,
    arg: InstrArg,
}

impl InstrWithArg {
    /// Decodes the instruction at address `at`.
    pub fn decode(at: Word, machine: &Machine) -> Option<Self> {
        let first = machine.load_byte(at);

        // Special case CB PREFIX instructions
        if first == 0xcb {
            // Prefixed instructions are always two bytes long.
            let second = machine.load_byte(at + 1u8);
            Some(Self {
                kind: PREFIXED_INSTRUCTIONS[second].unwrap(),
                arg: InstrArg::None,
            })
        } else {
            let kind = INSTRUCTIONS[first]?;

            let arg = match kind.len {
                1 => InstrArg::None,
                2 => InstrArg::Byte(machine.load_byte(at + 1u8)),
                3 => InstrArg::Word(machine.load_word(at + 1u8)),
                _ => unreachable!(),
            };

            Some(Self { kind, arg })
        }
    }

    pub fn kind(&self) -> &Instr {
        &self.kind
    }

    pub fn arg(&self) -> &InstrArg {
        &self.arg
    }

    /// Returns the jump target for JR, JP, CALL and RST instructions and
    /// `None` for other instructions, notably `RET` and `RETI`. Calculates
    /// relative jumps with `from` as base address.
    pub fn jump_target(&self, from: Word) -> Option<Word> {
        match self.kind.opcode.get() {
            opcode!("JR NZ, r8")
            | opcode!("JR NC, r8")
            | opcode!("JR r8")
            | opcode!("JR Z, r8")
            | opcode!("JR C, r8") => {
                let offset = self.arg.as_byte().unwrap().get() as i8;
                Some(from + offset + self.kind.len)
            }
            // TODO: more
            _ => None,
        }
    }
}

impl fmt::Debug for InstrWithArg {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}: '{}' <{:?}>",
            self.kind.opcode,
            self.kind.mnemonic,
            self.arg,
        )
    }
}

#[derive(Copy, Clone, Debug)]
pub enum InstrArg {
    None,
    Byte(Byte),
    Word(Word),
}

impl InstrArg {
    pub fn is_none(&self) -> bool {
        match self {
            InstrArg::None => true,
            _ => false,
        }
    }

    pub fn as_byte(&self) -> Option<Byte> {
        match self {
            InstrArg::Byte(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_word(&self) -> Option<Word> {
        match self {
            InstrArg::Word(w) => Some(*w),
            _ => None,
        }
    }
    // pub fn from_bytes(data: &[Byte]) -> Self {
    //     match *data {
    //         [] => RawInstr::None,
    //         [a] => RawInstr::Single([a]),
    //         [a, b] => RawInstr::Double([a, b]),
    //         _ => unreachable!(),
    //     }
    // }

    // pub fn instr(&self) -> Instr {
    //     // We can unwrap, because we checked we are a valid opcode when we were
    //     // created.
    //     match *self {
    //         RawInstr::One([a]) => decode_instr([a, Byte::new(0)]),
    //         RawInstr::Two([a, b]) | RawInstr::Three([a, b, _]) => decode_instr([a, b]),
    //     }.unwrap()
    // }

    // pub fn len(&self) -> u8 {
    //     self.as_slice().len() as u8
    // }

    // pub fn as_slice(&self) -> &[Byte] {
    //     match self {
    //         InstrArg::None => &[],
    //         InstrArg::Single(a) => a,
    //         InstrArg::Double(a) => a,
    //     }
    // }
}



pub trait InstrExt {
    fn is_one_of(&self, opcodes: &[u8]) -> bool;

    /// JR
    fn is_rel_jump(&self) -> bool;

    /// JP
    fn is_abs_jump(&self) -> bool;

    /// CALL
    fn is_call(&self) -> bool;

    /// RST (interrupt call)
    fn is_int_call(&self) -> bool;

    /// RET and RETI
    fn is_ret(&self) -> bool;

    /// Any instruction that modifies the PC in an abnormal way: JR, JP, CALL, RET, RETI, RST
    fn jumps(&self) -> bool {
        self.is_rel_jump()
            || self.is_abs_jump()
            || self.is_call()
            || self.is_int_call()
            || self.is_ret()
    }

    fn always_jumps(&self) -> bool {
        self.jumps() && self.is_one_of(&[
            0x18, // JR r8
            0xc3, // JP a16
            0xc9, // RET
            0xd9, // RETI
            0xe9, // JP (HL)
            0xcd, // CALL a16
            0xc7, // RST 00
            0xcf, // RST 08
            0xd7, // RST 10
            0xdf, // RST 18
            0xe7, // RST 20
            0xef, // RST 28
            0xf7, // RST 30
            0xff, // RST 38
        ])
    }
}

impl InstrExt for Instr {
    fn is_one_of(&self, opcodes: &[u8]) -> bool {
        opcodes.contains(&self.opcode.get())
    }

    fn is_rel_jump(&self) -> bool {
        self.mnemonic.starts_with("JR ")
    }

    fn is_abs_jump(&self) -> bool {
        self.mnemonic.starts_with("JP ")
    }

    fn is_call(&self) -> bool {
        self.mnemonic.starts_with("CALL ")
    }

    fn is_int_call(&self) -> bool {
        self.mnemonic.starts_with("RST ")
    }

    fn is_ret(&self) -> bool {
        self.mnemonic.starts_with("RET")
    }
}
