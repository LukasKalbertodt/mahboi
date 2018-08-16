//! Mahboi!

#![feature(rust_2018_preview)]


use crate::primitives::{Byte, Addr, Memory};
use crate::env::{Peripherals, Debugger};

mod primitives;
mod env;


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

impl Machine {
    fn new(cartridge: Cartridge) -> Self {
        Self {
            cpu: Cpu::new(),
            cartridge,
            vram: Memory::zeroed(Addr::new(0x2000)),
            wram: Memory::zeroed(Addr::new(0x1000)),
            oam: Memory::zeroed(Addr::new(0xA0)),
            hram: Memory::zeroed(Addr::new(0x7F)),
            ie: Byte::zero(),
        }
    }

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

impl Cpu {
    fn new() -> Self {
        Self {
            a: Byte::zero(),
            f: Byte::zero(),
            b: Byte::zero(),
            c: Byte::zero(),
            d: Byte::zero(),
            e: Byte::zero(),
            h: Byte::zero(),
            l: Byte::zero(),
            sp: Addr::zero(),
            pc: Addr::zero(),
        }
    }
}

pub struct Cartridge {}

pub struct Emulator<'a, P: 'a + Peripherals, D: 'a + Debugger> {
    machine: Machine,
    debug: &'a mut D,
    peripherals: &'a mut P,
}

impl<'a, P: 'a + Peripherals, D: 'a + Debugger> Emulator<'a, P, D> {
    pub fn new(cartridge: Cartridge, debug: &'a mut D, peripherals: &'a mut P) -> Self {
        Self {
            machine: Machine::new(cartridge),
            debug,
            peripherals,
        }
    }

    fn display(&mut self) -> &mut P::Display {
        self.peripherals.display()
    }

    fn sound(&mut self) -> &mut P::Sound {
        self.peripherals.sound()
    }

    fn input(&mut self) -> &mut P::Input {
        self.peripherals.input()
    }
}
