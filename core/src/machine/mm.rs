//! Everything related to memory mapping.

use super::Machine;
use crate::{
    primitives::{Word, Byte},
    log::*,
};


impl Machine {
    /// Loads a byte from the given address.
    pub fn load_byte(&self, addr: Word) -> Byte {
        // If DMA is ongoing, only HRAM can be accessed.
        if self.ppu.oam_dma_status.is_some() && !(0xFF80..0xFFFF).contains(&addr.get()) {
            Byte::new(0xFF) // TODO: is it really FF?
        } else {
            self.load_byte_bypass_dma(addr)
        }
    }

    /// Loads a byte from the given address, even if DMA is active (this is
    /// mainly used by the DMA precedure itself).
    pub fn load_byte_bypass_dma(&self, addr: Word) -> Byte {
        match addr.get() {
            // ROM mounted switch
            0x0000..0x0100 if self.bios_mounted() => self.bios[addr],

            0x0000..0x8000 => self.cartridge.mbc.load_rom_byte(addr), // Cartridge
            0x8000..0xA000 => self.ppu.load_vram_byte(addr),
            0xA000..0xC000 => self.cartridge.mbc.load_ram_byte(addr - 0xA000), // exram
            0xC000..0xE000 => self.wram[addr - 0xC000], // wram
            0xE000..0xFE00 => self.wram[addr - 0xE000], // wram echo
            0xFE00..0xFEA0 => self.ppu.load_oam_byte(addr), // oam
            0xFEA0..0xFF00 => {
                // On DMG this returns 0x00
                // TODO: Add correct CGB behavior
                Byte::zero()
            }

            // IF register
            0xFF00 => self.input_controller.load_register(),
            0xFF04..=0xFF07 => self.timer.load_byte(addr),
            0xFF0F => self.interrupt_controller.load_if(),
            0xFF40..=0xFF4B => self.ppu.load_io_byte(addr),
            0xFF00..0xFF80 => self.io[addr - 0xFF00], // IO registers
            0xFF80..0xFFFF => self.hram[addr - 0xFF80], // hram
            0xFFFF => self.interrupt_controller.interrupt_enable, // IE register
        }
    }

    /// Stores the given byte at the given address.
    pub(crate) fn store_byte(&mut self, addr: Word, byte: Byte) {
        // If DMA is ongoing, only HRAM can be accessed.
        if self.ppu.oam_dma_status.is_some() && !(0xFF80..0xFFFF).contains(&addr.get()) {
            return;
        }

        match addr.get() {
            // ROM mounted switch
            0x0000..0x0100 if self.bios_mounted() => warn!("Wrote to BIOS ROM!"),

            0x0000..0x8000 => self.cartridge.mbc.store_rom_byte(addr, byte), // Cartridge
            0x8000..0xA000 => self.ppu.store_vram_byte(addr, byte),
            0xA000..0xC000 => self.cartridge.mbc.store_ram_byte(addr - 0xA000, byte), // exram
            0xC000..0xE000 => self.wram[addr - 0xC000] = byte, // wram
            0xE000..0xFE00 => self.wram[addr - 0xE000] = byte, // wram echo
            0xFE00..0xFEA0 => self.ppu.store_oam_byte(addr, byte), // oam
            0xFEA0..0xFF00 => {
                // On DMG writes to this are ignored
                // TODO: Add correct CGB behavior
                trace!("Wrote to {} which is in not writable range: 0xFEA0..0xFF00!", addr);
            },

            // Register with flag for mounting/unmounting the BIOS (this is an IO register). To
            // this register may only be written, if the BIOS is mounted. When the BIOS is
            // unmounted, the write access is denied. We assume the Gameboy hardware does the same.
            0xFF50 if !self.bios_mounted() => warn!("Tried to re-mount BIOS!"),

            // IF register
            0xFF00 => self.input_controller.store_register(byte),
            0xFF04..=0xFF07 => self.timer.store_byte(addr, byte),
            0xFF0F => self.interrupt_controller.store_if(byte),
            0xFF40..=0xFF4B => self.ppu.store_io_byte(addr, byte),
            0xFF00..0xFF80 => self.io[addr - 0xFF00] = byte, // IO registers
            0xFF80..0xFFFF => self.hram[addr - 0xFF80] = byte, // hram
            0xFFFF => self.interrupt_controller.interrupt_enable = byte, // IE register
        }
    }
}
