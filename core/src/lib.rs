//! Mahboi!

#![feature(rust_2018_preview)]


use std::{
    ops::{Add, Sub, Index, IndexMut},
};


/// Width of the Game Boy screen in pixels.
pub const SCREEN_WIDTH: usize = 160;

/// Height of the Game Boy screen in pixels.
pub const SCREEN_HEIGHT: usize = 144;


#[derive(Clone, Copy)]
struct Byte(u8);
#[derive(Clone, Copy)]
struct Addr(u16);

// TODO overflow semantics?
impl Add for Addr {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        Addr(self.0 + rhs.0)
    }
}

impl Sub for Addr {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self {
        Addr(self.0 - rhs.0)
    }
}

struct Memory(Box<[Byte]>);

impl Index<Addr> for Memory {
    type Output = Byte;
    fn index(&self, index: Addr) -> &Self::Output {
        &(*self.0)[index.0 as usize]
    }
}

struct Machine {
    cpu: Cpu,

    cartridge: Cartridge,

    // TODO These should be arrays!
    vram: Memory,
    wram: Memory,
    oam: Memory,
    // TODO IO register??? 0x80 bytes
    hram: Memory,
    ie: Byte,
}

struct Cpu {
    // general purpose registers
    a: Byte,
    f: Byte,
    b: Byte,
    c: Byte,
    d: Byte,
    e: Byte,
    h: Byte,
    l: Byte,

    // addressing registers
    sp: Addr,
    pc: Addr,

    // flags
    zero: bool,
    subtract: bool,
    half_carry: bool,
    carry: bool,
}

struct Cartridge {}

impl Machine {
    fn load_byte(&self, addr: Addr) -> Byte {
        // TODO :(
        match addr.0 {
            0x0000..=0x7FFF => unimplemented!(), // cartridge
            0x8000..=0x9FFF => unimplemented!(), // vram
            0xA000..=0xBFFF => unimplemented!(), // exram
            0xC000..=0xDFFF => self.wram[addr - Addr(0xC000)], // wram
            0xE000..=0xFDFF => self.wram[addr - Addr(0xC000 - 0x2000)], // wram echo
            0xFE00..=0xFE9F => unimplemented!(), // oam
            0xFEA0..=0xFEFF => unimplemented!(), // not usable (random ram, maybe use as rng???)
            0xFF00..=0xFF7F => unimplemented!(), // IO registers
            0xFF80..=0xFFFE => unimplemented!(), // hram
            0xFFFF => self.ie, // ie
            _ => unreachable!(),
        }
    }
}
