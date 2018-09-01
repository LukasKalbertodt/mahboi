//! Everything related to the cartridge and its header.

use std::{
    fmt,
    cmp::{PartialOrd, Ord, Ordering},
};

use crate::{
    log::*,
    mbc::{Mbc, NoMbc, Mbc1},
    primitives::{Byte, Word},
};


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
    Mbc1,
    Mbc1Ram,
    Mbc1RamBattery,
    Mbc2,
    Mbc2Battery,
    RomRam,
    RomRamBattery,
    Mmm01,
    Mmm01Ram,
    Mmm01RamBattery,
    Mbc3TimerBattery,
    Mbc3TimerRamBattery,
    Mbc3,
    Mbc3Ram,
    Mbc3RamBattery,
    Mbc5,
    Mbc5Ram,
    Mbc5RamBattery,
    Mbc5Rumble,
    Mbc5RumbleRam,
    Mbc5RumbleRamBattery,
    Mbc6,
    Mbc7SensorRumbleRamBattery,
    PocketCamera,
    BandaiTama5,
    HuC3,
    HuC1RamBattery,
}

impl CartridgeType {
    /// Parses the cartridge type from the given byte.
    pub fn from_byte(byte: u8) -> Self {
        use self::CartridgeType::*;

        match byte {
            0x00 => RomOnly,
            0x01 => Mbc1,
            0x02 => Mbc1Ram,
            0x03 => Mbc1RamBattery,
            0x05 => Mbc2,
            0x06 => Mbc2Battery,
            0x08 => RomRam,
            0x09 => RomRamBattery,
            0x0B => Mmm01,
            0x0C => Mmm01Ram,
            0x0D => Mmm01RamBattery,
            0x0F => Mbc3TimerBattery,
            0x10 => Mbc3TimerRamBattery,
            0x11 => Mbc3,
            0x12 => Mbc3Ram,
            0x13 => Mbc3RamBattery,
            0x19 => Mbc5,
            0x1A => Mbc5Ram,
            0x1B => Mbc5RamBattery,
            0x1C => Mbc5Rumble,
            0x1D => Mbc5RumbleRam,
            0x1E => Mbc5RumbleRamBattery,
            0x20 => Mbc6,
            0x22 => Mbc7SensorRumbleRamBattery,
            0xFC => PocketCamera,
            0xFD => BandaiTama5,
            0xFE => HuC3,
            0xFF => HuC1RamBattery,
            _ => panic!("Unsupported cartridge type {:02x}!", byte)
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
    Banks512,
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
            0x08 => RomSize::Banks512,
            0x52 => RomSize::Banks72,
            0x53 => RomSize::Banks80,
            0x54 => RomSize::Banks96,
            _ => panic!("Invalid ROM size in cartridge: {:02x}!", byte)
        }
    }

    /// Returns the number of bytes of the ROM.
    pub fn len(&self) -> usize {
        const BANK_SIZE: usize = 16 * 1024;

        match self {
            RomSize::NoBanking => 2 * BANK_SIZE,
            RomSize::Banks4 => 4 * BANK_SIZE,
            RomSize::Banks8 => 8 * BANK_SIZE,
            RomSize::Banks16 => 16 * BANK_SIZE,
            RomSize::Banks32 => 32 * BANK_SIZE,
            RomSize::Banks64 => 64 * BANK_SIZE,
            RomSize::Banks128 => 128 * BANK_SIZE,
            RomSize::Banks256 => 256 * BANK_SIZE,
            RomSize::Banks512 => 512 * BANK_SIZE,
            RomSize::Banks72 => 72 * BANK_SIZE,
            RomSize::Banks80 => 80 * BANK_SIZE,
            RomSize::Banks96 => 96 * BANK_SIZE,
        }
    }
}

impl Ord for RomSize {
    fn cmp(&self, other: &RomSize) -> Ordering {
        self.len().cmp(&other.len())
    }
}

impl PartialOrd for RomSize {
    fn partial_cmp(&self, other: &RomSize) -> Option<Ordering> {
        Some(self.cmp(other))
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

    /// Returns the number of bytes of the RAM.
    pub fn len(&self) -> usize {
        match self {
            RamSize::None => 0,
            RamSize::Kb2 => 2 * 1024,
            RamSize::Kb8 => 8 * 1024,
            RamSize::Kb32 => 32 * 1024,
        }
    }
}

/// A loaded cartridge.
///
/// This contains the full cartridge data and a number of fields for specific
/// header values.
pub struct Cartridge {
    title: String,
    cgb_mode: CgbMode,
    mbc: Box<dyn Mbc>,
    rom_size: RomSize,
    ram_size: RamSize,
    cartridge_type: CartridgeType,
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
        info!("{:?}, {:?}", cartridge_type, rom_size);

        // TODO checksum and nintendo logo check

        let mbc = Self::get_mbc_impl(cartridge_type)(bytes, rom_size, ram_size);

        Self {
            title: title.into_owned(),
            cgb_mode,
            mbc,
            rom_size,
            ram_size,
            cartridge_type,
        }
    }

    /// Load a [`Byte`] from the cartridge.
    ///
    /// The address has to be in the range `0x0000..0x8000` or
    /// `0xA000..0xC000`!
    pub fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0x0000..0x8000 => self.mbc.load_rom_byte(addr),
            0xA000..0xC000 => self.mbc.load_ram_byte(addr),
            _ => unreachable!(),
        }
    }

    /// Stores a [`Byte`] to the cartridge.
    ///
    /// The address has to be in the range `0x0000..0x8000` or
    /// `0xA000..0xC000`!
    pub fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0x0000..0x8000 => self.mbc.store_rom_byte(addr, byte),
            0xA000..0xC000 => self.mbc.store_ram_byte(addr, byte),
            _ => unreachable!(),
        }
    }

    /// Returns a function that creates the MBC implementation matching the
    /// given cartridge type.
    fn get_mbc_impl(ty: CartridgeType) -> impl FnOnce(&[u8], RomSize, RamSize) -> Box<dyn Mbc> {
        move |data, rom_size, ram_size| {
            match ty {
                CartridgeType::RomOnly => Box::new(NoMbc::new(data, rom_size, ram_size)),
                CartridgeType::Mbc1 => Box::new(Mbc1::new(data, rom_size, ram_size)),
                CartridgeType::Mbc1Ram => unimplemented!(),
                CartridgeType::Mbc1RamBattery => unimplemented!(),
                CartridgeType::Mbc2 => unimplemented!(),
                CartridgeType::Mbc2Battery => unimplemented!(),
                CartridgeType::RomRam => unimplemented!(),
                CartridgeType::RomRamBattery => unimplemented!(),
                CartridgeType::Mmm01 => unimplemented!(),
                CartridgeType::Mmm01Ram => unimplemented!(),
                CartridgeType::Mmm01RamBattery => unimplemented!(),
                CartridgeType::Mbc3TimerBattery => unimplemented!(),
                CartridgeType::Mbc3TimerRamBattery => unimplemented!(),
                CartridgeType::Mbc3 => unimplemented!(),
                CartridgeType::Mbc3Ram => unimplemented!(),
                CartridgeType::Mbc3RamBattery => unimplemented!(),
                CartridgeType::Mbc5 => unimplemented!(),
                CartridgeType::Mbc5Ram => unimplemented!(),
                CartridgeType::Mbc5RamBattery => unimplemented!(),
                CartridgeType::Mbc5Rumble => unimplemented!(),
                CartridgeType::Mbc5RumbleRam => unimplemented!(),
                CartridgeType::Mbc5RumbleRamBattery => unimplemented!(),
                CartridgeType::Mbc6 => unimplemented!(),
                CartridgeType::Mbc7SensorRumbleRamBattery => unimplemented!(),
                CartridgeType::PocketCamera => unimplemented!(),
                CartridgeType::BandaiTama5 => unimplemented!(),
                CartridgeType::HuC3 => unimplemented!(),
                CartridgeType::HuC1RamBattery => unimplemented!(),
            }
        }
    }
}

// Manual implementation to omit printing the full memory.
impl fmt::Debug for Cartridge {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Cartridge")
            .field("title", &self.title)
            .field("cgb_mode", &self.cgb_mode)
            .field("cartridge_type", &self.cartridge_type)
            .field("rom_size", &self.rom_size)
            .field("ram_size", &self.ram_size)
            .finish()
    }
}
