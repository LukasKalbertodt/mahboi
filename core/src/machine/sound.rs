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

    channel4_length: Byte,
    channel4_volume: Byte,
    channel4_polynomial_counter: Byte,
    channel4_counter: Byte,

    channel_control: Byte,
    selection_output: Byte,
    sound_on_off: Byte,

    wave: WaveChannel,
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
            channel4_length: Byte::zero(),
            channel4_volume: Byte::zero(),
            channel4_polynomial_counter: Byte::zero(),
            channel4_counter: Byte::zero(),
            channel_control: Byte::zero(),
            selection_output: Byte::zero(),
            sound_on_off: Byte::zero(),

            wave: WaveChannel::new(),
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

            // TODO: This is only a placeholder implementation
            0x10 => self.channel4_length,
            0x11 => self.channel4_volume,
            0x12 => self.channel4_polynomial_counter,
            0x13 => self.channel4_counter,

            // TODO: This is only a placeholder implementation
            0x14 => self.channel_control,
            0x15 => self.selection_output,
            0x16 => self.sound_on_off,

            0x0A..=0x0E | 0x20..=0x2F => self.wave.load_byte(addr),

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

            // TODO: This is only a placeholder implementation
            0x10 => self.channel4_length = byte,
            0x11 => self.channel4_volume = byte,
            0x12 => self.channel4_polynomial_counter = byte,
            0x13 => self.channel4_counter = byte,

            // TODO: This is only a placeholder implementation
            0x14 => self.channel_control = byte,
            0x15 => self.selection_output = byte,
            0x16 => self.sound_on_off = byte,

            0x0A..=0x0E | 0x20..=0x2F => self.wave.store_byte(addr, byte),

            _ => log::trace!("ignored write to {} in audio controller", addr),
        }
    }

    /// Executes one machine cycle (1,048,576 Hz) of the sound system. Returns
    /// the current sound output.
    pub(crate) fn step(&mut self) {
        // self.counter = (self.counter + 1) % 2u16.pow(13);
        self.wave.step();
    }

    pub(crate) fn output(&self) -> f32 {
        // (self.counter as f32 * 2.0 * 3.1415926 / 2u16.pow(13) as f32).sin()
        self.wave.output()
    }
}

// TODO:
// - length
// - reading and writing wave data when enabled
struct WaveChannel {
    enable: Byte,       // FF1A  E111_1111
    length: Byte,       // FF1B
    volume: Byte,       // FF1C  1VV1_1111
    freq_lo: Byte,      // FF1D  FFFF_FFFF
    control_freq: Byte, // FF1E  TL11_1FFF
    wave_table: Memory, // FF30 - FF3F

    /// Internal position counter that wraps at 32.
    position: u8,

    /// Internal timer which counts down. This value is reloaded with
    /// `(2048 - self.freq()) * 2`.
    timer: u16,
}

impl WaveChannel {
    fn new() -> Self {
        Self {
            enable: Byte::zero(),
            length: Byte::zero(),
            volume: Byte::zero(),
            freq_lo: Byte::zero(),
            control_freq: Byte::zero(),
            wave_table: Memory::zeroed(Word::new(0x10)),
            position: 0,
            timer: 0,
        }
    }

    fn freq(&self) -> u16 {
        self.freq_lo.get() as u16 + ((self.control_freq.get() as u16 & 0b111) << 8)
    }

    fn timer_reset_value(&self) -> u16 {
        // The "correct" counter value is `(2048 - freq) * 2`, but that's for
        // when the timer is decremented with 4Mhz. We only decrement with 1Mhz,
        // so we divide that by 4.
        (2048 - self.freq()) / 2
    }

    fn enabled(&self) -> bool {
        self.enable.get() & 0b1000_0000 != 0
    }

    pub(crate) fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0x0A => self.enable,
            0x0B => self.length,
            0x0C => self.volume,
            0x0D => self.freq_lo,
            0x0E => self.control_freq,
            0x20..=0x2F => self.wave_table[addr - 0x20],
            _ => unreachable!(),
        }
    }


    fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0x0A => self.enable = byte.mask_or(0b1000_0000),
            0x0B => self.length = byte,
            0x0C => self.volume = byte.mask_or(0b0110_0000),
            0x0D => self.freq_lo = byte,
            0x0E => {
                self.control_freq = byte.mask_or(0b1100_0111);
                if byte.get() & 0b1000_0000 != 0 {
                    self.trigger();
                }
            }
            0x20..=0x2F => self.wave_table[addr - 0x20] = byte,
            _ => unreachable!(),
        }
    }

    fn trigger(&mut self) {
        // TODO: "If length counter is zero, it is set to 64 (256 for wave channel)."
        self.position = 0;
        self.timer = self.timer_reset_value();
    }

    fn step(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            self.timer = self.timer_reset_value();
            self.position = (self.position + 1) % 32;
        }
    }

    fn output(&self) -> f32 {
        if !self.enabled() {
            return 0.0;
        }

        let b = self.wave_table[Word::new(self.position as u16 / 2)].get();
        let v = if self.position % 2 == 0 {
            b >> 4
        } else {
            b & 0xF
        };

        let volume = match (self.volume.get() & 0b0110_0000) >> 5 {
            0 => 0.0,
            1 => 1.0,
            2 => 0.5,
            3 => 0.25,
            _ => unreachable!(),
        };

        dac(v) * volume
    }
}

/// Mimics the digital analog converted that converts a 4 bit number into an
/// analog signal.
fn dac(input: u8) -> f32 {
    (input as f32 / 7.5) - 1.0
}
