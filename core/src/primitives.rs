//! Types to represent Gameboy data.

use std::{
    ops::{Add, Sub, Index, IndexMut, AddAssign, SubAssign},
    fmt::{self, Debug, Display},
};

use derive_more::{BitXor, BitXorAssign, Display};


/// A single Gameboy byte.
///
/// This wrapper type is used to assert correct overflow behavior in arithmetic
/// operations.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, BitXor, BitXorAssign)]
pub struct Byte(u8);

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

impl AddAssign<u8> for Byte {
    fn add_assign(&mut self, rhs: u8) {
        *self = *self + rhs;
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

impl SubAssign<u8> for Byte {
    fn sub_assign(&mut self, rhs: u8) {
        *self = *self - rhs;
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


/// This represents a value consisting of two [`Byte`]s (e.g. an address).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Word(u16);


impl Word {
    pub fn new(val: u16) -> Self {
        Word(val)
    }

    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn get(&self) -> u16 {
        self.0
    }

    /// Creates a word from the two given bytes.
    pub fn from_bytes(lsb: Byte, msb: Byte) -> Self {
        let val = ((msb.get() as u16) << 8) | lsb.get() as u16;

        Self::new(val)
    }

    /// Destructs the word into two [`Byte`]s.
    ///
    /// The first [`Byte`] in the returned tuple is the lsb and the second
    /// [`Byte`] in the returned tuple is the msb (i.e. returned tuple: (lsb,
    /// msb)).
    pub fn into_bytes(self) -> (Byte, Byte) {
        let lsb = (self.0 & 0xff) as u8;
        let msb = ((self.0 >> 8) & 0xff) as u8;

        (Byte::new(lsb), Byte::new(msb))
    }
}

impl Add for Word {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Word(self.0.wrapping_add(rhs.0))
    }
}

impl Add<i8> for Word {
    type Output = Self;

    fn add(self, rhs: i8) -> Self {
        Word((self.0 as i16).wrapping_add(rhs as i16) as u16)
    }
}

impl Add<u16> for Word {
    type Output = Self;

    fn add(self, rhs: u16) -> Self {
        Word(self.0.wrapping_add(rhs as u16))
    }
}

impl AddAssign<i8> for Word {
    fn add_assign(&mut self, rhs: i8) {
        *self = *self + rhs;
    }
}

impl AddAssign<u16> for Word {
    fn add_assign(&mut self, rhs: u16) {
        *self = *self + rhs;
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

impl SubAssign<u16> for Word {
    fn sub_assign(&mut self, rhs: u16) {
        *self = *self - rhs;
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


/// A chunk of Gameboy memory. Can be indexed by `Word`.
pub struct Memory(Box<[Byte]>);

impl Memory {
    /// Returns a slice of memory with the specified length where all bytes are
    /// set to 0.
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
pub const CYCLES_PER_FRAME: u64 = 17556;

/// A simple integer to count how many cycles were already executed by the
/// emulator. This allows to check in what part of the frame we currently are.
#[derive(Debug, Display, Clone, Copy)]
pub struct CycleCounter(u64);

impl CycleCounter {
    pub fn zero() -> Self {
        CycleCounter(0)
    }

    /// Returns true, if the counter is exactly btweeen two frames, false otherwise.
    pub fn is_between_frames(&self) -> bool {
        self.0 % CYCLES_PER_FRAME == 0
    }
}

impl AddAssign<u8> for CycleCounter {
    fn add_assign(&mut self, rhs: u8) {
        self.0 += rhs as u64;
    }
}
