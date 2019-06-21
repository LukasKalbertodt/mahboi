//! Instruction data.
//!
//! This module contains some basic information like size and clock count for
//! all instructions. It is stored in two 256-element long arrays -- one for
//! the main instructions and one for all PREFIX CB instructions.

use std::ops::Index;

use crate::primitives::Byte;

/// The information we store per instruction.
#[derive(Debug, Clone, Copy)]
pub struct Instr {
    /// The (meaningful) opcode of this instruction. For prefixed instructions,
    /// this is the second byte and not `0xCB`. This is always the index in the
    /// corresponding array. But it's still useful to store in the struct, if
    /// the struct is stored somewhere else, too.
    pub opcode: Byte,

    /// Full mnemonic.
    ///
    /// This includes the arguments with several placeholders for values:
    /// - `d8`: immediate 8 bit data
    /// - `d16`: immediate 16 bit data
    /// - `a8`: 8 bit unsigned value that is added to `$FF00`
    /// - `a16`: 16 bit address
    /// - `r8`: 8 bit signed value which is added to PC
    pub mnemonic: &'static str,

    /// Length in bytes
    pub len: u8,

    /// CPU clocks (4 Mhz steps - 1 Mhz steps are called cycles)
    pub clocks: u8,

    /// CPU clocks, if branch is taken.
    ///
    /// This is only set for instructions that have to decide something and
    /// only sometimes perform an operation (conditional jumps, mostly). If the
    /// branch/action is not taken, `clocks` is the correct value.
    pub clocks_taken: Option<u8>,
}

impl Instr {
    const fn some(
        opcode: u8,
        mnemonic: &'static str,
        len: u8,
        clocks: u8,
        clocks_taken: Option<u8>,
    ) -> Option<Self> {
        Some(Instr {
            opcode: Byte::new(opcode),
            mnemonic,
            len,
            clocks,
            clocks_taken,
        })
    }

    const fn new(
        opcode: u8,
        mnemonic: &'static str,
        len: u8,
        clocks: u8,
        clocks_taken: Option<u8>,
    ) -> Self {
        Instr {
            opcode: Byte::new(opcode),
            mnemonic,
            len,
            clocks,
            clocks_taken,
        }
    }
}

/// Simple wrapper to make the static array indexable with `Byte` instead of
/// `usize`.
pub struct InstrDb<T>([T; 256]);

impl<T> Index<Byte> for InstrDb<T> {
    type Output = T;
    fn index(&self, idx: Byte) -> &Self::Output {
        &self.0[idx.get() as usize]
    }
}

/// Main instruction data.
///
/// Entries with the value `None` are invalid opcodes.
///
/// Regarding the special CB PREFIX instructions: in this array it has a len
/// and clock number of 0. In the cheat sheet, those values are stated as 1 and
/// 4 respectively. This is wrong in a sense: the length/clock values for all
/// CB-instructions in `PREFIXED_INSTRUCTIONS` already contains the total
/// value. The actual prefix doesn't add anything.
pub const INSTRUCTIONS: InstrDb<Option<Instr>> = InstrDb([
    Instr::some(0x00, "NOP",          1,  4,  None),
    Instr::some(0x01, "LD BC, d16",   3, 12,  None),
    Instr::some(0x02, "LD (BC), A",   1,  8,  None),
    Instr::some(0x03, "INC BC",       1,  8,  None),
    Instr::some(0x04, "INC B",        1,  4,  None),
    Instr::some(0x05, "DEC B",        1,  4,  None),
    Instr::some(0x06, "LD B, d8",     2,  8,  None),
    Instr::some(0x07, "RLCA",         1,  4,  None),
    Instr::some(0x08, "LD (a16), SP", 3,  20, None),
    Instr::some(0x09, "ADD HL, BC",   1,  8,  None),
    Instr::some(0x0a, "LD A, (BC)",   1,  8,  None),
    Instr::some(0x0b, "DEC BC",       1,  8,  None),
    Instr::some(0x0c, "INC C",        1,  4,  None),
    Instr::some(0x0d, "DEC C",        1,  4,  None),
    Instr::some(0x0e, "LD C, d8",     2,  8,  None),
    Instr::some(0x0f, "RRCA",         1,  4,  None),

    Instr::some(0x10, "STOP",         2,  4,  None),
    Instr::some(0x11, "LD DE, d16",   3, 12,  None),
    Instr::some(0x12, "LD (DE), A",   1,  8,  None),
    Instr::some(0x13, "INC DE",       1,  8,  None),
    Instr::some(0x14, "INC D",        1,  4,  None),
    Instr::some(0x15, "DEC D",        1,  4,  None),
    Instr::some(0x16, "LD D, d8",     2,  8,  None),
    Instr::some(0x17, "RLA",          1,  4,  None),
    Instr::some(0x18, "JR r8",        2, 12,  None),
    Instr::some(0x19, "ADD HL, DE",   1,  8,  None),
    Instr::some(0x1a, "LD A, (DE)",   1,  8,  None),
    Instr::some(0x1b, "DEC DE",       1,  8,  None),
    Instr::some(0x1c, "INC E",        1,  4,  None),
    Instr::some(0x1d, "DEC E",        1,  4,  None),
    Instr::some(0x1e, "LD E, d8",     2,  8,  None),
    Instr::some(0x1f, "RRA",          1,  4,  None),

    Instr::some(0x20, "JR NZ, r8",    2,  8,  Some(12)),
    Instr::some(0x21, "LD HL, d16",   3,  12, None),
    Instr::some(0x22, "LD (HL+), A",  1,  8,  None),
    Instr::some(0x23, "INC HL",       1,  8,  None),
    Instr::some(0x24, "INC H",        1,  4,  None),
    Instr::some(0x25, "DEC H",        1,  4,  None),
    Instr::some(0x26, "LD H, d8",     2,  8,  None),
    Instr::some(0x27, "DAA",          1,  4,  None),
    Instr::some(0x28, "JR Z, r8",     2,  8,  Some(12)),
    Instr::some(0x29, "ADD HL, HL",   1,  8,  None),
    Instr::some(0x2a, "LD A, (HL+)",  1,  8,  None),
    Instr::some(0x2b, "DEC HL",       1,  8,  None),
    Instr::some(0x2c, "INC L",        1,  4,  None),
    Instr::some(0x2d, "DEC L",        1,  4,  None),
    Instr::some(0x2e, "LD L, d8",     2,  8,  None),
    Instr::some(0x2f, "CPL",          1,  4,  None),

    Instr::some(0x30, "JR NC, r8",    2,  8,  Some(12)),
    Instr::some(0x31, "LD SP, d16",   3,  12, None),
    Instr::some(0x32, "LD (HL-), A",  1,  8,  None),
    Instr::some(0x33, "INC SP",       1,  8,  None),
    Instr::some(0x34, "INC (HL)",     1,  12, None),
    Instr::some(0x35, "DEC (HL)",     1,  12, None),
    Instr::some(0x36, "LD (HL), d8",  2,  12, None),
    Instr::some(0x37, "SCF",          1,  4,  None),
    Instr::some(0x38, "JR C, r8",     2,  8,  Some(12)),
    Instr::some(0x39, "ADD HL, SP",   1,  8,  None),
    Instr::some(0x3a, "LD A, (HL-)",  1,  8,  None),
    Instr::some(0x3b, "DEC SP",       1,  8,  None),
    Instr::some(0x3c, "INC A",        1,  4,  None),
    Instr::some(0x3d, "DEC A",        1,  4,  None),
    Instr::some(0x3e, "LD A, d8",     2,  8,  None),
    Instr::some(0x3f, "CCF",          1,  4,  None),

    Instr::some(0x40, "LD B, B",      1,  4,  None),
    Instr::some(0x41, "LD B, C",      1,  4,  None),
    Instr::some(0x42, "LD B, D",      1,  4,  None),
    Instr::some(0x43, "LD B, E",      1,  4,  None),
    Instr::some(0x44, "LD B, H",      1,  4,  None),
    Instr::some(0x45, "LD B, L",      1,  4,  None),
    Instr::some(0x46, "LD B, (HL)",   1,  8,  None),
    Instr::some(0x47, "LD B, A",      1,  4,  None),
    Instr::some(0x48, "LD C, B",      1,  4,  None),
    Instr::some(0x49, "LD C, C",      1,  4,  None),
    Instr::some(0x4a, "LD C, D",      1,  4,  None),
    Instr::some(0x4b, "LD C, E",      1,  4,  None),
    Instr::some(0x4c, "LD C, H",      1,  4,  None),
    Instr::some(0x4d, "LD C, L",      1,  4,  None),
    Instr::some(0x4e, "LD C, (HL)",   1,  8,  None),
    Instr::some(0x4f, "LD C, A",      1,  4,  None),

    Instr::some(0x50, "LD D, B",      1,  4,  None),
    Instr::some(0x51, "LD D, C",      1,  4,  None),
    Instr::some(0x52, "LD D, D",      1,  4,  None),
    Instr::some(0x53, "LD D, E",      1,  4,  None),
    Instr::some(0x54, "LD D, H",      1,  4,  None),
    Instr::some(0x55, "LD D, L",      1,  4,  None),
    Instr::some(0x56, "LD D, (HL)",   1,  8,  None),
    Instr::some(0x57, "LD D, A",      1,  4,  None),
    Instr::some(0x58, "LD E, B",      1,  4,  None),
    Instr::some(0x59, "LD E, C",      1,  4,  None),
    Instr::some(0x5a, "LD E, D",      1,  4,  None),
    Instr::some(0x5b, "LD E, E",      1,  4,  None),
    Instr::some(0x5c, "LD E, H",      1,  4,  None),
    Instr::some(0x5d, "LD E, L",      1,  4,  None),
    Instr::some(0x5e, "LD E, (HL)",   1,  8,  None),
    Instr::some(0x5f, "LD E, A",      1,  4,  None),

    Instr::some(0x60, "LD H, B",      1,  4,  None),
    Instr::some(0x61, "LD H, C",      1,  4,  None),
    Instr::some(0x62, "LD H, D",      1,  4,  None),
    Instr::some(0x63, "LD H, E",      1,  4,  None),
    Instr::some(0x64, "LD H, H",      1,  4,  None),
    Instr::some(0x65, "LD H, L",      1,  4,  None),
    Instr::some(0x66, "LD H, (HL)",   1,  8,  None),
    Instr::some(0x67, "LD H, A",      1,  4,  None),
    Instr::some(0x68, "LD L, B",      1,  4,  None),
    Instr::some(0x69, "LD L, C",      1,  4,  None),
    Instr::some(0x6a, "LD L, D",      1,  4,  None),
    Instr::some(0x6b, "LD L, E",      1,  4,  None),
    Instr::some(0x6c, "LD L, H",      1,  4,  None),
    Instr::some(0x6d, "LD L, L",      1,  4,  None),
    Instr::some(0x6e, "LD L, (HL)",   1,  8,  None),
    Instr::some(0x6f, "LD L, A",      1,  4,  None),

    Instr::some(0x70, "LD (HL), B",   1,  8,  None),
    Instr::some(0x71, "LD (HL), C",   1,  8,  None),
    Instr::some(0x72, "LD (HL), D",   1,  8,  None),
    Instr::some(0x73, "LD (HL), E",   1,  8,  None),
    Instr::some(0x74, "LD (HL), H",   1,  8,  None),
    Instr::some(0x75, "LD (HL), L",   1,  8,  None),
    Instr::some(0x76, "HALT",         1,  4,  None),
    Instr::some(0x77, "LD (HL), A",   1,  8,  None),
    Instr::some(0x78, "LD A, B",      1,  4,  None),
    Instr::some(0x79, "LD A, C",      1,  4,  None),
    Instr::some(0x7a, "LD A, D",      1,  4,  None),
    Instr::some(0x7b, "LD A, E",      1,  4,  None),
    Instr::some(0x7c, "LD A, H",      1,  4,  None),
    Instr::some(0x7d, "LD A, L",      1,  4,  None),
    Instr::some(0x7e, "LD A, (HL)",   1,  8,  None),
    Instr::some(0x7f, "LD A, A",      1,  4,  None),

    Instr::some(0x80, "ADD A, B",     1,  4,  None),
    Instr::some(0x81, "ADD A, C",     1,  4,  None),
    Instr::some(0x82, "ADD A, D",     1,  4,  None),
    Instr::some(0x83, "ADD A, E",     1,  4,  None),
    Instr::some(0x84, "ADD A, H",     1,  4,  None),
    Instr::some(0x85, "ADD A, L",     1,  4,  None),
    Instr::some(0x86, "ADD A, (HL)",  1,  8,  None),
    Instr::some(0x87, "ADD A, A",     1,  4,  None),
    Instr::some(0x88, "ADC A, B",     1,  4,  None),
    Instr::some(0x89, "ADC A, C",     1,  4,  None),
    Instr::some(0x8a, "ADC A, D",     1,  4,  None),
    Instr::some(0x8b, "ADC A, E",     1,  4,  None),
    Instr::some(0x8c, "ADC A, H",     1,  4,  None),
    Instr::some(0x8d, "ADC A, L",     1,  4,  None),
    Instr::some(0x8e, "ADC A, (HL)",  1,  8,  None),
    Instr::some(0x8f, "ADC A, A",     1,  4,  None),

    Instr::some(0x90, "SUB B",        1,  4,  None),
    Instr::some(0x91, "SUB C",        1,  4,  None),
    Instr::some(0x92, "SUB D",        1,  4,  None),
    Instr::some(0x93, "SUB E",        1,  4,  None),
    Instr::some(0x94, "SUB H",        1,  4,  None),
    Instr::some(0x95, "SUB L",        1,  4,  None),
    Instr::some(0x96, "SUB (HL)",     1,  8,  None),
    Instr::some(0x97, "SUB A",        1,  4,  None),
    Instr::some(0x98, "SBC A, B",     1,  4,  None),
    Instr::some(0x99, "SBC A, C",     1,  4,  None),
    Instr::some(0x9a, "SBC A, D",     1,  4,  None),
    Instr::some(0x9b, "SBC A, E",     1,  4,  None),
    Instr::some(0x9c, "SBC A, H",     1,  4,  None),
    Instr::some(0x9d, "SBC A, L",     1,  4,  None),
    Instr::some(0x9e, "SBC A, (HL)",  1,  8,  None),
    Instr::some(0x9f, "SBC A, A",     1,  4,  None),

    Instr::some(0xa0, "AND B",        1,  4,  None),
    Instr::some(0xa1, "AND C",        1,  4,  None),
    Instr::some(0xa2, "AND D",        1,  4,  None),
    Instr::some(0xa3, "AND E",        1,  4,  None),
    Instr::some(0xa4, "AND H",        1,  4,  None),
    Instr::some(0xa5, "AND L",        1,  4,  None),
    Instr::some(0xa6, "AND (HL)",     1,  8,  None),
    Instr::some(0xa7, "AND A",        1,  4,  None),
    Instr::some(0xa8, "XOR B",        1,  4,  None),
    Instr::some(0xa9, "XOR C",        1,  4,  None),
    Instr::some(0xaa, "XOR D",        1,  4,  None),
    Instr::some(0xab, "XOR E",        1,  4,  None),
    Instr::some(0xac, "XOR H",        1,  4,  None),
    Instr::some(0xad, "XOR L",        1,  4,  None),
    Instr::some(0xae, "XOR (HL)",     1,  8,  None),
    Instr::some(0xaf, "XOR A",        1,  4,  None),

    Instr::some(0xb0, "OR B",         1,  4,  None),
    Instr::some(0xb1, "OR C",         1,  4,  None),
    Instr::some(0xb2, "OR D",         1,  4,  None),
    Instr::some(0xb3, "OR E",         1,  4,  None),
    Instr::some(0xb4, "OR H",         1,  4,  None),
    Instr::some(0xb5, "OR L",         1,  4,  None),
    Instr::some(0xb6, "OR (HL)",      1,  8,  None),
    Instr::some(0xb7, "OR A",         1,  4,  None),
    Instr::some(0xb8, "CP B",         1,  4,  None),
    Instr::some(0xb9, "CP C",         1,  4,  None),
    Instr::some(0xba, "CP D",         1,  4,  None),
    Instr::some(0xbb, "CP E",         1,  4,  None),
    Instr::some(0xbc, "CP H",         1,  4,  None),
    Instr::some(0xbd, "CP L",         1,  4,  None),
    Instr::some(0xbe, "CP (HL)",      1,  8,  None),
    Instr::some(0xbf, "CP A",         1,  4,  None),

    Instr::some(0xc0, "RET NZ",       1,  8,  Some(20)),
    Instr::some(0xc1, "POP BC",       1,  12, None),
    Instr::some(0xc2, "JP NZ, a16",   3,  12, Some(16)),
    Instr::some(0xc3, "JP a16",       3,  16, None),
    Instr::some(0xc4, "CALL NZ, a16", 3,  12, Some(24)),
    Instr::some(0xc5, "PUSH BC",      1,  16, None),
    Instr::some(0xc6, "ADD A, d8",    2,  8,  None),
    Instr::some(0xc7, "RST 00H",      1,  16, None),
    Instr::some(0xc8, "RET Z",        1,  8,  Some(20)),
    Instr::some(0xc9, "RET",          1,  16, None),
    Instr::some(0xca, "JP Z, a16",    3,  12, Some(16)),
    Instr::some(0xcb, "PREFIX CB",    0,  0,  None),
    Instr::some(0xcc, "CALL Z, a16",  3,  12, Some(24)),
    Instr::some(0xcd, "CALL a16",     3,  24, None),
    Instr::some(0xce, "ADC A, d8",    2,  8,  None),
    Instr::some(0xcf, "RST 08H",      1,  16, None),

    Instr::some(0xd0, "RET NC",       1,  8,  Some(20)),
    Instr::some(0xd1, "POP DE",       1,  12, None),
    Instr::some(0xd2, "JP NC, a16",   3,  12, Some(16)),
    None,
    Instr::some(0xd4, "CALL NC, a16", 3,  12, Some(24)),
    Instr::some(0xd5, "PUSH DE",      1,  16, None),
    Instr::some(0xd6, "SUB d8",       2,  8,  None),
    Instr::some(0xd7, "RST 10H",      1,  16, None),
    Instr::some(0xd8, "RET C",        1,  8,  Some(20)),
    Instr::some(0xd9, "RETI",         1,  16, None),
    Instr::some(0xda, "JP C, a16",    3,  12, Some(16)),
    None,
    Instr::some(0xdc, "CALL C, a16",  3,  12, Some(24)),
    None,
    Instr::some(0xde, "SBC A, d8",    2,  8,  None),
    Instr::some(0xdf, "RST 18H",      1,  16, None),

    Instr::some(0xe0, "LDH (a8), A",  2,  12, None),
    Instr::some(0xe1, "POP HL",       1,  12, None),
    Instr::some(0xe2, "LD (C), A",    1,  8,  None),
    None,
    None,
    Instr::some(0xe5, "PUSH HL",      1,  16, None),
    Instr::some(0xe6, "AND d8",       2,  8,  None),
    Instr::some(0xe7, "RST 20H",      1,  16, None),
    Instr::some(0xe8, "ADD SP, r8",   2,  16, None),
    Instr::some(0xe9, "JP HL",      1,  4,  None),
    Instr::some(0xea, "LD (a16), A",  3,  16, None),
    None,
    None,
    None,
    Instr::some(0xee, "XOR d8",       2,  8,  None),
    Instr::some(0xef, "RST 28H",      1,  16, None),

    Instr::some(0xf0, "LDH A, (a8)",  2,  12, None),
    Instr::some(0xf1, "POP AF",       1,  12, None),
    Instr::some(0xf2, "LD A, (C)",    1,  8,  None),
    Instr::some(0xf3, "DI",           1,  4,  None),
    None,
    Instr::some(0xf5, "PUSH AF",      1,  16, None),
    Instr::some(0xf6, "OR d8",        2,  8,  None),
    Instr::some(0xf7, "RST 30H",      1,  16, None),
    Instr::some(0xf8, "LD HL, SP+r8", 2,  12, None),
    Instr::some(0xf9, "LD SP, HL",    1,  8,  None),
    Instr::some(0xfa, "LD A, (a16)",  3,  16, None),
    Instr::some(0xfb, "EI",           1,  4,  None),
    None,
    None,
    Instr::some(0xfe, "CP d8",        2,  8,  None),
    Instr::some(0xff, "RST 38H",      1,  16, None),
]);

/// Instructions prefixed by CB opcode. (These opcodes are 2 bytes long.)
pub const PREFIXED_INSTRUCTIONS: InstrDb<Instr> = InstrDb([
    Instr::new(0x00, "RLC B",        2,  8,  None),
    Instr::new(0x01, "RLC C",        2,  8,  None),
    Instr::new(0x02, "RLC D",        2,  8,  None),
    Instr::new(0x03, "RLC E",        2,  8,  None),
    Instr::new(0x04, "RLC H",        2,  8,  None),
    Instr::new(0x05, "RLC L",        2,  8,  None),
    Instr::new(0x06, "RLC (HL)",     2,  16, None),
    Instr::new(0x07, "RLC A",        2,  8,  None),
    Instr::new(0x08, "RRC B",        2,  8,  None),
    Instr::new(0x09, "RRC C",        2,  8,  None),
    Instr::new(0x0a, "RRC D",        2,  8,  None),
    Instr::new(0x0b, "RRC E",        2,  8,  None),
    Instr::new(0x0c, "RRC H",        2,  8,  None),
    Instr::new(0x0d, "RRC L",        2,  8,  None),
    Instr::new(0x0e, "RRC (HL)",     2,  16, None),
    Instr::new(0x0f, "RRC A",        2,  8,  None),

    Instr::new(0x10, "RL B",         2,  8,  None),
    Instr::new(0x11, "RL C",         2,  8,  None),
    Instr::new(0x12, "RL D",         2,  8,  None),
    Instr::new(0x13, "RL E",         2,  8,  None),
    Instr::new(0x14, "RL H",         2,  8,  None),
    Instr::new(0x15, "RL L",         2,  8,  None),
    Instr::new(0x16, "RL (HL)",      2,  16, None),
    Instr::new(0x17, "RL A",         2,  8,  None),
    Instr::new(0x18, "RR B",         2,  8,  None),
    Instr::new(0x19, "RR C",         2,  8,  None),
    Instr::new(0x1a, "RR D",         2,  8,  None),
    Instr::new(0x1b, "RR E",         2,  8,  None),
    Instr::new(0x1c, "RR H",         2,  8,  None),
    Instr::new(0x1d, "RR L",         2,  8,  None),
    Instr::new(0x1e, "RR (HL)",      2,  16, None),
    Instr::new(0x1f, "RR A",         2,  8,  None),

    Instr::new(0x20, "SLA B",        2,  8,  None),
    Instr::new(0x21, "SLA C",        2,  8,  None),
    Instr::new(0x22, "SLA D",        2,  8,  None),
    Instr::new(0x23, "SLA E",        2,  8,  None),
    Instr::new(0x24, "SLA H",        2,  8,  None),
    Instr::new(0x25, "SLA L",        2,  8,  None),
    Instr::new(0x26, "SLA (HL)",     2,  16, None),
    Instr::new(0x27, "SLA A",        2,  8,  None),
    Instr::new(0x28, "SRA B",        2,  8,  None),
    Instr::new(0x29, "SRA C",        2,  8,  None),
    Instr::new(0x2a, "SRA D",        2,  8,  None),
    Instr::new(0x2b, "SRA E",        2,  8,  None),
    Instr::new(0x2c, "SRA H",        2,  8,  None),
    Instr::new(0x2d, "SRA L",        2,  8,  None),
    Instr::new(0x2e, "SRA (HL)",     2,  16, None),
    Instr::new(0x2f, "SRA A",        2,  8,  None),

    Instr::new(0x30, "SWAP B",       2,  8,  None),
    Instr::new(0x31, "SWAP C",       2,  8,  None),
    Instr::new(0x32, "SWAP D",       2,  8,  None),
    Instr::new(0x33, "SWAP E",       2,  8,  None),
    Instr::new(0x34, "SWAP H",       2,  8,  None),
    Instr::new(0x35, "SWAP L",       2,  8,  None),
    Instr::new(0x36, "SWAP (HL)",    2,  16, None),
    Instr::new(0x37, "SWAP A",       2,  8,  None),
    Instr::new(0x38, "SRL B",        2,  8,  None),
    Instr::new(0x39, "SRL C",        2,  8,  None),
    Instr::new(0x3a, "SRL D",        2,  8,  None),
    Instr::new(0x3b, "SRL E",        2,  8,  None),
    Instr::new(0x3c, "SRL H",        2,  8,  None),
    Instr::new(0x3d, "SRL L",        2,  8,  None),
    Instr::new(0x3e, "SRL (HL)",     2,  16, None),
    Instr::new(0x3f, "SRL A",        2,  8,  None),

    Instr::new(0x40, "BIT 0, B",     2,  8,  None),
    Instr::new(0x41, "BIT 0, C",     2,  8,  None),
    Instr::new(0x42, "BIT 0, D",     2,  8,  None),
    Instr::new(0x43, "BIT 0, E",     2,  8,  None),
    Instr::new(0x44, "BIT 0, H",     2,  8,  None),
    Instr::new(0x45, "BIT 0, L",     2,  8,  None),
    Instr::new(0x46, "BIT 0, (HL)",  2,  16, None),
    Instr::new(0x47, "BIT 0, A",     2,  8,  None),
    Instr::new(0x48, "BIT 1, B",     2,  8,  None),
    Instr::new(0x49, "BIT 1, C",     2,  8,  None),
    Instr::new(0x4a, "BIT 1, D",     2,  8,  None),
    Instr::new(0x4b, "BIT 1, E",     2,  8,  None),
    Instr::new(0x4c, "BIT 1, H",     2,  8,  None),
    Instr::new(0x4d, "BIT 1, L",     2,  8,  None),
    Instr::new(0x4e, "BIT 1, (HL)",  2,  16, None),
    Instr::new(0x4f, "BIT 1, A",     2,  8,  None),

    Instr::new(0x50, "BIT 2, B",     2,  8,  None),
    Instr::new(0x51, "BIT 2, C",     2,  8,  None),
    Instr::new(0x52, "BIT 2, D",     2,  8,  None),
    Instr::new(0x53, "BIT 2, E",     2,  8,  None),
    Instr::new(0x54, "BIT 2, H",     2,  8,  None),
    Instr::new(0x55, "BIT 2, L",     2,  8,  None),
    Instr::new(0x56, "BIT 2, (HL)",  2,  16, None),
    Instr::new(0x57, "BIT 2, A",     2,  8,  None),
    Instr::new(0x58, "BIT 3, B",     2,  8,  None),
    Instr::new(0x59, "BIT 3, C",     2,  8,  None),
    Instr::new(0x5a, "BIT 3, D",     2,  8,  None),
    Instr::new(0x5b, "BIT 3, E",     2,  8,  None),
    Instr::new(0x5c, "BIT 3, H",     2,  8,  None),
    Instr::new(0x5d, "BIT 3, L",     2,  8,  None),
    Instr::new(0x5e, "BIT 3, (HL)",  2,  16, None),
    Instr::new(0x5f, "BIT 3, A",     2,  8,  None),

    Instr::new(0x60, "BIT 4, B",     2,  8,  None),
    Instr::new(0x61, "BIT 4, C",     2,  8,  None),
    Instr::new(0x62, "BIT 4, D",     2,  8,  None),
    Instr::new(0x63, "BIT 4, E",     2,  8,  None),
    Instr::new(0x64, "BIT 4, H",     2,  8,  None),
    Instr::new(0x65, "BIT 4, L",     2,  8,  None),
    Instr::new(0x66, "BIT 4, (HL)",  2,  16, None),
    Instr::new(0x67, "BIT 4, A",     2,  8,  None),
    Instr::new(0x68, "BIT 5, B",     2,  8,  None),
    Instr::new(0x69, "BIT 5, C",     2,  8,  None),
    Instr::new(0x6a, "BIT 5, D",     2,  8,  None),
    Instr::new(0x6b, "BIT 5, E",     2,  8,  None),
    Instr::new(0x6c, "BIT 5, H",     2,  8,  None),
    Instr::new(0x6d, "BIT 5, L",     2,  8,  None),
    Instr::new(0x6e, "BIT 5, (HL)",  2,  16, None),
    Instr::new(0x6f, "BIT 5, A",     2,  8,  None),

    Instr::new(0x70, "BIT 6, B",     2,  8,  None),
    Instr::new(0x71, "BIT 6, C",     2,  8,  None),
    Instr::new(0x72, "BIT 6, D",     2,  8,  None),
    Instr::new(0x73, "BIT 6, E",     2,  8,  None),
    Instr::new(0x74, "BIT 6, H",     2,  8,  None),
    Instr::new(0x75, "BIT 6, L",     2,  8,  None),
    Instr::new(0x76, "BIT 6, (HL)",  2,  16, None),
    Instr::new(0x77, "BIT 6, A",     2,  8,  None),
    Instr::new(0x78, "BIT 7, B",     2,  8,  None),
    Instr::new(0x79, "BIT 7, C",     2,  8,  None),
    Instr::new(0x7a, "BIT 7, D",     2,  8,  None),
    Instr::new(0x7b, "BIT 7, E",     2,  8,  None),
    Instr::new(0x7c, "BIT 7, H",     2,  8,  None),
    Instr::new(0x7d, "BIT 7, L",     2,  8,  None),
    Instr::new(0x7e, "BIT 7, (HL)",  2,  16, None),
    Instr::new(0x7f, "BIT 7, A",     2,  8,  None),

    Instr::new(0x80, "RES 0, B",     2,  8,  None),
    Instr::new(0x81, "RES 0, C",     2,  8,  None),
    Instr::new(0x82, "RES 0, D",     2,  8,  None),
    Instr::new(0x83, "RES 0, E",     2,  8,  None),
    Instr::new(0x84, "RES 0, H",     2,  8,  None),
    Instr::new(0x85, "RES 0, L",     2,  8,  None),
    Instr::new(0x86, "RES 0, (HL)",  2,  16, None),
    Instr::new(0x87, "RES 0, A",     2,  8,  None),
    Instr::new(0x88, "RES 1, B",     2,  8,  None),
    Instr::new(0x89, "RES 1, C",     2,  8,  None),
    Instr::new(0x8a, "RES 1, D",     2,  8,  None),
    Instr::new(0x8b, "RES 1, E",     2,  8,  None),
    Instr::new(0x8c, "RES 1, H",     2,  8,  None),
    Instr::new(0x8d, "RES 1, L",     2,  8,  None),
    Instr::new(0x8e, "RES 1, (HL)",  2,  16, None),
    Instr::new(0x8f, "RES 1, A",     2,  8,  None),

    Instr::new(0x90, "RES 2, B",     2,  8,  None),
    Instr::new(0x91, "RES 2, C",     2,  8,  None),
    Instr::new(0x92, "RES 2, D",     2,  8,  None),
    Instr::new(0x93, "RES 2, E",     2,  8,  None),
    Instr::new(0x94, "RES 2, H",     2,  8,  None),
    Instr::new(0x95, "RES 2, L",     2,  8,  None),
    Instr::new(0x96, "RES 2, (HL)",  2,  16, None),
    Instr::new(0x97, "RES 2, A",     2,  8,  None),
    Instr::new(0x98, "RES 3, B",     2,  8,  None),
    Instr::new(0x99, "RES 3, C",     2,  8,  None),
    Instr::new(0x9a, "RES 3, D",     2,  8,  None),
    Instr::new(0x9b, "RES 3, E",     2,  8,  None),
    Instr::new(0x9c, "RES 3, H",     2,  8,  None),
    Instr::new(0x9d, "RES 3, L",     2,  8,  None),
    Instr::new(0x9e, "RES 3, (HL)",  2,  16, None),
    Instr::new(0x9f, "RES 3, A",     2,  8,  None),

    Instr::new(0xa0, "RES 4, B",     2,  8,  None),
    Instr::new(0xa1, "RES 4, C",     2,  8,  None),
    Instr::new(0xa2, "RES 4, D",     2,  8,  None),
    Instr::new(0xa3, "RES 4, E",     2,  8,  None),
    Instr::new(0xa4, "RES 4, H",     2,  8,  None),
    Instr::new(0xa5, "RES 4, L",     2,  8,  None),
    Instr::new(0xa6, "RES 4, (HL)",  2,  16, None),
    Instr::new(0xa7, "RES 4, A",     2,  8,  None),
    Instr::new(0xa8, "RES 5, B",     2,  8,  None),
    Instr::new(0xa9, "RES 5, C",     2,  8,  None),
    Instr::new(0xaa, "RES 5, D",     2,  8,  None),
    Instr::new(0xab, "RES 5, E",     2,  8,  None),
    Instr::new(0xac, "RES 5, H",     2,  8,  None),
    Instr::new(0xad, "RES 5, L",     2,  8,  None),
    Instr::new(0xae, "RES 5, (HL)",  2,  16, None),
    Instr::new(0xaf, "RES 5, A",     2,  8,  None),

    Instr::new(0xb0, "RES 6, B",     2,  8,  None),
    Instr::new(0xb1, "RES 6, C",     2,  8,  None),
    Instr::new(0xb2, "RES 6, D",     2,  8,  None),
    Instr::new(0xb3, "RES 6, E",     2,  8,  None),
    Instr::new(0xb4, "RES 6, H",     2,  8,  None),
    Instr::new(0xb5, "RES 6, L",     2,  8,  None),
    Instr::new(0xb6, "RES 6, (HL)",  2,  16, None),
    Instr::new(0xb7, "RES 6, A",     2,  8,  None),
    Instr::new(0xb8, "RES 7, B",     2,  8,  None),
    Instr::new(0xb9, "RES 7, C",     2,  8,  None),
    Instr::new(0xba, "RES 7, D",     2,  8,  None),
    Instr::new(0xbb, "RES 7, E",     2,  8,  None),
    Instr::new(0xbc, "RES 7, H",     2,  8,  None),
    Instr::new(0xbd, "RES 7, L",     2,  8,  None),
    Instr::new(0xbe, "RES 7, (HL)",  2,  16, None),
    Instr::new(0xbf, "RES 7, A",     2,  8,  None),

    Instr::new(0xc0, "SET 0, B",     2,  8,  None),
    Instr::new(0xc1, "SET 0, C",     2,  8,  None),
    Instr::new(0xc2, "SET 0, D",     2,  8,  None),
    Instr::new(0xc3, "SET 0, E",     2,  8,  None),
    Instr::new(0xc4, "SET 0, H",     2,  8,  None),
    Instr::new(0xc5, "SET 0, L",     2,  8,  None),
    Instr::new(0xc6, "SET 0, (HL)",  2,  16, None),
    Instr::new(0xc7, "SET 0, A",     2,  8,  None),
    Instr::new(0xc8, "SET 1, B",     2,  8,  None),
    Instr::new(0xc9, "SET 1, C",     2,  8,  None),
    Instr::new(0xca, "SET 1, D",     2,  8,  None),
    Instr::new(0xcb, "SET 1, E",     2,  8,  None),
    Instr::new(0xcc, "SET 1, H",     2,  8,  None),
    Instr::new(0xcd, "SET 1, L",     2,  8,  None),
    Instr::new(0xce, "SET 1, (HL)",  2,  16, None),
    Instr::new(0xcf, "SET 1, A",     2,  8,  None),

    Instr::new(0xd0, "SET 2, B",     2,  8,  None),
    Instr::new(0xd1, "SET 2, C",     2,  8,  None),
    Instr::new(0xd2, "SET 2, D",     2,  8,  None),
    Instr::new(0xd3, "SET 2, E",     2,  8,  None),
    Instr::new(0xd4, "SET 2, H",     2,  8,  None),
    Instr::new(0xd5, "SET 2, L",     2,  8,  None),
    Instr::new(0xd6, "SET 2, (HL)",  2,  16, None),
    Instr::new(0xd7, "SET 2, A",     2,  8,  None),
    Instr::new(0xd8, "SET 3, B",     2,  8,  None),
    Instr::new(0xd9, "SET 3, C",     2,  8,  None),
    Instr::new(0xda, "SET 3, D",     2,  8,  None),
    Instr::new(0xdb, "SET 3, E",     2,  8,  None),
    Instr::new(0xdc, "SET 3, H",     2,  8,  None),
    Instr::new(0xdd, "SET 3, L",     2,  8,  None),
    Instr::new(0xde, "SET 3, (HL)",  2,  16, None),
    Instr::new(0xdf, "SET 3, A",     2,  8,  None),

    Instr::new(0xe0, "SET 4, B",     2,  8,  None),
    Instr::new(0xe1, "SET 4, C",     2,  8,  None),
    Instr::new(0xe2, "SET 4, D",     2,  8,  None),
    Instr::new(0xe3, "SET 4, E",     2,  8,  None),
    Instr::new(0xe4, "SET 4, H",     2,  8,  None),
    Instr::new(0xe5, "SET 4, L",     2,  8,  None),
    Instr::new(0xe6, "SET 4, (HL)",  2,  16, None),
    Instr::new(0xe7, "SET 4, A",     2,  8,  None),
    Instr::new(0xe8, "SET 5, B",     2,  8,  None),
    Instr::new(0xe9, "SET 5, C",     2,  8,  None),
    Instr::new(0xea, "SET 5, D",     2,  8,  None),
    Instr::new(0xeb, "SET 5, E",     2,  8,  None),
    Instr::new(0xec, "SET 5, H",     2,  8,  None),
    Instr::new(0xed, "SET 5, L",     2,  8,  None),
    Instr::new(0xee, "SET 5, (HL)",  2,  16, None),
    Instr::new(0xef, "SET 5, A",     2,  8,  None),

    Instr::new(0xf0, "SET 6, B",     2,  8,  None),
    Instr::new(0xf1, "SET 6, C",     2,  8,  None),
    Instr::new(0xf2, "SET 6, D",     2,  8,  None),
    Instr::new(0xf3, "SET 6, E",     2,  8,  None),
    Instr::new(0xf4, "SET 6, H",     2,  8,  None),
    Instr::new(0xf5, "SET 6, L",     2,  8,  None),
    Instr::new(0xf6, "SET 6, (HL)",  2,  16, None),
    Instr::new(0xf7, "SET 6, A",     2,  8,  None),
    Instr::new(0xf8, "SET 7, B",     2,  8,  None),
    Instr::new(0xf9, "SET 7, C",     2,  8,  None),
    Instr::new(0xfa, "SET 7, D",     2,  8,  None),
    Instr::new(0xfb, "SET 7, E",     2,  8,  None),
    Instr::new(0xfc, "SET 7, H",     2,  8,  None),
    Instr::new(0xfd, "SET 7, L",     2,  8,  None),
    Instr::new(0xfe, "SET 7, (HL)",  2,  16, None),
    Instr::new(0xff, "SET 7, A",     2,  8,  None),
]);

#[macro_export]
macro_rules! opcode {
    ("NOP") => { 0x00 };
    ("LD BC, d16") => { 0x01 };
    ("LD (BC), A") => { 0x02 };
    ("INC BC") => { 0x03 };
    ("INC B") => { 0x04 };
    ("DEC B") => { 0x05 };
    ("LD B, d8") => { 0x06 };
    ("RLCA") => { 0x07 };
    ("LD (a16), SP") => { 0x08 };
    ("ADD HL, BC") => { 0x09 };
    ("LD A, (BC)") => { 0x0a };
    ("DEC BC") => { 0x0b };
    ("INC C") => { 0x0c };
    ("DEC C") => { 0x0d };
    ("LD C, d8") => { 0x0e };
    ("RRCA") => { 0x0f };

    ("STOP") => { 0x10 };
    ("LD DE, d16") => { 0x11 };
    ("LD (DE), A") => { 0x12 };
    ("INC DE") => { 0x13 };
    ("INC D") => { 0x14 };
    ("DEC D") => { 0x15 };
    ("LD D, d8") => { 0x16 };
    ("RLA") => { 0x17 };
    ("JR r8") => { 0x18 };
    ("ADD HL, DE") => { 0x19 };
    ("LD A, (DE)") => { 0x1a };
    ("DEC DE") => { 0x1b };
    ("INC E") => { 0x1c };
    ("DEC E") => { 0x1d };
    ("LD E, d8") => { 0x1e };
    ("RRA") => { 0x1f };

    ("JR NZ, r8") => { 0x20 };
    ("LD HL, d16") => { 0x21 };
    ("LD (HL+), A") => { 0x22 };
    ("INC HL") => { 0x23 };
    ("INC H") => { 0x24 };
    ("DEC H") => { 0x25 };
    ("LD H, d8") => { 0x26 };
    ("DAA") => { 0x27 };
    ("JR Z, r8") => { 0x28 };
    ("ADD HL, HL") => { 0x29 };
    ("LD A, (HL+)") => { 0x2a };
    ("DEC HL") => { 0x2b };
    ("INC L") => { 0x2c };
    ("DEC L") => { 0x2d };
    ("LD L, d8") => { 0x2e };
    ("CPL") => { 0x2f };

    ("JR NC, r8") => { 0x30 };
    ("LD SP, d16") => { 0x31 };
    ("LD (HL-), A") => { 0x32 };
    ("INC SP") => { 0x33 };
    ("INC (HL)") => { 0x34 };
    ("DEC (HL)") => { 0x35 };
    ("LD (HL), d8") => { 0x36 };
    ("SCF") => { 0x37 };
    ("JR C, r8") => { 0x38 };
    ("ADD HL, SP") => { 0x39 };
    ("LD A, (HL-)") => { 0x3a };
    ("DEC SP") => { 0x3b };
    ("INC A") => { 0x3c };
    ("DEC A") => { 0x3d };
    ("LD A, d8") => { 0x3e };
    ("CCF") => { 0x3f };

    ("LD B, B") => { 0x40 };
    ("LD B, C") => { 0x41 };
    ("LD B, D") => { 0x42 };
    ("LD B, E") => { 0x43 };
    ("LD B, H") => { 0x44 };
    ("LD B, L") => { 0x45 };
    ("LD B, (HL)") => { 0x46 };
    ("LD B, A") => { 0x47 };
    ("LD C, B") => { 0x48 };
    ("LD C, C") => { 0x49 };
    ("LD C, D") => { 0x4a };
    ("LD C, E") => { 0x4b };
    ("LD C, H") => { 0x4c };
    ("LD C, L") => { 0x4d };
    ("LD C, (HL)") => { 0x4e };
    ("LD C, A") => { 0x4f };

    ("LD D, B") => { 0x50 };
    ("LD D, C") => { 0x51 };
    ("LD D, D") => { 0x52 };
    ("LD D, E") => { 0x53 };
    ("LD D, H") => { 0x54 };
    ("LD D, L") => { 0x55 };
    ("LD D, (HL)") => { 0x56 };
    ("LD D, A") => { 0x57 };
    ("LD E, B") => { 0x58 };
    ("LD E, C") => { 0x59 };
    ("LD E, D") => { 0x5a };
    ("LD E, E") => { 0x5b };
    ("LD E, H") => { 0x5c };
    ("LD E, L") => { 0x5d };
    ("LD E, (HL)") => { 0x5e };
    ("LD E, A") => { 0x5f };

    ("LD H, B") => { 0x60 };
    ("LD H, C") => { 0x61 };
    ("LD H, D") => { 0x62 };
    ("LD H, E") => { 0x63 };
    ("LD H, H") => { 0x64 };
    ("LD H, L") => { 0x65 };
    ("LD H, (HL)") => { 0x66 };
    ("LD H, A") => { 0x67 };
    ("LD L, B") => { 0x68 };
    ("LD L, C") => { 0x69 };
    ("LD L, D") => { 0x6a };
    ("LD L, E") => { 0x6b };
    ("LD L, H") => { 0x6c };
    ("LD L, L") => { 0x6d };
    ("LD L, (HL)") => { 0x6e };
    ("LD L, A") => { 0x6f };

    ("LD (HL), B") => { 0x70 };
    ("LD (HL), C") => { 0x71 };
    ("LD (HL), D") => { 0x72 };
    ("LD (HL), E") => { 0x73 };
    ("LD (HL), H") => { 0x74 };
    ("LD (HL), L") => { 0x75 };
    ("HALT") => { 0x76 };
    ("LD (HL), A") => { 0x77 };
    ("LD A, B") => { 0x78 };
    ("LD A, C") => { 0x79 };
    ("LD A, D") => { 0x7a };
    ("LD A, E") => { 0x7b };
    ("LD A, H") => { 0x7c };
    ("LD A, L") => { 0x7d };
    ("LD A, (HL)") => { 0x7e };
    ("LD A, A") => { 0x7f };

    ("ADD A, B") => { 0x80 };
    ("ADD A, C") => { 0x81 };
    ("ADD A, D") => { 0x82 };
    ("ADD A, E") => { 0x83 };
    ("ADD A, H") => { 0x84 };
    ("ADD A, L") => { 0x85 };
    ("ADD A, (HL)") => { 0x86 };
    ("ADD A, A") => { 0x87 };
    ("ADC A, B") => { 0x88 };
    ("ADC A, C") => { 0x89 };
    ("ADC A, D") => { 0x8a };
    ("ADC A, E") => { 0x8b };
    ("ADC A, H") => { 0x8c };
    ("ADC A, L") => { 0x8d };
    ("ADC A, (HL)") => { 0x8e };
    ("ADC A, A") => { 0x8f };

    ("SUB B") => { 0x90 };
    ("SUB C") => { 0x91 };
    ("SUB D") => { 0x92 };
    ("SUB E") => { 0x93 };
    ("SUB H") => { 0x94 };
    ("SUB L") => { 0x95 };
    ("SUB (HL)") => { 0x96 };
    ("SUB A") => { 0x97 };
    ("SBC A, B") => { 0x98 };
    ("SBC A, C") => { 0x99 };
    ("SBC A, D") => { 0x9a };
    ("SBC A, E") => { 0x9b };
    ("SBC A, H") => { 0x9c };
    ("SBC A, L") => { 0x9d };
    ("SBC A, (HL)") => { 0x9e };
    ("SBC A, A") => { 0x9f };

    ("AND B") => { 0xa0 };
    ("AND C") => { 0xa1 };
    ("AND D") => { 0xa2 };
    ("AND E") => { 0xa3 };
    ("AND H") => { 0xa4 };
    ("AND L") => { 0xa5 };
    ("AND (HL)") => { 0xa6 };
    ("AND A") => { 0xa7 };
    ("XOR B") => { 0xa8 };
    ("XOR C") => { 0xa9 };
    ("XOR D") => { 0xaa };
    ("XOR E") => { 0xab };
    ("XOR H") => { 0xac };
    ("XOR L") => { 0xad };
    ("XOR (HL)") => { 0xae };
    ("XOR A") => { 0xaf };

    ("OR B") => { 0xb0 };
    ("OR C") => { 0xb1 };
    ("OR D") => { 0xb2 };
    ("OR E") => { 0xb3 };
    ("OR H") => { 0xb4 };
    ("OR L") => { 0xb5 };
    ("OR (HL)") => { 0xb6 };
    ("OR A") => { 0xb7 };
    ("CP B") => { 0xb8 };
    ("CP C") => { 0xb9 };
    ("CP D") => { 0xba };
    ("CP E") => { 0xbb };
    ("CP H") => { 0xbc };
    ("CP L") => { 0xbd };
    ("CP (HL)") => { 0xbe };
    ("CP A") => { 0xbf };

    ("RET NZ") => { 0xc0 };
    ("POP BC") => { 0xc1 };
    ("JP NZ, a16") => { 0xc2 };
    ("JP a16") => { 0xc3 };
    ("CALL NZ, a16") => { 0xc4 };
    ("PUSH BC") => { 0xc5 };
    ("ADD A, d8") => { 0xc6 };
    ("RST 00H") => { 0xc7 };
    ("RET Z") => { 0xc8 };
    ("RET") => { 0xc9 };
    ("JP Z, a16") => { 0xca };
    ("PREFIX CB") => { 0xcb };
    ("CALL Z, a16") => { 0xcc };
    ("CALL a16") => { 0xcd };
    ("ADC A, d8") => { 0xce };
    ("RST 08H") => { 0xcf };

    ("RET NC") => { 0xd0 };
    ("POP DE") => { 0xd1 };
    ("JP NC, a16") => { 0xd2 };
    ("CALL NC, a16") => { 0xd4 };
    ("PUSH DE") => { 0xd5 };
    ("SUB d8") => { 0xd6 };
    ("RST 10H") => { 0xd7 };
    ("RET C") => { 0xd8 };
    ("RETI") => { 0xd9 };
    ("JP C, a16") => { 0xda };
    ("CALL C, a16") => { 0xdc };
    ("SBC A, d8") => { 0xde };
    ("RST 18H") => { 0xdf };

    ("LDH (a8), A") => { 0xe0 };
    ("POP HL") => { 0xe1 };
    ("LD (C), A") => { 0xe2 };
    ("PUSH HL") => { 0xe5 };
    ("AND d8") => { 0xe6 };
    ("RST 20H") => { 0xe7 };
    ("ADD SP, r8") => { 0xe8 };
    ("JP HL") => { 0xe9 };
    ("LD (a16), A") => { 0xea };
    ("XOR d8") => { 0xee };
    ("RST 28H") => { 0xef };

    ("LDH A, (a8)") => { 0xf0 };
    ("POP AF") => { 0xf1 };
    ("LD A, (C)") => { 0xf2 };
    ("DI") => { 0xf3 };
    ("PUSH AF") => { 0xf5 };
    ("OR d8") => { 0xf6 };
    ("RST 30H") => { 0xf7 };
    ("LD HL, SP+r8") => { 0xf8 };
    ("LD SP, HL") => { 0xf9 };
    ("LD A, (a16)") => { 0xfa };
    ("EI") => { 0xfb };
    ("CP d8") => { 0xfe };
    ("RST 38H") => { 0xff };
}

#[macro_export]
macro_rules! prefixed_opcode {
    ("RLC B") => { 0x00 };
    ("RLC C") => { 0x01 };
    ("RLC D") => { 0x02 };
    ("RLC E") => { 0x03 };
    ("RLC H") => { 0x04 };
    ("RLC L") => { 0x05 };
    ("RLC (HL)") => { 0x06 };
    ("RLC A") => { 0x07 };
    ("RRC B") => { 0x08 };
    ("RRC C") => { 0x09 };
    ("RRC D") => { 0x0a };
    ("RRC E") => { 0x0b };
    ("RRC H") => { 0x0c };
    ("RRC L") => { 0x0d };
    ("RRC (HL)") => { 0x0e };
    ("RRC A") => { 0x0f };

    ("RL B") => { 0x10 };
    ("RL C") => { 0x11 };
    ("RL D") => { 0x12 };
    ("RL E") => { 0x13 };
    ("RL H") => { 0x14 };
    ("RL L") => { 0x15 };
    ("RL (HL)") => { 0x16 };
    ("RL A") => { 0x17 };
    ("RR B") => { 0x18 };
    ("RR C") => { 0x19 };
    ("RR D") => { 0x1a };
    ("RR E") => { 0x1b };
    ("RR H") => { 0x1c };
    ("RR L") => { 0x1d };
    ("RR (HL)") => { 0x1e };
    ("RR A") => { 0x1f };

    ("SLA B") => { 0x20 };
    ("SLA C") => { 0x21 };
    ("SLA D") => { 0x22 };
    ("SLA E") => { 0x23 };
    ("SLA H") => { 0x24 };
    ("SLA L") => { 0x25 };
    ("SLA (HL)") => { 0x26 };
    ("SLA A") => { 0x27 };
    ("SRA B") => { 0x28 };
    ("SRA C") => { 0x29 };
    ("SRA D") => { 0x2a };
    ("SRA E") => { 0x2b };
    ("SRA H") => { 0x2c };
    ("SRA L") => { 0x2d };
    ("SRA (HL)") => { 0x2e };
    ("SRA A") => { 0x2f };

    ("SWAP B") => { 0x30 };
    ("SWAP C") => { 0x31 };
    ("SWAP D") => { 0x32 };
    ("SWAP E") => { 0x33 };
    ("SWAP H") => { 0x34 };
    ("SWAP L") => { 0x35 };
    ("SWAP (HL)") => { 0x36 };
    ("SWAP A") => { 0x37 };
    ("SRL B") => { 0x38 };
    ("SRL C") => { 0x39 };
    ("SRL D") => { 0x3a };
    ("SRL E") => { 0x3b };
    ("SRL H") => { 0x3c };
    ("SRL L") => { 0x3d };
    ("SRL (HL)") => { 0x3e };
    ("SRL A") => { 0x3f };

    ("BIT 0, B") => { 0x40 };
    ("BIT 0, C") => { 0x41 };
    ("BIT 0, D") => { 0x42 };
    ("BIT 0, E") => { 0x43 };
    ("BIT 0, H") => { 0x44 };
    ("BIT 0, L") => { 0x45 };
    ("BIT 0, (HL)") => { 0x46 };
    ("BIT 0, A") => { 0x47 };
    ("BIT 1, B") => { 0x48 };
    ("BIT 1, C") => { 0x49 };
    ("BIT 1, D") => { 0x4a };
    ("BIT 1, E") => { 0x4b };
    ("BIT 1, H") => { 0x4c };
    ("BIT 1, L") => { 0x4d };
    ("BIT 1, (HL)") => { 0x4e };
    ("BIT 1, A") => { 0x4f };

    ("BIT 2, B") => { 0x50 };
    ("BIT 2, C") => { 0x51 };
    ("BIT 2, D") => { 0x52 };
    ("BIT 2, E") => { 0x53 };
    ("BIT 2, H") => { 0x54 };
    ("BIT 2, L") => { 0x55 };
    ("BIT 2, (HL)") => { 0x56 };
    ("BIT 2, A") => { 0x57 };
    ("BIT 3, B") => { 0x58 };
    ("BIT 3, C") => { 0x59 };
    ("BIT 3, D") => { 0x5a };
    ("BIT 3, E") => { 0x5b };
    ("BIT 3, H") => { 0x5c };
    ("BIT 3, L") => { 0x5d };
    ("BIT 3, (HL)") => { 0x5e };
    ("BIT 3, A") => { 0x5f };

    ("BIT 4, B") => { 0x60 };
    ("BIT 4, C") => { 0x61 };
    ("BIT 4, D") => { 0x62 };
    ("BIT 4, E") => { 0x63 };
    ("BIT 4, H") => { 0x64 };
    ("BIT 4, L") => { 0x65 };
    ("BIT 4, (HL)") => { 0x66 };
    ("BIT 4, A") => { 0x67 };
    ("BIT 5, B") => { 0x68 };
    ("BIT 5, C") => { 0x69 };
    ("BIT 5, D") => { 0x6a };
    ("BIT 5, E") => { 0x6b };
    ("BIT 5, H") => { 0x6c };
    ("BIT 5, L") => { 0x6d };
    ("BIT 5, (HL)") => { 0x6e };
    ("BIT 5, A") => { 0x6f };

    ("BIT 6, B") => { 0x70 };
    ("BIT 6, C") => { 0x71 };
    ("BIT 6, D") => { 0x72 };
    ("BIT 6, E") => { 0x73 };
    ("BIT 6, H") => { 0x74 };
    ("BIT 6, L") => { 0x75 };
    ("BIT 6, (HL)") => { 0x76 };
    ("BIT 6, A") => { 0x77 };
    ("BIT 7, B") => { 0x78 };
    ("BIT 7, C") => { 0x79 };
    ("BIT 7, D") => { 0x7a };
    ("BIT 7, E") => { 0x7b };
    ("BIT 7, H") => { 0x7c };
    ("BIT 7, L") => { 0x7d };
    ("BIT 7, (HL)") => { 0x7e };
    ("BIT 7, A") => { 0x7f };

    ("RES 0, B") => { 0x80 };
    ("RES 0, C") => { 0x81 };
    ("RES 0, D") => { 0x82 };
    ("RES 0, E") => { 0x83 };
    ("RES 0, H") => { 0x84 };
    ("RES 0, L") => { 0x85 };
    ("RES 0, (HL)") => { 0x86 };
    ("RES 0, A") => { 0x87 };
    ("RES 1, B") => { 0x88 };
    ("RES 1, C") => { 0x89 };
    ("RES 1, D") => { 0x8a };
    ("RES 1, E") => { 0x8b };
    ("RES 1, H") => { 0x8c };
    ("RES 1, L") => { 0x8d };
    ("RES 1, (HL)") => { 0x8e };
    ("RES 1, A") => { 0x8f };

    ("RES 2, B") => { 0x90 };
    ("RES 2, C") => { 0x91 };
    ("RES 2, D") => { 0x92 };
    ("RES 2, E") => { 0x93 };
    ("RES 2, H") => { 0x94 };
    ("RES 2, L") => { 0x95 };
    ("RES 2, (HL)") => { 0x96 };
    ("RES 2, A") => { 0x97 };
    ("RES 3, B") => { 0x98 };
    ("RES 3, C") => { 0x99 };
    ("RES 3, D") => { 0x9a };
    ("RES 3, E") => { 0x9b };
    ("RES 3, H") => { 0x9c };
    ("RES 3, L") => { 0x9d };
    ("RES 3, (HL)") => { 0x9e };
    ("RES 3, A") => { 0x9f };

    ("RES 4, B") => { 0xa0 };
    ("RES 4, C") => { 0xa1 };
    ("RES 4, D") => { 0xa2 };
    ("RES 4, E") => { 0xa3 };
    ("RES 4, H") => { 0xa4 };
    ("RES 4, L") => { 0xa5 };
    ("RES 4, (HL)") => { 0xa6 };
    ("RES 4, A") => { 0xa7 };
    ("RES 5, B") => { 0xa8 };
    ("RES 5, C") => { 0xa9 };
    ("RES 5, D") => { 0xaa };
    ("RES 5, E") => { 0xab };
    ("RES 5, H") => { 0xac };
    ("RES 5, L") => { 0xad };
    ("RES 5, (HL)") => { 0xae };
    ("RES 5, A") => { 0xaf };

    ("RES 6, B") => { 0xb0 };
    ("RES 6, C") => { 0xb1 };
    ("RES 6, D") => { 0xb2 };
    ("RES 6, E") => { 0xb3 };
    ("RES 6, H") => { 0xb4 };
    ("RES 6, L") => { 0xb5 };
    ("RES 6, (HL)") => { 0xb6 };
    ("RES 6, A") => { 0xb7 };
    ("RES 7, B") => { 0xb8 };
    ("RES 7, C") => { 0xb9 };
    ("RES 7, D") => { 0xba };
    ("RES 7, E") => { 0xbb };
    ("RES 7, H") => { 0xbc };
    ("RES 7, L") => { 0xbd };
    ("RES 7, (HL)") => { 0xbe };
    ("RES 7, A") => { 0xbf };

    ("SET 0, B") => { 0xc0 };
    ("SET 0, C") => { 0xc1 };
    ("SET 0, D") => { 0xc2 };
    ("SET 0, E") => { 0xc3 };
    ("SET 0, H") => { 0xc4 };
    ("SET 0, L") => { 0xc5 };
    ("SET 0, (HL)") => { 0xc6 };
    ("SET 0, A") => { 0xc7 };
    ("SET 1, B") => { 0xc8 };
    ("SET 1, C") => { 0xc9 };
    ("SET 1, D") => { 0xca };
    ("SET 1, E") => { 0xcb };
    ("SET 1, H") => { 0xcc };
    ("SET 1, L") => { 0xcd };
    ("SET 1, (HL)") => { 0xce };
    ("SET 1, A") => { 0xcf };

    ("SET 2, B") => { 0xd0 };
    ("SET 2, C") => { 0xd1 };
    ("SET 2, D") => { 0xd2 };
    ("SET 2, E") => { 0xd3 };
    ("SET 2, H") => { 0xd4 };
    ("SET 2, L") => { 0xd5 };
    ("SET 2, (HL)") => { 0xd6 };
    ("SET 2, A") => { 0xd7 };
    ("SET 3, B") => { 0xd8 };
    ("SET 3, C") => { 0xd9 };
    ("SET 3, D") => { 0xda };
    ("SET 3, E") => { 0xdb };
    ("SET 3, H") => { 0xdc };
    ("SET 3, L") => { 0xdd };
    ("SET 3, (HL)") => { 0xde };
    ("SET 3, A") => { 0xdf };

    ("SET 4, B") => { 0xe0 };
    ("SET 4, C") => { 0xe1 };
    ("SET 4, D") => { 0xe2 };
    ("SET 4, E") => { 0xe3 };
    ("SET 4, H") => { 0xe4 };
    ("SET 4, L") => { 0xe5 };
    ("SET 4, (HL)") => { 0xe6 };
    ("SET 4, A") => { 0xe7 };
    ("SET 5, B") => { 0xe8 };
    ("SET 5, C") => { 0xe9 };
    ("SET 5, D") => { 0xea };
    ("SET 5, E") => { 0xeb };
    ("SET 5, H") => { 0xec };
    ("SET 5, L") => { 0xed };
    ("SET 5, (HL)") => { 0xee };
    ("SET 5, A") => { 0xef };

    ("SET 6, B") => { 0xf0 };
    ("SET 6, C") => { 0xf1 };
    ("SET 6, D") => { 0xf2 };
    ("SET 6, E") => { 0xf3 };
    ("SET 6, H") => { 0xf4 };
    ("SET 6, L") => { 0xf5 };
    ("SET 6, (HL)") => { 0xf6 };
    ("SET 6, A") => { 0xf7 };
    ("SET 7, B") => { 0xf8 };
    ("SET 7, C") => { 0xf9 };
    ("SET 7, D") => { 0xfa };
    ("SET 7, E") => { 0xfb };
    ("SET 7, H") => { 0xfc };
    ("SET 7, L") => { 0xfd };
    ("SET 7, (HL)") => { 0xfe };
    ("SET 7, A") => { 0xff };
}
