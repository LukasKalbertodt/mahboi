use std::fmt;

use crate::{
    primitives::{Word},
};

#[derive(Copy, Clone)]
pub struct Span {
    pub lo: Word,
    pub hi: Word,
}

impl Span {
    pub fn empty_at(addr: Word) -> Self {
        Self::new(addr, addr)
    }

    pub fn new(lo: Word, hi: Word) -> Self {
        assert!(hi >= lo);
        Self { lo, hi }
    }

    pub fn len(&self) -> Word {
        self.hi - self.lo
    }

    pub fn contains(&self, addr: Word) -> bool {
        self.lo <= addr && addr < self.hi
    }
}

impl fmt::Debug for Span {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}..{}", self.lo, self.hi)
    }
}
