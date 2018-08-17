use std::{
    ops::{Add, Sub, Index, IndexMut},
    fmt::{self, Debug, Display},
};

/// This represents a byte
#[derive(Clone, Copy)]
pub struct Byte(u8);

impl Add for Byte {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Byte(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for Byte {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Byte(self.0.wrapping_sub(rhs.0))
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
}

/// This represents an adress
#[derive(Clone, Copy)]
pub struct Addr(u16);

impl Add for Addr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Addr(self.0.wrapping_add(rhs.0))
    }
}

impl Sub for Addr {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Addr(self.0.wrapping_sub(rhs.0))
    }
}

impl Addr {
    pub fn new(val: u16) -> Self {
        Addr(val)
    }

    pub fn zero() -> Self {
        Self::new(0)
    }

    pub fn get(&self) -> u16 {
        self.0
    }
}

impl Debug for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "0x{:04x}", self.0)
    }
}

impl Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// This represents memory
pub struct Memory(Box<[Byte]>);

impl Memory {
    pub fn zeroed(len: Addr) -> Self {
        Memory(vec![Byte::zero(); len.get() as usize].into_boxed_slice())
    }

    pub fn from_bytes(bytes: &[u8]) -> Self {
        let copy: Vec<_> = bytes.iter().cloned().map(Byte::new).collect();
        Memory(copy.into_boxed_slice())
    }

    pub fn len(&self) -> Addr {
        Addr::new(self.0.len() as u16)
    }
}

impl Index<Addr> for Memory {
    type Output = Byte;
    fn index(&self, index: Addr) -> &Self::Output {
        &(*self.0)[index.0 as usize]
    }
}

impl IndexMut<Addr> for Memory {
    fn index_mut(&mut self, index: Addr) -> &mut Self::Output {
        &mut (*self.0)[index.0 as usize]
    }
}

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
