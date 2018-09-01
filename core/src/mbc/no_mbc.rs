use crate::{
    cartridge::{RamSize, RomSize},
    primitives::{Byte, Word},
};
use super::Mbc;

pub(crate) struct NoMbc {
    rom: Box<[Byte]>,
    ram: Box<[Byte]>,
}


impl NoMbc {
    pub(crate) fn new(data: &[u8], rom_size: RomSize, ram_size: RamSize) -> Self {
        assert!(ram_size != RamSize::Kb32, "More than 8KiB of RAM, but no MBC!");
        assert!(rom_size == RomSize::NoBanking, "ROM banking, but no MBC!");
        assert!(
            rom_size.len() == data.len(),
            "Length of cartridge doesn't match length specified in ROM size header",
        );

        let rom: Vec<_> = data.iter().cloned().map(Byte::new).collect();
        let ram = vec![Byte::zero(); ram_size.len()];

        Self {
            rom: rom.into_boxed_slice(),
            ram: ram.into_boxed_slice(),
        }
    }
}

impl Mbc for NoMbc {
    fn load_rom_byte(&self, addr: Word) -> Byte {
        self.rom[addr.get() as usize]
    }

    fn store_rom_byte(&mut self, _: Word, _: Byte) {
        // Nothing happens, writes are completely ignored
    }

    fn load_ram_byte(&self, addr: Word) -> Byte {
        let idx = addr.get() as usize;
        if idx < self.ram.len() {
            self.ram[idx]
        } else {
            Byte::zero()
        }
    }

    fn store_ram_byte(&mut self, addr: Word, byte: Byte) {
        // If some address outside of the RAM is referenced, nothing happens.
        let idx = addr.get() as usize;
        if idx < self.ram.len() {
            self.ram[idx] = byte;
        }
    }
}
