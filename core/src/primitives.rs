use std::{
    ops::{Add, Sub, Index, IndexMut},
    fmt::{self, Debug, Display},
};

/// This represents a byte
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Byte(u8);

impl Add for Byte {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Byte(self.0.wrapping_add(rhs.0))
    }
}

impl Add<u8> for Byte {
    type Output = Self;

    fn add(self, rhs: u8) -> Self {
        Byte(self.0.wrapping_add(rhs))
    }
}

impl Sub for Byte {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Byte(self.0.wrapping_sub(rhs.0))
    }
}

impl Sub<u8> for Byte {
    type Output = Self;

    fn sub(self, rhs: u8) -> Self {
        Byte(self.0.wrapping_sub(rhs))
    }
}

impl Debug for Byte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:02x}", self.0)
    }
}

impl Display for Byte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

impl Byte {
    pub fn new(val: u8) -> Self {
        Byte(val)
    }

    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn get(&self) -> u8 {
        self.0
    }
}

/// This represents a value consisting of two [`Byte`] (e.g. an address)
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Word(u16);

impl Add for Word {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Word(self.0.wrapping_add(rhs.0))
    }
}

impl Add<u16> for Word {
    type Output = Self;

    fn add(self, rhs: u16) -> Self {
        Word(self.0.wrapping_add(rhs as u16))
    }
}

impl Sub for Word {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Word(self.0.wrapping_sub(rhs.0))
    }
}

impl Sub<u16> for Word {
    type Output = Self;

    fn sub(self, rhs: u16) -> Self {
        Word(self.0.wrapping_sub(rhs as u16))
    }
}

impl Word {
    pub fn new(val: u16) -> Self {
        Word(val)
    }

    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn from_bytes(lsb: Byte, msb: Byte) -> Self {
        let val = ((msb.get() as u16) << 8) | lsb.get() as u16;

        Self::new(val)
    }

    pub fn get(&self) -> u16 {
        self.0
    }

    /// Destructs the word into two [`Byte`]s
    ///
    /// The first [`Byte`] in the returned tuple is the lsb and the second [`Byte`] in the
    /// returned tuple is the msb (e.g. returned tuple: (lsb, msb)).
    pub fn into_bytes(self) -> (Byte, Byte) {
        let lsb = (self.0 & 0xff) as u8;
        let msb = ((self.0 >> 8) & 0xff) as u8;

        (Byte::new(lsb), Byte::new(msb))
    }
}

impl Debug for Word {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:04x}", self.0)
    }
}

impl Display for Word {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// This represents memory
pub struct Memory(Box<[Byte]>);

impl Memory {
    pub fn zeroed(len: Word) -> Self {
        Memory(vec![Byte::zero(); len.get() as usize].into_boxed_slice())
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let copy: Vec<_> = bytes.iter().cloned().map(Byte::new).collect();
        Memory(copy.into_boxed_slice())
    }

    pub fn len(&self) -> Word {
        Word::new(self.0.len() as u16)
    }
}

impl Index<Word> for Memory {
    type Output = Byte;
    fn index(&self, index: Word) -> &Self::Output {
        &(*self.0)[index.0 as usize]
    }
}

impl IndexMut<Word> for Memory {
    fn index_mut(&mut self, index: Word) -> &mut Self::Output {
        &mut (*self.0)[index.0 as usize]
    }
}

// TODO cpu cycles or machine cycles???
/// Numbers of cycles per frame (including v-blank)
pub const CYCLES_PER_FRAME: u16 = 17556;

/// This represents the cycle counter which can have values between 0 and 17,556.
#[derive(Debug, Clone, Copy)]
pub struct CycleCounter(u16);

impl CycleCounter {
    pub fn zero() -> Self {
        CycleCounter(0)
    }

    /// Increases the counter by one. Automatically wraps around 17,556 to 1.
    pub fn inc(&mut self) {
        self.0 = if self.0 == CYCLES_PER_FRAME {
            1
        } else {
            self.0 + 1
        }
    }

    /// Returns true, if the cycle counter has reached the end of its range, false otherwise.
    pub fn at_end_of_frame(&self) -> bool {
        self.0 == CYCLES_PER_FRAME
    }
}
