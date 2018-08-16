use std::{
    ops::{Add, Sub, Index, IndexMut},
    fmt::{self, Debug, Display},
};

/// This represents a byte
#[derive(Clone, Copy)]
pub(crate) struct Byte(u8);

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
        write!(f, "{:02x}", self.0)
    }
}

impl Display for Byte {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// This represents an adress
#[derive(Clone, Copy)]
pub(crate) struct Addr(u16);

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
    pub(crate) fn get(&self) -> u16 {
        self.0
    }

    pub(crate) fn new(val: u16) -> Self {
        Addr(val)
    }
}

impl Debug for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:04x}", self.0)
    }
}

impl Display for Addr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self, f)
    }
}

/// This represents memory
pub(crate) struct Memory(Box<[Byte]>);

impl Index<Addr> for Memory {
    type Output = Byte;
    fn index(&self, index: Addr) -> &Self::Output {
        &(*self.0)[index.0 as usize]
    }
}
