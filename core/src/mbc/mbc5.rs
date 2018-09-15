use crate::{
    log::*,
    cartridge::{RamSize, RomSize},
    primitives::{Byte, Word},
};
use super::Mbc;

/// MBC5.
///
/// With this controller, the cartridge can have up to 8MiB of ROM and up to
/// 128KiB of external RAM.
pub(crate) struct Mbc5 {
    rom: Box<[Byte]>,
    ram: Box<[Byte]>,

    /// A 9 bit number to select the bank mapped to 0x4000 -- 0x8000. Values 0
    /// to 0x1E0. In MBC5 you can actually select bank 0 here to map bank 0
    /// twice. Bits 9 to 15 are always 0.
    rom_bank: u16,

    /// A 4 bit number to select the RAM bank. Values 0 to 0xF.
    ram_bank: u8,

    /// Whether or not the RAM is enabled.
    ram_enabled: bool,
}


impl Mbc5 {
    pub(crate) fn new(data: &[u8], rom_size: RomSize, ram_size: RamSize) -> Self {
        assert!(
            rom_size.len() == data.len(),
            "Length of cartridge doesn't match length specified in ROM size header",
        );
        assert!(
            [RamSize::None, RamSize::Kb8, RamSize::Kb32, RamSize::Kb128].contains(&ram_size),
            "Illegal ram size {:?} for MBC5",
            ram_size,
        );

        let rom: Vec<_> = data.iter().cloned().map(Byte::new).collect();
        let ram = vec![Byte::zero(); ram_size.len()];

        Self {
            rom: rom.into_boxed_slice(),
            ram: ram.into_boxed_slice(),
            rom_bank: 0,
            ram_bank: 0,
            ram_enabled: false, // TODO: is that the correct initial value?
        }
    }
}

impl Mbc for Mbc5 {
    fn load_rom_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            // Always bank 0
            0x0000..0x4000 => self.rom[addr.get() as usize],

            // Bank 0 to N
            0x4000..0x8000 => {
                let bank_offset = self.rom_bank as usize * 0x4000;
                let relative_addr = addr.get() as usize - 0x4000;

                // We made sure that the actual cartridge data length matches
                // the number of banks specified in the header. However, the
                // game might enable a bank higher than specified in the
                // header. In that case we return FF.
                self.rom.get(bank_offset + relative_addr)
                    .cloned()
                    .unwrap_or(Byte::new(0xFF))
            }

            _ => unreachable!(),
        }
    }

    fn store_rom_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            // RAM enable
            0x0000..0x2000 => self.ram_enabled = byte.get() & 0x0F == 0x0A,

            // Lower 8 bits of ROM bank number
            0x2000..0x3000 => {
                self.rom_bank = (self.rom_bank & 0xFF00) | byte.get() as u16;
            }

            // Bit 9 of ROM bank number
            0x3000..0x4000 => {
                self.rom_bank = (self.rom_bank & 0xFF) | (byte.get() as u16 & 1);
            }

            // RAM bank number
            0x4000..0x6000 => {
                self.ram_bank = byte.get() & 0x0F;
            }

            // This is unused; the write is ignored.
            0x6000..0x8000 => {}

            _ => unreachable!(),
        }
    }

    fn load_ram_byte(&self, addr: Word) -> Byte {
        if !self.ram_enabled {
            return Byte::new(0xFF);
        }

        // If a value outside of the usable RAM is requested, we return FF.
        self.ram.get(self.ram_bank as usize * 0x2000 + addr.get() as usize)
            .cloned()
            .unwrap_or(Byte::new(0xFF))
    }

    fn store_ram_byte(&mut self, addr: Word, byte: Byte) {
        if !self.ram_enabled {
            return;
        }

        // Writes outside of the valid RAM are ignored.
        let idx = self.ram_bank as usize * 0x2000 + addr.get() as usize;
        if idx < self.ram.len() {
            self.ram[idx] = byte;
        } else {
            warn!(
                "[mbc5] write outside of valid RAM (bank {}, address {})",
                self.ram_bank,
                addr,
            );
        }
    }
}
