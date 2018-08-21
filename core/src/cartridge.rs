//! Everything related to the cartridge and its header.

use std::fmt;

use crate::primitives::{Byte, Word};


/// Specifies how this ROM works with the CGB. Stored at `0x0143`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CgbMode {
    /// Only CGB is supported. Value `0xC0`.
    CgbOnly,

    /// DMG and CGB are supported. Value `0x80`.
    BothSupported,

    /// CGB features are not supported, but something special happens. We
    /// think. More investigation needed. Value: bit 7 and at least one of bit
    /// 2 or bit 3 is set.
    NonCgbSpecial,

    /// CGB features are not supported. Value: bit 7 is not set.
    NonCgb,
}

impl CgbMode {
    /// Parses the CGB mode from the given byte.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            // Bit 7 not set
            0x00..=0x7F => CgbMode::NonCgb,
            0xC0 => CgbMode::CgbOnly,
            0x80 => CgbMode::BothSupported,

            // Bit 7 and bit 2 or 3 set
            b if (b & 0b0000_0110) != 0 => CgbMode::NonCgbSpecial,
            _ => panic!("Unsupported cartridge CGB mode!"),
        }
    }
}

/// The type of a cartridge. This defines whether a cartridge has a memory bank
/// controller, a battery, external ram or anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CartridgeType {
    RomOnly,
    Mbc5RamBattery,
}

impl CartridgeType {
    /// Parses the cartridge type from the given byte.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => CartridgeType::RomOnly,
            0x1B => CartridgeType::Mbc5RamBattery,
            _ => panic!("The given cartridge type {:02x} is unimplemented!", byte)
        }
    }
}

/// Size of cartridge's ROM. Defined by the number of banks (each 16 KiB).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RomSize {
    NoBanking,
    Banks4,
    Banks8,
    Banks16,
    Banks32,
    Banks64,
    Banks128,
    Banks256,
    Banks72,
    Banks80,
    Banks96,
}

impl RomSize {
    /// Parses the ROM size from the given byte.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => RomSize::NoBanking,
            0x01 => RomSize::Banks4,
            0x02 => RomSize::Banks8,
            0x03 => RomSize::Banks16,
            0x04 => RomSize::Banks32,
            0x05 => RomSize::Banks64,
            0x06 => RomSize::Banks128,
            0x07 => RomSize::Banks256,
            0x52 => RomSize::Banks72,
            0x53 => RomSize::Banks80,
            0x54 => RomSize::Banks96,
            _ => panic!("Invalid ROM size in cartridge: {:02x}!", byte)
        }
    }
}

/// Size of a cartridge's RAM. Specified in KiB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RamSize {
    None,
    Kb2,
    Kb8,
    Kb32, // 4 banks of 8KBytes each
}

impl RamSize {
    /// Parses the RAM size from the given byte.
    pub fn from_byte(byte: u8) -> Self {
        match byte {
            0x00 => RamSize::None,
            0x01 => RamSize::Kb2,
            0x02 => RamSize::Kb8,
            0x03 => RamSize::Kb32,
            _ => panic!("Invalid RAM size in cartridge: {:02x}!", byte)
        }
    }
}

/// A loaded cartridge.
///
/// This contains the full cartridge data and a number of fields for specific
/// header values.
pub struct Cartridge {
    rom: Box<[Byte]>,
    title: String,
    cgb_mode: CgbMode,
    cartridge_type: CartridgeType,
    rom_size: RomSize,
    ram_size: RamSize,
}

impl Cartridge {
    pub fn from_bytes(bytes: &[u8]) -> Self {
        // Parse header fields

        // Detect the name length by testing if the last 4 bytes contain a 0
        let man_code = &bytes[0x013F..=0x0142];
        let max_title_len = if man_code.iter().any(|b| *b == 0x00) {
            15
        } else {
            11
        };

        // Get title
        let title_len = bytes[0x0134..0x0134 + max_title_len]
            .iter()
            .position(|b| *b == 0x00)
            .unwrap_or(max_title_len);
        let title = String::from_utf8_lossy(&bytes[0x0134..0x0134 + title_len]);

        // Read a couple of one byte values
        let cgb_mode = CgbMode::from_byte(bytes[0x0143]);
        let cartridge_type = CartridgeType::from_byte(bytes[0x0147]);
        let rom_size = RomSize::from_byte(bytes[0x0148]);
        let ram_size = RamSize::from_byte(bytes[0x0149]);

        // TODO checksum and nintendo logo check

        // Copy ROM data
        let copy: Vec<_> = bytes.iter().cloned().map(Byte::new).collect();
        Self {
            rom: copy.into_boxed_slice(),
            title: title.into_owned(),
            cgb_mode,
            cartridge_type,
            rom_size,
            ram_size,
        }
    }

    /// Load a [`Byte`] from the cartridge.
    pub fn load_byte(&self, addr: Word) -> Byte {
        match self.cartridge_type {
            CartridgeType::RomOnly => self.rom[addr.get() as usize],
            _ => unimplemented!(),
        }
    }
}

// Manual implementation to omit printing the full memory.
impl fmt::Debug for Cartridge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Cartridge")
            .field("length", &self.rom.len())
            .field("title", &self.title)
            .field("cgb_mode", &self.cgb_mode)
            .field("cartridge_type", &self.cartridge_type)
            .field("rom_size", &self.rom_size)
            .field("ram_size", &self.ram_size)
            .finish()
    }
}
