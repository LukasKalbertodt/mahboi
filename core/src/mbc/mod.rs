use crate::{
    primitives::{Byte, Word},
};
pub(crate) use self::{
    no_mbc::NoMbc,
    mbc1::Mbc1,
};

mod no_mbc;
mod mbc1;


/// A memory bank controller.
///
/// This part of the cartridge controls all writes and reads to and from ROM
/// and RAM. Usually, some kind of banking strategy is used to store more than
/// `0x8000` bytes on the cartridge.
pub(crate) trait Mbc {
    /// Loads one byte from the cartridge ROM. The `addr` has to be between `0`
    /// and `0x8000`.
    fn load_rom_byte(&self, addr: Word) -> Byte;

    /// Stores one byte to the cartridge ROM. The `addr` has to be between `0`
    /// and `0x8000`. This usually does nothing except potentially writing into
    /// MBC registers.
    fn store_rom_byte(&mut self, addr: Word, byte: Byte);

    /// Loads one byte from the external RAM. The `addr` is relative and has to
    /// be between `0` and `0x2000`.
    fn load_ram_byte(&self, addr: Word) -> Byte;

    /// Stores one byte to the external RAM. The `addr` is relative and has to
    /// be between `0` and `0x2000`.
    fn store_ram_byte(&mut self, addr: Word, byte: Byte);
}
