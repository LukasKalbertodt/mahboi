use crate::{
    primitives::{Byte, Word, Memory, CycleCounter},
    cartridge::{Cartridge},
};


#[macro_use]
mod macros;

pub mod instr;
mod mm;
mod step;


pub struct Machine {
    pub cpu: Cpu,

    pub cartridge: Cartridge,

    // TODO These should be arrays!
    pub bios: Memory,
    pub vram: Memory,
    pub wram: Memory,
    pub oam: Memory,

    // TODO IO register??? 0x80 bytes
    // Register with flag for mounting/unmounting the BIOS (this is an IO register).
    // Currently this is implemented as a single bool representing the flag.
    pub bios_mounted: bool,

    pub hram: Memory,
    pub ie: Byte,

    pub cycle_counter: CycleCounter,
}

impl Machine {
    pub(crate) fn new(cartridge: Cartridge) -> Self {
        Self {
            cpu: Cpu::new(),
            cartridge,
            bios: Memory::from_bytes(
                include_bytes!(
                    concat!(env!("CARGO_MANIFEST_DIR"), "/data/DMG_BIOS_ROM.bin")
                )
            ),
            vram: Memory::zeroed(Word::new(0x2000)),
            wram: Memory::zeroed(Word::new(0x1000)),
            oam: Memory::zeroed(Word::new(0xA0)),
            hram: Memory::zeroed(Word::new(0x7F)),
            ie: Byte::zero(),
            cycle_counter: CycleCounter::zero(),
            bios_mounted: true,
        }
    }

    pub fn load_word(&self, addr: Word) -> Word {
        // TODO: Check what happens on DMG hardware in this case
        if addr.get() == 0xffff {
            panic!("Index out of bounds!");
        }

        let lsb = self.load_byte(addr);
        let msb = self.load_byte(addr + 1u16);

        Word::from_bytes(lsb, msb)
    }

    pub fn store_word(&mut self, addr: Word, word: Word) {
        // TODO: Check what happens on DMG hardware in this case
        if addr.get() == 0xffff {
            panic!("Index out of bounds!");
        }

        let (lsb, msb) = word.into_bytes();
        self.store_byte(addr, lsb);
        self.store_byte(addr + 1u16, msb);
    }
}

pub struct Cpu {
    // general purpose registers
    pub a: Byte, // accumulator
    pub f: Byte, // flags: 7 = zero, 6 = substract, 5 = half carry, 4 = carry
    pub b: Byte,
    pub c: Byte,
    pub d: Byte,
    pub e: Byte,
    pub h: Byte,
    pub l: Byte,

    // addressing registers
    pub sp: Word,
    pub pc: Word,
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
            sp: Word::zero(),
            pc: Word::zero(),
        }
    }

    pub fn hl(&self) -> Word {
        Word::from_bytes(self.l, self.h)
    }

    pub fn de(&self) -> Word {
        Word::from_bytes(self.e, self.d)
    }

    pub fn bc(&self) -> Word {
        Word::from_bytes(self.c, self.b)
    }

    pub fn af(&self) -> Word {
        Word::from_bytes(self.f, self.a)
    }

    pub fn set_hl(&mut self, word: Word) {
        let (lsb, msb) = word.into_bytes();
        self.l = lsb;
        self.h = msb;
    }

    pub fn set_de(&mut self, word: Word) {
        let (lsb, msb) = word.into_bytes();
        self.e = lsb;
        self.d = msb;
    }

    pub fn set_bc(&mut self, word: Word) {
        let (lsb, msb) = word.into_bytes();
        self.c = lsb;
        self.b = msb;
    }

    pub fn set_af(&mut self, word: Word) {
        let (lsb, msb) = word.into_bytes();
        self.f = lsb;
        self.a = msb;
    }

    pub fn zero(&self) -> bool {
        (self.f.get() & 0b1000_0000) != 0
    }

    pub fn substract(&self) -> bool {
        (self.f.get() & 0b0100_0000) != 0
    }

    pub fn half_carry(&self) -> bool {
        (self.f.get() & 0b0010_0000) != 0
    }

    pub fn carry(&self) -> bool {
        (self.f.get() & 0b0001_0000) != 0
    }
}
