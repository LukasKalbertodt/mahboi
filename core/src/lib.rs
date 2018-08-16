//! Mahboi!

#![feature(rust_2018_preview)]


use crate::primitives::{Byte, Addr, Memory};

mod primitives;


/// Width of the Game Boy screen in pixels.
pub const SCREEN_WIDTH: usize = 160;

/// Height of the Game Boy screen in pixels.
pub const SCREEN_HEIGHT: usize = 144;


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
    a: Byte, // accumulator
    f: Byte, // flags: 7 = zero, 6 = substract, 5 = half carry, 4 = carry
    b: Byte,
    c: Byte,
    d: Byte,
    e: Byte,
    h: Byte,
    l: Byte,

    // addressing registers
    sp: Addr,
    pc: Addr,
}

struct Cartridge {}

impl Machine {
    fn load_byte(&self, addr: Addr) -> Byte {
        // TODO :(
        match addr.get() {
            0x0000..=0x7FFF => unimplemented!(), // cartridge
            0x8000..=0x9FFF => unimplemented!(), // vram
            0xA000..=0xBFFF => unimplemented!(), // exram
            0xC000..=0xDFFF => self.wram[addr - Addr::new(0xC000)], // wram
            0xE000..=0xFDFF => self.wram[addr - Addr::new(0xC000 - 0x2000)], // wram echo
            0xFE00..=0xFE9F => unimplemented!(), // oam
            0xFEA0..=0xFEFF => unimplemented!(), // not usable (random ram, maybe use as rng???)
            0xFF00..=0xFF7F => unimplemented!(), // IO registers
            0xFF80..=0xFFFE => unimplemented!(), // hram
            0xFFFF => self.ie, // ie
            _ => unreachable!(),
        }
    }
}
