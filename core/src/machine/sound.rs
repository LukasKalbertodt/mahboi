use crate::primitives::{Byte, Memory, Word};


// TODO: Because of the lack of information some assumptions has been made which need proove:
// - Not readable registers/bits return 0xFF or 1 when read.

/// Manages the sound. This is mapped to `0xFF10..0xFF40` in the Memory.
///
/// Unused bits read 1 and writes are ignored. In our implementation we maintain
/// that unused bits in our stored `Byte`s are indeed 1. So on read, we just
/// return them; on write we `|` the input value.
pub(crate) struct SoundController {
    channel1_sweep: Byte,
    channel1_length: Byte,
    channel1_volume: Byte,
    channel1_frequency_lo: Byte,
    channel1_frequency_hi: Byte,

    channel4_length: Byte,
    channel4_volume: Byte,
    channel4_polynomial_counter: Byte,
    channel4_counter: Byte,

    channel_control: Byte,
    selection_output: Byte,
    sound_on_off: Byte,

    square2: SquareChannel2,
    wave: WaveChannel,

    /// A counter used to generate a 512Hz clock. This is used to control length
    /// (256Hz), volume (64Hz) and sweep (128Hz) counters of the sound channels.
    /// This particular counter is incremented each machine cycle (1_048_576
    /// Hz). As the slowest clock we want to generate is 64Hz, this counter
    /// wraps at `1_048_576 / 64 = 16_384`.
    frame_sequencer: u32,

    // For highpass filter.
    last_filtered_out: f32,
    last_unfiltered_out: f32,
}

impl SoundController {
    pub(crate) fn new() -> Self {
        Self {
            channel1_sweep: Byte::zero(),
            channel1_length: Byte::zero(),
            channel1_volume: Byte::zero(),
            channel1_frequency_lo: Byte::zero(),
            channel1_frequency_hi: Byte::zero(),
            channel4_length: Byte::zero(),
            channel4_volume: Byte::zero(),
            channel4_polynomial_counter: Byte::zero(),
            channel4_counter: Byte::zero(),
            channel_control: Byte::zero(),
            selection_output: Byte::zero(),
            sound_on_off: Byte::zero(),

            square2: SquareChannel2::new(),
            wave: WaveChannel::new(),
            frame_sequencer: 0,

            last_filtered_out: 0.0,
            last_unfiltered_out: 0.0,
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
            0x10 => self.channel4_length,
            0x11 => self.channel4_volume,
            0x12 => self.channel4_polynomial_counter,
            0x13 => self.channel4_counter,

            // TODO: This is only a placeholder implementation
            0x14 => self.channel_control,
            0x15 => self.selection_output,
            0x16 => self.sound_on_off,

            0x06..=0x09 => self.square2.load_byte(addr),
            0x0A..=0x0E | 0x20..=0x2F => self.wave.load_byte(addr),

            0x17..=0x1F => todo!(),
            0x05 | 0x0F => todo!(),
            0x30..=0xFFFF => panic!("`Sound::load_byte` called with out of bounds address"),
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
            0x10 => self.channel4_length = byte,
            0x11 => self.channel4_volume = byte,
            0x12 => self.channel4_polynomial_counter = byte,
            0x13 => self.channel4_counter = byte,

            // TODO: This is only a placeholder implementation
            0x14 => self.channel_control = byte,
            0x15 => self.selection_output = byte,
            0x16 => self.sound_on_off = byte,

            0x06..=0x09 => self.square2.store_byte(addr, byte),
            0x0A..=0x0E | 0x20..=0x2F => self.wave.store_byte(addr, byte),

            _ => log::trace!("ignored write to {} in audio controller", addr),
        }
    }

    /// Executes one machine cycle (1,048,576 Hz) of the sound system. Returns
    /// the current sound output.
    pub(crate) fn step(&mut self) {
        self.frame_sequencer += 1;

        // This is the 512Hz clock (1_048_576 / 512 = 2048).
        if self.frame_sequencer % 2048 == 0 {
            let step = self.frame_sequencer / 2048;

            // 256Hz length clock.
            if step % 2 == 0 {
                self.wave.clock_length();
            }

            // 128Hz sweep clock.
            if step == 2 || step == 6 {
            }

            // 64Hz volume envelop clock.
            if step == 7 {
                self.square2.clock_volume_envelope();
            }

            // Wrap frame sequencer.
            if step == 8 {
                self.frame_sequencer = 0;
            }
        }

        self.square2.step();
        self.wave.step();
    }

    pub(crate) fn output(&mut self, sample_rate: f32) -> f32 {
        // The high-pass filter needs a parameter alpha which determines how
        // quickly the existing signal decays. This can be calculated from the
        // sample rate and the cutoff frequency. The Gameboy's cutoff frequency
        // is 60Hz according to https://github.com/bwhitman/pushpin/blob/master/src/gbsound.txt
        //
        // Resulting alpha for 60Hz is 0.9915, for 20Hz it's 0.9972.
        const CUTOFF: f32 = 60.0;
        let alpha = 1.0 / (2.0 * std::f32::consts::PI * 1.0 / sample_rate * CUTOFF + 1.0);

        // We use a simple highpass filter to mainly remove the DC component.
        let unfiltered_out = self.wave.output() + self.square2.output();
        self.last_filtered_out = alpha * self.last_filtered_out
            + alpha * (unfiltered_out - self.last_unfiltered_out);
        self.last_unfiltered_out = unfiltered_out;

        self.last_filtered_out
    }
}


/// The pulse or square-wave channel 2. This one is the same as the first one,
/// but doesn't have frequency sweep.
///
/// Things not implemented (and maybe never will, because weird):
/// - TODO: Make sure the envelop operation is over once it
///   overflows/underflows. (Is that even correct, only have one source).
/// - TODO: length timer and stuff
struct SquareChannel2 {
    // Raw registers
    duty_and_length: Byte,  // FF16   DDLL_LLLL
    volume_envelope: Byte,  // FF17   VVVV_DNNN (initial Volume, Direction, Number)
    freq_lo: Byte,          // FF18   FFFF_FFFF
    control_and_freq: Byte, // FF19   TL11_1FFF

    /// Internal "frequency" timer which counts down.
    timer: u16,

    /// The position within the 8 value waveform table. Wraps around at 8.
    position: u8,

    /// Internal volume of the volume envelope between 0 and 15.
    volume: u8,

    /// Counts down from "envelope period" to 0. When 0 is reached, it is reset
    /// and an envelop operation happens.
    volume_counter: u8,
}

impl SquareChannel2 {
    fn new() -> Self {
        Self {
            duty_and_length: Byte::zero(),
            volume_envelope: Byte::zero(),
            freq_lo: Byte::zero(),
            control_and_freq: Byte::zero(),
            timer: 0,
            position: 0,
            volume: 0,
            volume_counter: 0,
        }
    }

    pub(crate) fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0x06 => self.duty_and_length,
            0x07 => self.volume_envelope,
            0x08 => self.freq_lo,
            0x09 => self.control_and_freq,
            _ => unreachable!(),
        }
    }

    fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0x06 => self.duty_and_length = byte,
            0x07 => self.volume_envelope = byte,
            0x08 => self.freq_lo = byte,
            0x09 => {
                self.control_and_freq = byte.mask_or(0b1100_0111);
                if byte.get() & 0b1000_0000 != 0 {
                    self.trigger();
                }
            }
            _ => unreachable!(),
        }
    }

    fn reset_timer(&mut self) {
        let freq = self.freq_lo.get() as u16 + ((self.control_and_freq.get() as u16 & 0b111) << 8);
        self.timer = 2048 - freq;
    }

    fn envelope_period(&self) -> u8 {
        self.volume_envelope.get() & 0b111
    }

    fn trigger(&mut self) {
        // TODO: length stuff
        self.reset_timer();
        self.position = 0;
        self.volume = self.volume_envelope.get() >> 4;
        self.volume_counter = self.envelope_period();
    }

    fn clock_volume_envelope(&mut self) {
        if self.volume_envelope.get()  & 0b111 == 0 {
            return;
        }

        if self.volume_counter > 0 {
            self.volume_counter -= 1;
        } else {
            self.volume_counter = self.envelope_period();

            // TODO: once it overflows/underflows, the envelop operation should
            // stop.

            if self.volume_envelope.get() & 0b1000 == 0 {
                // Decrease volume
                self.volume = self.volume.saturating_sub(1);
            } else {
                // Increase volume
                if self.volume < 15 {
                    self.volume += 1;
                }
            }
        }
    }

    fn step(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            self.reset_timer();
            self.position = (self.position + 1) % 8;
        }
    }

    fn output(&self) -> f32 {
        if (self.volume_envelope.get() & 0b1111_1000) == 0 {
            return 0.0;
        }

        let waveform = match self.duty_and_length.get() >> 6 {
            0b00 => [0, 0, 0, 0, 0, 0, 0, 1],
            0b01 => [1, 0, 0, 0, 0, 0, 0, 1],
            0b10 => [1, 0, 0, 0, 0, 1, 1, 1],
            0b11 => [0, 1, 1, 1, 1, 1, 1, 0],
            _ => unreachable!(),
        };

        dac(self.volume * waveform[self.position as usize])
    }
}


/// The wave channel.
///
/// Things not implemented (and maybe never will, because weird):
/// - length
/// - "When triggering the wave channel, the first sample to play is the
///   previous one still in the high nibble of the sample buffer, and the next
///   sample is the second nibble from the wave table. This is because it
///   doesn't load the first byte on trigger like it 'should'. The first nibble
///   from the wave table is thus not played until the waveform loops."
/// - "Triggering the wave channel on the DMG while it reads a sample byte will
///   alter the first four bytes of wave RAM. If the channel was reading one of
///   the first four bytes, the only first byte will be rewritten with the byte
///   being read. If the channel was reading one of the later 12 bytes, the
///   first FOUR bytes of wave RAM will be rewritten with the four aligned bytes
///   that the read was from (bytes 4-7, 8-11, or 12-15); for example if it were
///   reading byte 9 when it was retriggered, the first four bytes would be
///   rewritten with the contents of bytes 8-11. To avoid this corruption you
///   should stop the wave by writing 0 then $80 to NR30 before triggering it
///   again. The game Duck Tales encounters this issue part way through most
///   songs."
/// - Initial wave channel data after powering on. Different between Gameboy
///   models.
/// - "CH3 output level control does not, in fact, alter the output level. It
///   shifts the digital value CH3 is outputting (read below), not the analog
///   value." -> I am not sure what this means exactly. Also: "That value is
///   digital, and can range between 0 and 0xF. This is then fed to a DAC, which
///   maps this to an analog value; 7 maps to the lowest (negative) voltage, 0
///   to the highest (positive) one.". So either the digital signal is signed
///   and that works out somehow, OR the value is indeed shifted and we rely on
///   the high-pass filter to make sure the DC offset of "25% volume" is
///   removed.
struct WaveChannel {
    enable: Byte,       // FF1A  E111_1111
    length: Byte,       // FF1B
    volume: Byte,       // FF1C  1VV1_1111
    freq_lo: Byte,      // FF1D  FFFF_FFFF
    control_freq: Byte, // FF1E  TL11_1FFF
    wave_table: Memory, // FF30 - FF3F

    /// Internal position counter that wraps at 32.
    position: u8,

    /// Internal "frequency" timer which counts down.
    timer: u16,

    // This is an internal counter which can be loaded by writing `length`.
    length_counter: u16,
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
            length_counter: 0,
        }
    }

    fn reset_timer(&mut self) {
        // The "correct" counter value is `(2048 - freq) * 2`, but that's for
        // when the timer is decremented with 4Mhz. We only decrement with 1Mhz,
        // so we divide that by 4.
        let freq = self.freq_lo.get() as u16 + ((self.control_freq.get() as u16 & 0b111) << 8);
        self.timer = (2048 - freq) / 2;
    }

    fn dac_enabled(&self) -> bool {
        self.enable.get() & 0b1000_0000 != 0
    }

    fn is_length_enabled(&self) -> bool {
        self.control_freq.get() & 0b0100_0000 != 0
    }

    pub(crate) fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0x0A => self.enable,
            0x0B => self.length,
            0x0C => self.volume,
            0x0D => self.freq_lo,
            0x0E => self.control_freq,
            0x20..=0x2F => {
                if self.dac_enabled() {
                    // This behavior is very weird and different between
                    // different Gameboy models. Returning FF is what some do,
                    // and as no game should be relying on weird behavior, we
                    // just always return FF.
                    Byte::new(0xFF)
                } else {
                    self.wave_table[addr - 0x20]
                }
            }
            _ => unreachable!(),
        }
    }

    fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0x0A => self.enable = byte.mask_or(0b1000_0000),
            0x0B => {
                self.length = byte;
                self.length_counter = 256 - byte.get() as u16;
            }
            0x0C => self.volume = byte.mask_or(0b0110_0000),
            0x0D => self.freq_lo = byte,
            0x0E => {
                self.control_freq = byte.mask_or(0b1100_0111);
                if byte.get() & 0b1000_0000 != 0 {
                    self.trigger();
                }
            }
            0x20..=0x2F => {
                // The behavior when the channel is activated is very weird and
                // different between different Gameboy models. Ignoring the
                // write is what some do, and as no game should be relying on
                // weird behavior, we ignore it.
                if !self.dac_enabled() {
                    self.wave_table[addr - 0x20] = byte;
                }
            }
            _ => unreachable!(),
        }
    }

    fn trigger(&mut self) {
        // TODO: "If length counter is zero, it is set to 64 (256 for wave channel)."
        self.position = 0;
        self.reset_timer();
    }

    fn clock_length(&mut self) {
        if self.is_length_enabled() && self.length_counter > 0 {
            self.length_counter -= 1;
        }
    }

    fn step(&mut self) {
        if self.timer > 0 {
            self.timer -= 1;
        } else {
            self.reset_timer();
            self.position = (self.position + 1) % 32;
        }
    }

    fn output(&self) -> f32 {
        if !self.dac_enabled() || (self.is_length_enabled() && self.length_counter == 0) {
            return 0.0;
        }

        let b = self.wave_table[Word::new(self.position as u16 / 2)].get();
        let v = if self.position % 2 == 0 {
            b >> 4
        } else {
            b & 0xF
        };

        // This is probably wrong, see type docs.
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
///
/// This is probably still not correct: "That value is digital, and can range
/// between 0 and 0xF. This is then fed to a DAC, which maps this to an analog
/// value; 7 maps to the lowest (negative) voltage, 0 to the highest (positive)
/// one." (This quote is strange tho, what happens with 8-F?)
fn dac(input: u8) -> f32 {
    (input as f32 / 7.5) - 1.0
}
