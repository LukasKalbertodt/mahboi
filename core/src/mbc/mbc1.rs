use std::cmp::max;

use crate::{
    cartridge::{RamSize, RomSize},
    primitives::{Byte, Word},
};
use super::Mbc;

/// The first version of a memory bank controller used by many games such as
/// "Super Mario World".
///
/// With this controller, the cartridge can have up to 2MiB of ROM and up to
/// 32KiB of external RAM.
pub(crate) struct Mbc1 {
    rom: Box<[Byte]>,
    ram: Box<[Byte]>,

    /// This register is used both for ROM and RAM banking. Bits 0--4 are
    /// always used for the ROM bank. Bits 5 and 6 are either used to select
    /// the RAM bank or the ROM bank, depending on `mode`. Bit 7 is always 0.
    ///
    /// Bits 0--4 cannot be 0. They are always in the range 1--31.
    current_bank: u8,

    /// RAM/ROM mode. `false` is ROM mode (bits 5 and 6 in `current_bank` are
    /// used to select the ROM bank) and `true` is RAM mode (bits 5 and 6 in
    /// `current_bank` are used to select the RAM bank (0 to 3)).
    ram_mode: bool,

    /// Whether or not the RAM is enabled.
    ram_enabled: bool,
}


impl Mbc1 {
    pub(crate) fn new(data: &[u8], rom_size: RomSize, ram_size: RamSize) -> Self {
        assert!(rom_size <= RomSize::Banks128, "More than 128 banks, but only MBC1!");
        assert!(
            rom_size.len() == data.len(),
            "Length of cartridge doesn't match length specified in ROM size header",
        );

        let rom: Vec<_> = data.iter().cloned().map(Byte::new).collect();
        let ram = vec![Byte::zero(); ram_size.len()];

        Self {
            rom: rom.into_boxed_slice(),
            ram: ram.into_boxed_slice(),
            current_bank: 1,
            ram_mode: false,
            ram_enabled: false, // TODO: is that the correct initial value?
        }
    }

    /// Returns the real ROM bank number (with respect to `ram_mode`)
    fn rom_bank(&self) -> usize {
        if self.ram_mode {
            (self.current_bank & 0b0001_1111) as usize
        } else {
            (self.current_bank & 0b0111_1111) as usize
        }
    }

    /// Returns the real RAM bank number (with respect to `ram_mode`)
    fn ram_bank(&self) -> usize {
        if self.ram_mode {
            ((self.current_bank & 0b0110_0000) >> 5) as usize
        } else {
            0
        }
    }
}

impl Mbc for Mbc1 {
    fn load_rom_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            // Always bank 0
            0x0000..0x4000 => self.rom[addr.get() as usize],

            // Bank 1 to N
            0x4000..0x8000 => {
                let bank_offset = self.rom_bank() * 0x4000;
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
            0x0000..0x2000 => self.ram_enabled = byte.get() & 0x0F == 0x0F,

            // Lower 5 bits of ROM bank number
            0x2000..0x4000 => {
                // Again, we can never write 0 to those bits.
                let new = max(byte.get() & 0b0001_1111, 1);
                self.current_bank = (self.current_bank & 0b1110_0000) | new;
            }

            // 2 Bits of ROM or RAM bank
            0x4000..0x6000 => {
                let new = byte.get() & 0b0110_0000;
                self.current_bank = (self.current_bank & 0b1001_1111) | new;
            }

            // Mode select
            0x6000..0x8000 => self.ram_mode = byte.get() != 0,

            _ => unreachable!(),
        }
    }

    fn load_ram_byte(&self, addr: Word) -> Byte {
        // If a value outside of the usable RAM is requested, we return FF.
        self.ram.get(self.ram_bank() * 0x2000 + addr.get() as usize)
            .cloned()
            .unwrap_or(Byte::new(0xFF))
    }

    fn store_ram_byte(&mut self, addr: Word, byte: Byte) {
        // Writes outside of the valid RAM are ignored.
        let idx = self.ram_bank() * 0x2000 + addr.get() as usize;
        if idx < self.ram.len() {
            self.ram[idx] = byte;
        }
    }
}
