use crate::{
    primitives::{Byte, Memory, Word},
};


// TODO: Because of the lack of information some assumptions has been made which need proove:
// - Not readable registers/bits return 0xFF or 1 when read.

/// Manages the sound. This is mapped to `0xFF10..0xFF40` in the Memory.
pub(crate) struct SoundController {
    channel1_sweep: Byte,
    channel1_length: Byte,
    channel1_volume: Byte,
    channel1_frequency_lo: Byte,
    channel1_frequency_hi: Byte,

    channel2_length: Byte,
    channel2_volume: Byte,
    channel2_frequency_lo: Byte,
    channel2_frequency_hi: Byte,

    channel3_on_off: Byte,
    channel3_length: Byte,
    channel3_output_level: Byte,
    channel3_frequency_lo: Byte,
    channel3_frequency_hi: Byte,

    channel4_length: Byte,
    channel4_volume: Byte,
    channel4_polynomial_counter: Byte,
    channel4_counter: Byte,

    channel_control: Byte,
    selection_output: Byte,
    sound_on_off: Byte,

    wave: Memory,
}

impl SoundController {
    pub(crate) fn new() -> Self {
        Self {
            channel1_sweep: Byte::zero(),
            channel1_length: Byte::zero(),
            channel1_volume: Byte::zero(),
            channel1_frequency_lo: Byte::zero(),
            channel1_frequency_hi: Byte::zero(),
            channel2_length: Byte::zero(),
            channel2_volume: Byte::zero(),
            channel2_frequency_lo: Byte::zero(),
            channel2_frequency_hi: Byte::zero(),
            channel3_on_off: Byte::zero(),
            channel3_length: Byte::zero(),
            channel3_output_level: Byte::zero(),
            channel3_frequency_lo: Byte::zero(),
            channel3_frequency_hi: Byte::zero(),
            channel4_length: Byte::zero(),
            channel4_volume: Byte::zero(),
            channel4_polynomial_counter: Byte::zero(),
            channel4_counter: Byte::zero(),
            channel_control: Byte::zero(),
            selection_output: Byte::zero(),
            sound_on_off: Byte::zero(),
            wave: Memory::zeroed(Word::new(0x10)),
        }
    }

    /// Loads one byte from the sound registers. The `addr` has to be between `0`
    /// and `0x30` (excluding).
    pub(crate) fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            // TODO: This is only a placeholder implementation
            0x00 => self.channel1_sweep,
            0x01 => self.channel1_length,
            0x02 => self.channel1_volume,
            0x03 => self.channel1_frequency_lo,
            0x04 => self.channel1_frequency_hi,

            // TODO: This is only a placeholder implementation
            0x06 => self.channel2_length,
            0x07 => self.channel2_volume,
            0x08 => self.channel2_frequency_lo,
            0x09 => self.channel2_frequency_hi,

            0x0A => self.channel3_on_off.mask_or(0b1000_0000),
            0x0B => Byte::new(0xFF),
            0x0C => self.channel3_output_level.mask_or(0b0110_0000),
            0x0D => Byte::new(0xFF),
            0x0E => self.channel3_frequency_hi.mask_or(0b1100_0111),

            // TODO: This is only a placeholder implementation
            0x10 => self.channel4_length,
            0x11 => self.channel4_volume,
            0x12 => self.channel4_polynomial_counter,
            0x13 => self.channel4_counter,

            // TODO: This is only a placeholder implementation
            0x14 => self.channel_control,
            0x15 => self.selection_output,
            0x16 => self.sound_on_off,

            0x20..=0x2F => self.wave[addr],

            _ => unreachable!(),
        }
    }

    /// Stores one byte to the sound registers. The `addr` has to be between `0`
    /// and `0x30` (excluding).
    pub(crate) fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            // TODO: This is only a placeholder implementation
            0x00 => self.channel1_sweep = byte,
            0x01 => self.channel1_length = byte,
            0x02 => self.channel1_volume = byte,
            0x03 => self.channel1_frequency_lo = byte,
            0x04 => self.channel1_frequency_hi = byte,

            // TODO: This is only a placeholder implementation
            0x06 => self.channel2_length = byte,
            0x07 => self.channel2_volume = byte,
            0x08 => self.channel2_frequency_lo = byte,
            0x09 => self.channel2_frequency_hi = byte,

            0x0A => self.channel3_on_off = byte.mask_or(0b1000_0000),
            0x0B => self.channel3_length = byte,
            0x0C => self.channel3_output_level = byte.mask_or(0b0110_0000),
            0x0D => self.channel3_frequency_lo = byte,
            0x0E => self.channel3_frequency_hi = byte.mask_or(0b1100_0111),

            // TODO: This is only a placeholder implementation
            0x10 => self.channel4_length = byte,
            0x11 => self.channel4_volume = byte,
            0x12 => self.channel4_polynomial_counter = byte,
            0x13 => self.channel4_counter = byte,

            // TODO: This is only a placeholder implementation
            0x14 => self.channel_control = byte,
            0x15 => self.selection_output = byte,
            0x16 => self.sound_on_off = byte,

            0x20..=0x2F => self.wave[addr - 0x20] = byte,

            _ => unreachable!(),
        }
    }
}
