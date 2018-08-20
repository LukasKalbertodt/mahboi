//! Instruction data.
//!
//! This module contains some basic information like size and cycle count for
//! all instructions. It is stored in two 256-element long arrays -- one for
//! the main instructions and one for all PREFIX CB instructions.

/// The information we store per instruction.
#[derive(Debug, Clone, Copy)]
pub struct Instr {
    /// Full mnemonic.
    ///
    /// This includes the arguments with several placeholders for values:
    /// - `d8`: immediate 8 bit data
    /// - `d16`: immediate 16 bit data
    /// - `a8`: 8 bit unsigned value that is added to `$FF00`
    /// - `a16`: 16 bit address
    /// - `r8`: 8 bit signed value which is added to PC
    pub mnemonic: &'static str,

    /// Kind of instruction (e.g. ADD, LD, INC)
    pub kind: &'static str,

    /// List of params for instruction with placeholders for values
    pub params: &'static str,

    /// Length in bytes
    pub len: u8,

    /// CPU cylces
    pub cycles: u8,

    /// CPU cylces, if branch is taken.
    ///
    /// This is only set for instructions that have to decide something and
    /// only sometimes perform an operation (conditional jumps, mostly). If the
    /// branch/action is not taken, `cycles` is the correct value.
    pub cycles_taken: Option<u8>,
}

impl Instr {
    const fn new(
        mnemonic: &'static str,
        kind: &'static str,
        params: &'static str,
        len: u8,
        cycles: u8,
        cycles_taken: Option<u8>,
    ) -> Option<Self> {
        Some(Instr {
            mnemonic,
            kind,
            params,
            len,
            cycles,
            cycles_taken,
        })
    }
}

/// Placeholder for not yet implemented instructions. This way we can
/// deliberately differentiate between opcodes that are invalid (`None`) and
/// instructions we haven't added yet (`missing()`).
const fn missing() -> Option<Instr> {
    None
}

/// Main instruction data.
///
/// Entries with the value `None` are either not yet added to the list
/// (`missing`) or are invalid opcodes.
///
/// Regarding the special CB PREFIX instructions: in this array it has a len
/// and cycle number of 0. In the cheat sheet, those values are stated as 1 and
/// 4 respectively. This is wrong in a sense: the length/cycle values for all
/// CB-instructions in `PREFIXED_INSTRUCTIONS` already contains the total
/// value. The actual prefix doesn't add anything.
pub const INSTRUCTIONS: [Option<Instr>; 256] = [
    /* 00 */ Instr::new("NOP",          "NOP",  "",         1,  4,  None),
    /* 01 */ Instr::new("LD BC, d16",   "LD",   "BC, {}",   1,  4,  None),
    /* 02 */ Instr::new("LD (BC), A",   "LD",   "",         1,  4,  None),
    /* 03 */ missing(),
    /* 04 */ missing(),
    /* 05 */ missing(),
    /* 06 */ missing(),
    /* 07 */ missing(),
    /* 08 */ Instr::new("LD (a16), SP", "LD",   "",         3,  20, None),
    /* 09 */ missing(),
    /* 0a */ missing(),
    /* 0b */ missing(),
    /* 0c */ missing(),
    /* 0d */ missing(),
    /* 0e */ missing(),
    /* 0f */ missing(),

    /* 10 */ missing(),
    /* 11 */ missing(),
    /* 12 */ missing(),
    /* 13 */ missing(),
    /* 14 */ missing(),
    /* 15 */ missing(),
    /* 16 */ missing(),
    /* 17 */ missing(),
    /* 18 */ missing(),
    /* 19 */ missing(),
    /* 1a */ missing(),
    /* 1b */ missing(),
    /* 1c */ missing(),
    /* 1d */ missing(),
    /* 1e */ missing(),
    /* 1f */ missing(),

    /* 20 */ Instr::new("JR NZ, r8",    "JR",   "NZ, {}",   2,  8,  Some(12)),
    /* 21 */ Instr::new("LD HL, d16",   "LD",   "HL, {}",   3,  12, None),
    /* 22 */ missing(),
    /* 23 */ missing(),
    /* 24 */ missing(),
    /* 25 */ missing(),
    /* 26 */ missing(),
    /* 27 */ missing(),
    /* 28 */ missing(),
    /* 29 */ missing(),
    /* 2a */ missing(),
    /* 2b */ missing(),
    /* 2c */ missing(),
    /* 2d */ missing(),
    /* 2e */ missing(),
    /* 2f */ missing(),

    /* 30 */ missing(),
    /* 31 */ Instr::new("LD SP, d16",   "LD",   "SP, {}",   3,  12, None),
    /* 32 */ Instr::new("LD (HL-), A",  "LD",   "(HL-), A", 1,  8,  None),
    /* 33 */ missing(),
    /* 34 */ missing(),
    /* 35 */ missing(),
    /* 36 */ missing(),
    /* 37 */ missing(),
    /* 38 */ missing(),
    /* 39 */ missing(),
    /* 3a */ missing(),
    /* 3b */ missing(),
    /* 3c */ missing(),
    /* 3d */ missing(),
    /* 3e */ missing(),
    /* 3f */ missing(),

    /* 40 */ missing(),
    /* 41 */ missing(),
    /* 42 */ missing(),
    /* 43 */ missing(),
    /* 44 */ missing(),
    /* 45 */ missing(),
    /* 46 */ missing(),
    /* 47 */ missing(),
    /* 48 */ missing(),
    /* 49 */ missing(),
    /* 4a */ missing(),
    /* 4b */ missing(),
    /* 4c */ missing(),
    /* 4d */ missing(),
    /* 4e */ missing(),
    /* 4f */ missing(),

    /* 50 */ missing(),
    /* 51 */ missing(),
    /* 52 */ missing(),
    /* 53 */ missing(),
    /* 54 */ missing(),
    /* 55 */ missing(),
    /* 56 */ missing(),
    /* 57 */ missing(),
    /* 58 */ missing(),
    /* 59 */ missing(),
    /* 5a */ missing(),
    /* 5b */ missing(),
    /* 5c */ missing(),
    /* 5d */ missing(),
    /* 5e */ missing(),
    /* 5f */ missing(),

    /* 60 */ missing(),
    /* 61 */ missing(),
    /* 62 */ missing(),
    /* 63 */ missing(),
    /* 64 */ missing(),
    /* 65 */ missing(),
    /* 66 */ missing(),
    /* 67 */ missing(),
    /* 68 */ missing(),
    /* 69 */ missing(),
    /* 6a */ missing(),
    /* 6b */ missing(),
    /* 6c */ missing(),
    /* 6d */ missing(),
    /* 6e */ missing(),
    /* 6f */ missing(),

    /* 70 */ missing(),
    /* 71 */ missing(),
    /* 72 */ missing(),
    /* 73 */ missing(),
    /* 74 */ missing(),
    /* 75 */ missing(),
    /* 76 */ missing(),
    /* 77 */ missing(),
    /* 78 */ missing(),
    /* 79 */ missing(),
    /* 7a */ missing(),
    /* 7b */ missing(),
    /* 7c */ missing(),
    /* 7d */ missing(),
    /* 7e */ missing(),
    /* 7f */ missing(),

    /* 80 */ missing(),
    /* 81 */ missing(),
    /* 82 */ missing(),
    /* 83 */ missing(),
    /* 84 */ missing(),
    /* 85 */ missing(),
    /* 86 */ missing(),
    /* 87 */ missing(),
    /* 88 */ missing(),
    /* 89 */ missing(),
    /* 8a */ missing(),
    /* 8b */ missing(),
    /* 8c */ missing(),
    /* 8d */ missing(),
    /* 8e */ missing(),
    /* 8f */ missing(),

    /* 90 */ missing(),
    /* 91 */ missing(),
    /* 92 */ missing(),
    /* 93 */ missing(),
    /* 94 */ missing(),
    /* 95 */ missing(),
    /* 96 */ missing(),
    /* 97 */ missing(),
    /* 98 */ missing(),
    /* 99 */ missing(),
    /* 9a */ missing(),
    /* 9b */ missing(),
    /* 9c */ missing(),
    /* 9d */ missing(),
    /* 9e */ missing(),
    /* 9f */ missing(),

    /* a0 */ missing(),
    /* a1 */ missing(),
    /* a2 */ missing(),
    /* a3 */ missing(),
    /* a4 */ missing(),
    /* a5 */ missing(),
    /* a6 */ missing(),
    /* a7 */ missing(),
    /* a8 */ missing(),
    /* a9 */ missing(),
    /* aa */ missing(),
    /* ab */ missing(),
    /* ac */ missing(),
    /* ad */ missing(),
    /* ae */ missing(),
    /* af */ Instr::new("XOR A",  "XOR",   "A",   1,  4, None),

    /* b0 */ missing(),
    /* b1 */ missing(),
    /* b2 */ missing(),
    /* b3 */ missing(),
    /* b4 */ missing(),
    /* b5 */ missing(),
    /* b6 */ missing(),
    /* b7 */ missing(),
    /* b8 */ missing(),
    /* b9 */ missing(),
    /* ba */ missing(),
    /* bb */ missing(),
    /* bc */ missing(),
    /* bd */ missing(),
    /* be */ missing(),
    /* bf */ missing(),

    /* 00 */ missing(),
    /* 01 */ missing(),
    /* 02 */ missing(),
    /* 03 */ missing(),
    /* 04 */ missing(),
    /* 05 */ missing(),
    /* 06 */ missing(),
    /* 07 */ missing(),
    /* 08 */ missing(),
    /* 09 */ missing(),
    /* 0a */ missing(),
    /* 0b */ Instr::new("PREFIX CB",  "PREFIX",   "CB",   0,  0, None),
    /* 0c */ missing(),
    /* 0d */ missing(),
    /* 0e */ missing(),
    /* 0f */ missing(),

    /* d0 */ missing(),
    /* d1 */ missing(),
    /* d2 */ missing(),
    /* d3 */ None,
    /* d4 */ missing(),
    /* d5 */ missing(),
    /* d6 */ missing(),
    /* d7 */ missing(),
    /* d8 */ missing(),
    /* d9 */ missing(),
    /* da */ missing(),
    /* db */ None,
    /* dc */ missing(),
    /* dd */ None,
    /* de */ missing(),
    /* df */ missing(),

    /* e0 */ missing(),
    /* e1 */ missing(),
    /* e2 */ missing(),
    /* e3 */ None,
    /* e4 */ None,
    /* e5 */ missing(),
    /* e6 */ missing(),
    /* e7 */ missing(),
    /* e8 */ missing(),
    /* e9 */ missing(),
    /* ea */ missing(),
    /* eb */ None,
    /* ec */ None,
    /* ed */ None,
    /* ee */ missing(),
    /* ef */ missing(),

    /* f0 */ missing(),
    /* f1 */ missing(),
    /* f2 */ missing(),
    /* f3 */ missing(),
    /* f4 */ None,
    /* f5 */ missing(),
    /* f6 */ missing(),
    /* f7 */ missing(),
    /* f8 */ missing(),
    /* f9 */ missing(),
    /* fa */ missing(),
    /* fb */ missing(),
    /* fc */ None,
    /* fd */ None,
    /* fe */ missing(),
    /* ff */ missing(),
];

/// Instructions prefixed by CB opcode. (These opcodes are 2 bytes long.)
pub const PREFIXED_INSTRUCTIONS: [Option<Instr>; 256] = [
    /* 00 */ missing(),
    /* 01 */ missing(),
    /* 02 */ missing(),
    /* 03 */ missing(),
    /* 04 */ missing(),
    /* 05 */ missing(),
    /* 06 */ missing(),
    /* 07 */ missing(),
    /* 08 */ missing(),
    /* 09 */ missing(),
    /* 0a */ missing(),
    /* 0b */ missing(),
    /* 0c */ missing(),
    /* 0d */ missing(),
    /* 0e */ missing(),
    /* 0f */ missing(),

    /* 10 */ missing(),
    /* 11 */ missing(),
    /* 12 */ missing(),
    /* 13 */ missing(),
    /* 14 */ missing(),
    /* 15 */ missing(),
    /* 16 */ missing(),
    /* 17 */ missing(),
    /* 18 */ missing(),
    /* 19 */ missing(),
    /* 1a */ missing(),
    /* 1b */ missing(),
    /* 1c */ missing(),
    /* 1d */ missing(),
    /* 1e */ missing(),
    /* 1f */ missing(),

    /* 20 */ missing(),
    /* 21 */ missing(),
    /* 22 */ missing(),
    /* 23 */ missing(),
    /* 24 */ missing(),
    /* 25 */ missing(),
    /* 26 */ missing(),
    /* 27 */ missing(),
    /* 28 */ missing(),
    /* 29 */ missing(),
    /* 2a */ missing(),
    /* 2b */ missing(),
    /* 2c */ missing(),
    /* 2d */ missing(),
    /* 2e */ missing(),
    /* 2f */ missing(),

    /* 30 */ missing(),
    /* 31 */ missing(),
    /* 32 */ missing(),
    /* 33 */ missing(),
    /* 34 */ missing(),
    /* 35 */ missing(),
    /* 36 */ missing(),
    /* 37 */ missing(),
    /* 38 */ missing(),
    /* 39 */ missing(),
    /* 3a */ missing(),
    /* 3b */ missing(),
    /* 3c */ missing(),
    /* 3d */ missing(),
    /* 3e */ missing(),
    /* 3f */ missing(),

    /* 40 */ missing(),
    /* 41 */ missing(),
    /* 42 */ missing(),
    /* 43 */ missing(),
    /* 44 */ missing(),
    /* 45 */ missing(),
    /* 46 */ missing(),
    /* 47 */ missing(),
    /* 48 */ missing(),
    /* 49 */ missing(),
    /* 4a */ missing(),
    /* 4b */ missing(),
    /* 4c */ missing(),
    /* 4d */ missing(),
    /* 4e */ missing(),
    /* 4f */ missing(),

    /* 50 */ missing(),
    /* 51 */ missing(),
    /* 52 */ missing(),
    /* 53 */ missing(),
    /* 54 */ missing(),
    /* 55 */ missing(),
    /* 56 */ missing(),
    /* 57 */ missing(),
    /* 58 */ missing(),
    /* 59 */ missing(),
    /* 5a */ missing(),
    /* 5b */ missing(),
    /* 5c */ missing(),
    /* 5d */ missing(),
    /* 5e */ missing(),
    /* 5f */ missing(),

    /* 60 */ missing(),
    /* 61 */ missing(),
    /* 62 */ missing(),
    /* 63 */ missing(),
    /* 64 */ missing(),
    /* 65 */ missing(),
    /* 66 */ missing(),
    /* 67 */ missing(),
    /* 68 */ missing(),
    /* 69 */ missing(),
    /* 6a */ missing(),
    /* 6b */ missing(),
    /* 6c */ missing(),
    /* 6d */ missing(),
    /* 6e */ missing(),
    /* 6f */ missing(),

    /* 70 */ missing(),
    /* 71 */ missing(),
    /* 72 */ missing(),
    /* 73 */ missing(),
    /* 74 */ missing(),
    /* 75 */ missing(),
    /* 76 */ missing(),
    /* 77 */ missing(),
    /* 78 */ missing(),
    /* 79 */ missing(),
    /* 7a */ missing(),
    /* 7b */ missing(),
    /* 7c */ Instr::new("BIT 7, H",  "BIT",   "7, H",   2,  8, None),
    /* 7d */ missing(),
    /* 7e */ missing(),
    /* 7f */ missing(),

    /* 80 */ missing(),
    /* 81 */ missing(),
    /* 82 */ missing(),
    /* 83 */ missing(),
    /* 84 */ missing(),
    /* 85 */ missing(),
    /* 86 */ missing(),
    /* 87 */ missing(),
    /* 88 */ missing(),
    /* 89 */ missing(),
    /* 8a */ missing(),
    /* 8b */ missing(),
    /* 8c */ missing(),
    /* 8d */ missing(),
    /* 8e */ missing(),
    /* 8f */ missing(),

    /* 90 */ missing(),
    /* 91 */ missing(),
    /* 92 */ missing(),
    /* 93 */ missing(),
    /* 94 */ missing(),
    /* 95 */ missing(),
    /* 96 */ missing(),
    /* 97 */ missing(),
    /* 98 */ missing(),
    /* 99 */ missing(),
    /* 9a */ missing(),
    /* 9b */ missing(),
    /* 9c */ missing(),
    /* 9d */ missing(),
    /* 9e */ missing(),
    /* 9f */ missing(),

    /* a0 */ missing(),
    /* a1 */ missing(),
    /* a2 */ missing(),
    /* a3 */ missing(),
    /* a4 */ missing(),
    /* a5 */ missing(),
    /* a6 */ missing(),
    /* a7 */ missing(),
    /* a8 */ missing(),
    /* a9 */ missing(),
    /* aa */ missing(),
    /* ab */ missing(),
    /* ac */ missing(),
    /* ad */ missing(),
    /* ae */ missing(),
    /* af */ missing(),

    /* b0 */ missing(),
    /* b1 */ missing(),
    /* b2 */ missing(),
    /* b3 */ missing(),
    /* b4 */ missing(),
    /* b5 */ missing(),
    /* b6 */ missing(),
    /* b7 */ missing(),
    /* b8 */ missing(),
    /* b9 */ missing(),
    /* ba */ missing(),
    /* bb */ missing(),
    /* bc */ missing(),
    /* bd */ missing(),
    /* be */ missing(),
    /* bf */ missing(),

    /* 00 */ missing(),
    /* 01 */ missing(),
    /* 02 */ missing(),
    /* 03 */ missing(),
    /* 04 */ missing(),
    /* 05 */ missing(),
    /* 06 */ missing(),
    /* 07 */ missing(),
    /* 08 */ missing(),
    /* 09 */ missing(),
    /* 0a */ missing(),
    /* 0b */ missing(),
    /* 0c */ missing(),
    /* 0d */ missing(),
    /* 0e */ missing(),
    /* 0f */ missing(),

    /* d0 */ missing(),
    /* d1 */ missing(),
    /* d2 */ missing(),
    /* d3 */ missing(),
    /* d4 */ missing(),
    /* d5 */ missing(),
    /* d6 */ missing(),
    /* d7 */ missing(),
    /* d8 */ missing(),
    /* d9 */ missing(),
    /* da */ missing(),
    /* db */ missing(),
    /* dc */ missing(),
    /* dd */ missing(),
    /* de */ missing(),
    /* df */ missing(),

    /* e0 */ missing(),
    /* e1 */ missing(),
    /* e2 */ missing(),
    /* e3 */ missing(),
    /* e4 */ missing(),
    /* e5 */ missing(),
    /* e6 */ missing(),
    /* e7 */ missing(),
    /* e8 */ missing(),
    /* e9 */ missing(),
    /* ea */ missing(),
    /* eb */ missing(),
    /* ec */ missing(),
    /* ed */ missing(),
    /* ee */ missing(),
    /* ef */ missing(),

    /* f0 */ missing(),
    /* f1 */ missing(),
    /* f2 */ missing(),
    /* f3 */ missing(),
    /* f4 */ missing(),
    /* f5 */ missing(),
    /* f6 */ missing(),
    /* f7 */ missing(),
    /* f8 */ missing(),
    /* f9 */ missing(),
    /* fa */ missing(),
    /* fb */ missing(),
    /* fc */ missing(),
    /* fd */ missing(),
    /* fe */ missing(),
    /* ff */ missing(),
];
