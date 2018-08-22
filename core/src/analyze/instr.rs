use std::fmt;

use crate::{
    machine::{
        instr::{Instr, INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
    },
    primitives::{Byte, Word},
};



pub fn decode_instr(data: [Byte; 2]) -> Option<Instr> {
    if data[0] == 0xcb {
        PREFIXED_INSTRUCTIONS[data[1]]
    } else {
        INSTRUCTIONS[data[0]]
    }
}

#[derive(Copy, Clone)]
pub enum RawInstr {
    Short([Byte; 1]),
    Medium([Byte; 2]),
    Long([Byte; 3]),
}

impl RawInstr {
    pub fn from_bytes(data: &[Byte]) -> Self {
        match *data {
            [a] => RawInstr::Short([a]),
            [a, b] => RawInstr::Medium([a, b]),
            [a, b, c] => RawInstr::Long([a, b, c]),
            _ => panic!("oopsie: {:?}", data),
        }
    }

    pub fn instr(&self) -> Instr {
        // We can unwrap, because we checked we are a valid opcode when we were
        // created.
        match *self {
            RawInstr::Short([a]) => decode_instr([a, Byte::new(0)]),
            RawInstr::Medium([a, b]) | RawInstr::Long([a, b, _]) => decode_instr([a, b]),
        }.unwrap()
    }

    pub fn len(&self) -> u8 {
        self.as_slice().len() as u8
    }

    pub fn as_slice(&self) -> &[Byte] {
        match self {
            RawInstr::Short(s) => s,
            RawInstr::Medium(s) => s,
            RawInstr::Long(s) => s,
        }
    }

    /// Returns the jump target for JR, JP, CALL and RST instructions. Will
    /// return `None` for other instructions, notably `RET` and `RETI`.
    pub fn jump_target(&self, from: Word) -> Option<Word> {
        let slice = self.as_slice();
        let instr = self.instr();

        match slice[0].get() {
            opcode!("JR NZ, r8")
            | opcode!("JR NC, r8")
            | opcode!("JR r8")
            | opcode!("JR Z, r8")
            | opcode!("JR C, r8") => {
                Some(from + (slice[1].get() as i8) + instr.len)
            }
            // TODO: more
            _ => None,
        }
    }
}

impl fmt::Debug for RawInstr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "'{}' {:?}", self.instr().mnemonic, self.as_slice())
    }
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
