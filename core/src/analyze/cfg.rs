use crate::{
    primitives::{Word},
};

use super::{
    instr::InstrWithArg,
    util::Span,
};


#[derive(Clone, Debug)]
pub struct Function {
    pub span: Span,
    pub blocks: Vec<Block>,
    pub foreign_calls: Vec<Word>,
}

/// A basic block in the CFG.
///
/// A basic block consists of consecutive instructions and has a single entry
/// (i.e. the program never jumps in the middle of a basic block).
#[derive(Clone, Debug)]
pub struct Block {
    pub span: Span,
    pub instrs: Vec<InstrWithArg>,
    // exits
}

impl Block {
    pub fn new(start: Word) -> Self {
        Self {
            span: Span::empty_at(start),
            instrs: vec![],
        }
    }

    pub(crate) fn add_instr(&mut self, instr: InstrWithArg) {
        self.span.hi += instr.kind().len;
        self.instrs.push(instr);
    }

    pub(crate) fn split_off(&mut self, at: Word) -> Block {
        assert!(self.span.contains(at));

        // Find the instruction index to split the vector
        let idx = self.instrs.iter()
            .scan(self.span.lo, |offset, instr| {
                let out = *offset;
                *offset += instr.kind().len;
                Some(out)
            })
            .position(|offset| offset == at)
            .unwrap_or(self.instrs.len());

        let second = self.instrs.split_off(idx);

        let end_second = self.span.hi;
        self.span.hi = at;

        Block {
            span: Span::new(at, end_second),
            instrs: second,
        }
    }
}

/// An address to some byte in a ROM region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RomAddr {
    /// Address to a byte in the BIOS.
    Bios(u8),

    /// Address to a byte in the cartridge ROM.
    Cartridge(u32),
}
