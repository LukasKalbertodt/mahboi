use std::cmp::max;

use crate::{
    log::*,
    cartridge::{RamSize, RomSize},
    primitives::{Byte, Word},
};
use super::Mbc;

/// Third version of the memory bank controller. In contrast to all other MBCs,
/// this one can have a real time clock (RTC).
pub(crate) struct Mbc3 {
    rom: Box<[Byte]>,
    ram: Box<[Byte]>,

    /// Stores the current ROM bank. 7 bits are usable, the MSB is always 0.
    /// While all bits are writeable, bank 0 still cannot be selected (this is
    /// checked when accessing the ROM).
    rom_bank: u8,

    /// This stores the current RAM bank OR the current RTC register. A value
    /// of 0--3 enables a specific RAM bank; a value from 8--C enables a RTC
    /// register.
    ram_bank: u8,

    /// Whether or not the RAM and RTC registers are enabled.
    ram_enabled: bool,

    /// This serves a dual role.
    ///
    /// For reading, the user has to latch the registers. That means that the
    /// actual values from the clock are written into this value (as some kind
    /// of cache). The clock continues to run in the background.
    ///
    /// For writing, the user has to set the HALT flag to `true`. As soon as
    /// that flag is set to `false` again, the values are "committed" to the
    /// actual clock.
    ///
    /// TODO: currently, the actual clock is not implemented. The registers
    /// always return 0.
    rtc_regs: RtcRegisters,

    /// When the user writes a 0 and then a 1 into this register, the clock's
    /// values are latched into the RTC registers.
    latch_rtc: Byte,
}


impl Mbc3 {
    pub(crate) fn new(data: &[u8], rom_size: RomSize, ram_size: RamSize) -> Self {
        assert!(rom_size <= RomSize::Banks128, "More than 128 banks, but only MBC3!");
        assert!(
            rom_size.len() == data.len(),
            "Length of cartridge doesn't match length specified in ROM size header",
        );

        let rom: Vec<_> = data.iter().cloned().map(Byte::new).collect();
        let ram = vec![Byte::zero(); ram_size.len()];

        // TODO: are these all the correct initial values?
        Self {
            rom: rom.into_boxed_slice(),
            ram: ram.into_boxed_slice(),
            rom_bank: 0,
            ram_bank: 0,
            ram_enabled: false,
            rtc_regs: RtcRegisters::new(),
            latch_rtc: Byte::zero(),
        }
    }
}

impl Mbc for Mbc3 {
    fn load_rom_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            // Always bank 0
            0x0000..0x4000 => self.rom[addr.get() as usize],

            // Bank 1 to N
            0x4000..0x8000 => {
                // Bank 0 cannot be mapped in this memory.
                let bank = max(self.rom_bank, 1);
                let bank_offset = bank as usize * 0x4000;
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

            // The ROM bank number
            0x2000..0x4000 => {
                // In contrast to MBC1, all seven bits are written (including
                // bit 0).
                self.rom_bank = byte.get() & 0b0111_1111;
            }

            // RAM bank or RTC register
            0x4000..0x6000 => {
                // Make sure a valid value is written.
                let b = byte.get();
                if (b <= 3) || (b >= 0x8 && b <= 0xC) {
                    self.ram_bank = b;
                } else {
                    // TODO: what happens here?
                    warn!("[mbc3] RAM bank/RTC register {} was selected. What now?!", byte);
                }
            }

            // RTC latch registers
            0x6000..0x8000 => {
                if self.latch_rtc == Byte::zero() && byte == Byte::new(1) {
                    self.rtc_regs.latch();
                }
                self.latch_rtc = byte;
            }

            _ => unreachable!(),
        }
    }

    fn load_ram_byte(&self, addr: Word) -> Byte {
        if !self.ram_enabled {
            return Byte::new(0xFF);
        }

        match self.ram_bank {
            // RAM
            0..=3 => {
                // If a value outside of the usable RAM is requested, we return FF.
                self.ram.get(self.ram_bank as usize * 0x2000 + addr.get() as usize)
                    .cloned()
                    .unwrap_or(Byte::new(0xFF))
            }

            // RTC registers
            0x8 => self.rtc_regs.secs,
            0x9 => self.rtc_regs.mins,
            0xA => self.rtc_regs.hours,
            0xB => self.rtc_regs.days_low,
            0xC => self.rtc_regs.extra,

            _ => unreachable!(),
        }
    }

    fn store_ram_byte(&mut self, addr: Word, byte: Byte) {
        if !self.ram_enabled {
            return;
        }

        match self.ram_bank {
            // RAM
            0..=3 => {
                // Writes outside of the valid RAM are ignored.
                let idx = self.ram_bank as usize * 0x2000 + addr.get() as usize;
                if idx < self.ram.len() {
                    self.ram[idx] = byte;
                }
            }

            // RTC registers
            0x8 if self.rtc_regs.is_halted() => self.rtc_regs.secs = byte,
            0x9 if self.rtc_regs.is_halted() => self.rtc_regs.mins = byte,
            0xA if self.rtc_regs.is_halted() => self.rtc_regs.hours = byte,
            0xB if self.rtc_regs.is_halted() => self.rtc_regs.days_low = byte,
            0x8..=0xB if !self.rtc_regs.is_halted() => {
                warn!("[mbc3] Write to RTC register 0x{:x} while RTC is running!", self.ram_bank);
            }
            0xC => {
                // Check what this write does with the HALT flag.
                if byte.get() & 0b0100_0000 != 0 {
                    // If the write halts the clock, we allow the write
                    self.rtc_regs.extra = byte;
                    self.rtc_regs.pause();
                } else if byte.get() & 0b0100_0000 == 0 {
                    // If the clock is resumed, we have to transfer all written
                    // registers to the actual clock.
                    self.rtc_regs.resume();
                } else {
                    warn!("[mbc3] Write to RTC register 0xC while RTC is running!");
                }
            },

            _ => unreachable!(),
        }
    }
}


/// Everything related to the real time clock (RTC).
struct RtcRegisters {
    /// Range 0 -- 59
    secs: Byte,

    /// Range 0 -- 59
    mins: Byte,

    /// Range 0 -- 23
    hours: Byte,

    /// Lower 8 bits of the day value. The day value consists of 9 bits and can
    /// thus hold values from 0 to 511.
    days_low: Byte,

    /// Holds three useful bits (all other bits are 0; TODO: really?):
    /// - Bit 0: bit 9 of the day value
    /// - Bit 6: HALT flag
    /// - Bit 7: day carry flag
    extra: Byte,
}

impl RtcRegisters {
    fn new() -> Self {
        Self {
            secs: Byte::zero(),
            mins: Byte::zero(),
            hours: Byte::zero(),
            days_low: Byte::zero(),
            extra: Byte::zero(),
        }
    }

    /// Checks if the RTC is halted right now (as determined by bit 6 of the
    /// extra register).
    fn is_halted(&self) -> bool {
        self.extra.get() & 0b0100_0000 != 0
    }

    /// Take the values from the real clock and write them into the user
    /// accessible registers. This has to be used before reading any registers.
    fn latch(&mut self) {
        // TODO: read actual value from system clock
        self.secs = Byte::zero();
        self.mins = Byte::zero();
        self.hours = Byte::zero();
        self.days_low = Byte::zero();
        self.extra = Byte::zero();
    }

    /// Pause the RTC. Done by writing 1 to the HALT flag.
    fn pause(&mut self) {
        // TODO
    }

    /// Continue the RTC. Done by writing 0 to the HALT flag.
    fn resume(&mut self) {
        // TODO
    }
}
