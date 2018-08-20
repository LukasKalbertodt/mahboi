use super::Machine;
use crate::{
    primitives::{Word, Byte},
    log::*,
};


impl Machine {
    pub fn load_byte(&self, addr: Word) -> Byte {
        // TODO :(
        match addr.get() {
            // ROM mounted switch
            0x0000..0x0100 if self.bios_mounted => self.bios[addr],

            0x0000..0x8000 => unimplemented!(), // Cartridge
            0x8000..0xA000 => unimplemented!(), // vram
            0xA000..0xC000 => unimplemented!(), // exram
            0xC000..0xE000 => self.wram[addr - 0xC000], // wram
            0xE000..0xFE00 => self.wram[addr - 0xC000 - 0x2000], // wram echo
            0xFE00..0xFEA0 => unimplemented!(), // oam
            0xFEA0..0xFF00 => unimplemented!(), // not usable (random ram, maybe use as rng???)
            0xFF00..0xFF80 => unimplemented!(), // IO registers
            0xFF80..0xFFFF => unimplemented!(), // hram
            0xFFFF => self.ie, // ie
            _ => unreachable!(),
        }
    }

    pub(crate) fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            // ROM mounted switch
            0x0000..0x0100 if self.bios_mounted => warn!("Wrote to BIOS ROM!"),

            0x0000..0x8000 => unimplemented!(), // Cartridge
            0x8000..0xA000 => self.vram[addr - 0x8000] = byte, // vram
            0xA000..0xC000 => unimplemented!(), // exram
            0xC000..0xE000 => self.wram[addr - 0xC000] = byte, // wram
            0xE000..0xFE00 => self.wram[addr - 0xE000] = byte, // wram echo
            0xFE00..0xFEA0 => unimplemented!(), // oam
            0xFEA0..0xFF00 => unimplemented!(), // not usable (random ram, maybe use as rng???)
            0xFF00..0xFF80 => unimplemented!(), // IO registers
            0xFF80..0xFFFF => unimplemented!(), // hram
            0xFFFF => unimplemented!(), // ie
            _ => unreachable!(),
        }
    }
}
