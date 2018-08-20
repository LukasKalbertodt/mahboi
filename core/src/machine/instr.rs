#[derive(Debug, Clone, Copy)]
pub struct Instr {

    /// Full mnemonic
    pub mnemonic: &'static str,

    /// Kind of instruction (e.g. ADD, LD, INC)
    pub kind: &'static str,

    /// List of params for instruction with placeholders for values
    pub params: &'static str,

    /// Length in bytes
    pub len: u8,

    /// CPU cylces
    pub cycles: u8,

    /// CPU cylces, if branch is taken
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

/// Placeholder for not implemented instructions.
const fn missing() -> Option<Instr> {
    None
}

/// Instruction sets
pub const INSTRUCTIONS: [Option<Instr>; 256] = [
    // 0x0_
    Instr::new("NOP",           "NOP",  "",         1,  4, None),
    Instr::new("LD BC, d16",    "LD",   "BC, {}",   1,  4, None),
    Instr::new("LD (BC), A",    "LD",   "",         1,  4, None),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    Instr::new("LD (a16), SP",  "LD",   "",         3,  20, None),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x1_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x2_
    Instr::new("JR NZ, r8",  "JR",   "NZ, {}",   2,  8, Some(12)),
    Instr::new("LD HL, d16",  "LD",   "HL, {}",   3,  12, None),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x3_
    missing(),
    Instr::new("LD SP, d16",  "LD",   "SP, {}",   3,  12, None),
    Instr::new("LD (HL-), A",  "LD",   "(HL-), A",   1,  8, None),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x4_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x5_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x6_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x7_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x8_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x9_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xA_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    Instr::new("XOR A",  "XOR",   "A",   1,  4, None),

    // 0xB_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xC_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // The opcode table is wrong! This instructions has a len and cycle number of 0
    Instr::new("PREFIX CB",  "PREFIX",   "CB",   0,  0, None),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xD_
    missing(),
    missing(),
    missing(),
    None,
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    None,
    missing(),
    None,
    missing(),
    missing(),

    // 0xE_
    missing(),
    missing(),
    missing(),
    None,
    None,
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    None,
    None,
    None,
    missing(),
    missing(),

    // 0xF_
    missing(),
    missing(),
    missing(),
    missing(),
    None,
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    None,
    None,
    missing(),
    missing(),
];

/// Instructions prefixed by CB opcode. (These opcodes are 2 bytes long.)
pub const PREFIXED_INSTRUCTIONS: [Option<Instr>; 256] = [
    // 0x0_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x1_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x2_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x3_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x4_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x5_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x6_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x7_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    Instr::new("BIT 7, H",  "BIT",   "7, H",   2,  8, None),
    missing(),
    missing(),
    missing(),

    // 0x8_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0x9_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xA_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xB_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xC_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xD_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xE_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),

    // 0xF_
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
    missing(),
];
