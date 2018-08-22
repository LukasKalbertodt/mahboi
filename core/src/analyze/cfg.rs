use crate::{
    primitives::{Word},
};

use super::{
    instr::RawInstr,
    util::Span,
};


#[derive(Clone, Debug)]
pub struct Function {
    pub span: Span,
    pub blocks: Vec<Block>,
}

/// Consecutive instructions in the control flow graph which are always
/// executed from the beginning (i.e. the program never jumps somewhere in the
/// middle of this block). A block has single exit (the last instruction).
#[derive(Clone, Debug)]
pub struct Block {
    pub span: Span,
    pub raw_instrs: Vec<RawInstr>,
}

impl Block {
    pub fn new(start: Word) -> Self {
        Self {
            span: Span::empty_at(start),
            raw_instrs: vec![]
        }
    }

    pub(crate) fn add_instr(&mut self, instr: RawInstr) {
        self.span.hi += instr.len();
        self.raw_instrs.push(instr);
    }

    pub(crate) fn split_off(&mut self, at: Word) -> Block {
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
