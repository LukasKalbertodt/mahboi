use std::{
    collections::BTreeMap,
    fmt,
};

// use slotmap::{Key, SlotMap};

use crate::{
    log::*,
    machine::{
        Machine,
        instr::{Instr, INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
    },
    primitives::{Byte, Memory, Word},
};


pub struct CodeMap {
    fns: BTreeMap<Word, Function>,

    /// For now, we only inspect the memory from 0 to 0x4000. This is read-only
    /// and basically guaranteed to not change. We capture this memory while
    /// the boot rom is still mounted. All of this will probably change later.
    mem: Memory,
}

impl CodeMap {
    pub fn new(machine: &Machine) -> Self {
        // Create the memory and fill it from the machine (only boot rom for now)
        let mut mem = Memory::zeroed(Word::new(0x100));
        for i in 0..0x100 {
            let addr = Word::new(i);
            mem[addr] = machine.load_byte(addr);
        }

        Self {
            fns: BTreeMap::new(),
            mem,
        }
    }

    pub fn add_entry_point(&mut self, entry_point: Word) {
        // We we already know about this entry point, do nothing
        if self.fns.contains_key(&entry_point) {
            return;
        }

        // Start analyzing the function.
        let mut blocks: Vec<Block> = vec![];
        let mut block_start_points = vec![entry_point];
        let mut counter = 3;

        while let Some(start) = block_start_points.pop() {
            trace!("Block start: {}", start);

            // Check if the start point is within an already existing block
            if let Some(idx) = blocks.iter_mut().position(|b| b.span.contains(start)) {
                let new_block = blocks[idx].split_off(start);
                blocks.push(new_block);
                continue;
            }

            // Start a new block
            let mut new_block = Block::new(start);

            let mut offset = start;
            loop {
                let instr = decode_instr([self.mem[offset], self.mem[offset + 1u8]])
                    .expect("tried to decode invalid opcode");

                let raw_instr = RawInstr::from_bytes(&self.mem[offset..offset + instr.len]);
                new_block.add_instr(raw_instr);


                if instr.jumps() {
                    // Add jump targets to the stack. If the jump is
                    // conditional, we add the the next instruction as start
                    // point.
                    if !instr.always_jumps() {
                        block_start_points.push(offset + instr.len);
                    }

                    // TODO: calculate jump destination
                    if let Some(target) = raw_instr.jump_target(offset) {
                        block_start_points.push(target);
                    }

                    break;
                }

                offset += instr.len;
            }

            blocks.push(new_block);

            counter -= 1;
            if counter == 0 {
                break;
            }
        }


        // print
        println!("{:#?}", blocks);
        println!("{:#?}", self.fns);
    }
}

#[derive(Clone, Debug)]
struct Function {
    span: Span,
    blocks: Vec<Block>,
}

/// Consecutive instructions in the control flow graph which are always
/// executed from the beginning (i.e. the program never jumps somewhere in the
/// middle of this block).
#[derive(Clone, Debug)]
struct Block {
    span: Span,
    raw_instrs: Vec<RawInstr>,
}

impl Block {
    fn new(start: Word) -> Self {
        Self {
            span: Span::empty_at(start),
            raw_instrs: vec![]
        }
    }

    fn add_instr(&mut self, instr: RawInstr) {
        self.span.hi += instr.len();
        self.raw_instrs.push(instr);
    }

    fn split_off(&mut self, at: Word) -> Block {
        assert!(self.span.contains(at));

        // Find the instruction index to split the vector
        let idx = self.raw_instrs.iter()
            .scan(self.span.lo, |offset, raw_instr| {
                let out = *offset;
                *offset += raw_instr.instr().len;
                Some(out)
            })
            .position(|offset| offset == at)
            .unwrap_or(self.raw_instrs.len());

        let second = self.raw_instrs.split_off(idx);

        let end_second = self.span.hi;
        self.span.hi = at;

        Block {
            span: Span::new(at, end_second),
            raw_instrs: second,
        }
    }
}


#[derive(Copy, Clone)]
struct Span {
    lo: Word,
    hi: Word,
}

impl Span {
    fn empty_at(addr: Word) -> Self {
        Self::new(addr, addr)
    }

    fn new(lo: Word, hi: Word) -> Self {
        assert!(hi >= lo);
        Self { lo, hi }
    }

    fn len(&self) -> Word {
        self.hi - self.lo
    }

    fn contains(&self, addr: Word) -> bool {
        self.lo <= addr && addr < self.hi
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}..{}", self.lo, self.hi)
    }
}

fn decode_instr(data: [Byte; 2]) -> Option<Instr> {
    if data[0] == 0xcb {
        PREFIXED_INSTRUCTIONS[data[1]]
    } else {
        INSTRUCTIONS[data[0]]
    }
}

#[derive(Copy, Clone)]
enum RawInstr {
    Short([Byte; 1]),
    Medium([Byte; 2]),
    Long([Byte; 3]),
}

impl RawInstr {
    fn from_bytes(data: &[Byte]) -> Self {
        match *data {
            [a] => RawInstr::Short([a]),
            [a, b] => RawInstr::Medium([a, b]),
            [a, b, c] => RawInstr::Long([a, b, c]),
            _ => panic!("oopsie: {:?}", data),
        }
    }

    fn instr(&self) -> Instr {
        // We can unwrap, because we checked we are a valid opcode when we were
        // created.
        match *self {
            RawInstr::Short([a]) => decode_instr([a, Byte::new(0)]),
            RawInstr::Medium([a, b]) | RawInstr::Long([a, b, _]) => decode_instr([a, b]),
        }.unwrap()
    }

    fn len(&self) -> u8 {
        self.as_slice().len() as u8
    }

    fn as_slice(&self) -> &[Byte] {
        match self {
            RawInstr::Short(s) => s,
            RawInstr::Medium(s) => s,
            RawInstr::Long(s) => s,
        }
    }

    /// Returns the jump target for JR, JP, CALL and RST instructions. Will
    /// return `None` for other instructions, notably `RET` and `RETI`.
    fn jump_target(&self, from: Word) -> Option<Word> {
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

trait InstrExt {
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
