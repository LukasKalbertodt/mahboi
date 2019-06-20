use std::fmt::{self, Display};

use crate::{
    env::{Sound},
    primitives::{Byte, Memory, Word, CycleCounter, CYCLES_PER_FRAME, TARGET_FPS},
    log::*,
};


// TODO: Because of the lack of information some assumptions has been made which need proove:
// - Not readable registers/bits return 0xFF or 1 when read.

/// Manages the sound. This is mapped to `0xFF10..0xFF40` in the Memory.
pub struct SoundController {
    pub tone1_channel: Tone1Channel,
    pub tone2_channel: Tone2Channel,
    pub wave_channel: WaveChannel,
    pub noise_channel: NoiseChannel,

    channel_control: Byte,
    selection_output: Byte,
    sound_on_off: Byte,

    cycles: u64,
}

impl SoundController {
    pub(crate) fn new() -> Self {
        Self {
            tone1_channel: Tone1Channel::new(),
            tone2_channel: Tone2Channel::new(),
            wave_channel: WaveChannel::new(),
            noise_channel: NoiseChannel::new(),

            channel_control: Byte::zero(),
            selection_output: Byte::zero(),
            sound_on_off: Byte::zero(),

            cycles: 0,
        }
    }

    /// Loads one byte from the sound registers. The `addr` has to be between `0`
    /// and `0x30` (excluding).
    pub(crate) fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            // This returns all 1 if the sound is disabled.
            any if !self.is_enabled() && any != 0x16 => Byte::new(0xFF),

            0x00..0x05 => self.tone1_channel.load_byte(addr),
            0x05..0x0A => self.tone2_channel.load_byte(addr),
            0x0A..0x10 | 0x20..0x30 => self.wave_channel.load_byte(addr),
            0x10..0x14 => self.noise_channel.load_byte(addr),

            0x14 => self.channel_control,
            0x15 => self.selection_output,
            0x16 => self.sound_on_off.mask_or(0b1000_1111),

            _ => unreachable!(),
        }
    }

    /// Stores one byte to the sound registers. The `addr` has to be between `0`
    /// and `0x30` (excluding).
    pub(crate) fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            // This does nothing if the sound is disabled.
            any if !self.is_enabled() && any != 0x16 => {},

            0x00..0x05 => self.tone1_channel.store_byte(addr, byte),
            0x05..0x0A => self.tone2_channel.store_byte(addr, byte),
            0x0A..0x10 | 0x20..0x30 => self.wave_channel.store_byte(addr, byte),
            0x10..0x14 => self.noise_channel.store_byte(addr, byte),

            0x14 => self.channel_control = byte,
            0x15 => self.selection_output = byte,
            0x16 => self.sound_on_off = byte.mask_or(0b1000_0000),

            _ => unreachable!(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.sound_on_off.get_bit(7)
    }

    pub fn get_stereo_output(&self, channel: SoundChannel) -> StereoOutput {
        // Check on which side the sound should be played -> (left, right)
        let left_right = (
            self.selection_output.get_bit(channel.to_bit()),
            self.selection_output.get_bit(channel.to_bit() + 4),
        );

        match left_right {
            (false, false) => StereoOutput::None,
            (false, true) => StereoOutput::Right,
            (true, false) => StereoOutput::Left,
            (true, true) => StereoOutput::Center,
        }
    }

    /// Clocked by a 512 Hz timer.
    fn step_frame_sequencer(&mut self) {

    }

    /// Clocked by a 256 Hz timer.
    fn step_length_counter(&mut self) {
        self.tone1_channel.length_counter.step(&mut self.tone1_channel.enabled);
    }

    /// Clocked by a 128 Hz timer.
    fn step_sweep_timer(&mut self) {

    }

    /// Clocked by a 64 Hz timer.
    fn step_volume_envelope(&mut self) {
        self.tone1_channel.volume_envelope.step();
    }

    fn step_clocked(&mut self) {
        let clocked512 = ((CYCLES_PER_FRAME as f64 * TARGET_FPS) / 512 as f64) as u64;
        let clocked256 = ((CYCLES_PER_FRAME as f64 * TARGET_FPS) / 256 as f64) as u64;
        let clocked128 = ((CYCLES_PER_FRAME as f64 * TARGET_FPS) / 128 as f64) as u64;
        let clocked64 = ((CYCLES_PER_FRAME as f64 * TARGET_FPS) / 64 as f64) as u64;

        if self.cycles % clocked512 == 0 {
            self.step_frame_sequencer();
        }
        if self.cycles % clocked256 == 0 {
            self.step_length_counter();
        }
        if self.cycles % clocked128 == 0 {
            self.step_sweep_timer();
        }
        if self.cycles % clocked64 == 0 {
            self.step_volume_envelope();
        }
    }

    pub(crate) fn step(&mut self, sound: &mut impl Sound) {
        self.cycles += 1;
        self.step_clocked();
        
        if !self.is_enabled() {
            return;
        }

        // TODO: mix tones according to volume and stereo settings
        if self.get_stereo_output(SoundChannel::Tone1) != StereoOutput::None {
            if let Some(tone) = self.tone1_channel.get_tone() {
                sound.play_on((tone, tone), SoundChannel::Tone1);
            }
        }

//        if self.get_stereo_output(SoundChannel::Tone2) != StereoOutput::None {
//            if let Some(tone) = self.tone2_channel.get_tone() {
//                sound.play_on(tone, SoundChannel::Tone2);
//            }
//        }
//
//        if self.get_stereo_output(SoundChannel::Wave) != StereoOutput::None {
//            if let Some(tone) = self.wave_channel.get_tone() {
//                sound.play_on(tone, SoundChannel::Wave);
//            }
//        }
//
//        if self.get_stereo_output(SoundChannel::Noise) != StereoOutput::None {
//            if let Some(tone) = self.noise_channel.get_tone() {
//                sound.play_on(tone, SoundChannel::Noise);
//            }
//        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Tone {
    pub frequency: u32,
    pub volume: u8,
//    wave_pattern: [u8],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StereoOutput {
    None,
    Left,
    Center,
    Right,
}

impl StereoOutput {
    fn to_string(&self) -> String {
        match self {
            StereoOutput::None => "muted".to_string(),
            StereoOutput::Left => "left".to_string(),
            StereoOutput::Right => "right".to_string(),
            StereoOutput::Center => "center".to_string(),
        }
    }
}

impl Display for StereoOutput {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundChannel {
    Tone1,
    Tone2,
    Wave,
    Noise,
}

impl SoundChannel {
    fn to_bit(&self) -> u8 {
        match self {
            SoundChannel::Tone1 => 0,
            SoundChannel::Tone2 => 1,
            SoundChannel::Wave => 2,
            SoundChannel::Noise => 3,
        }
    }
}

pub struct LengthCounter {
    counter: u8,
    enabled: bool,
}

impl LengthCounter {
    fn new() -> Self {
        Self {
            counter: 0,
            enabled: false,
        }
    }

    fn step(&mut self, channel_enabled: &mut bool) {
        if self.enabled && self.counter > 0 {
            self.counter -= 1;
            if self.counter == 0 {
                *channel_enabled = false;
            }
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

enum VolumeEnvelopeMode {
    Increase,
    Decrease,
}

impl VolumeEnvelopeMode {
    fn from_byte(byte: Byte) -> Self {
        match byte.get_bit(3) {
            true => VolumeEnvelopeMode::Increase,
            false => VolumeEnvelopeMode::Decrease,
        }
    }
}

pub struct VolumeEnvelope {
    period: u8,
    mode: VolumeEnvelopeMode,
    volume: u8,
}

impl VolumeEnvelope {
    fn new() -> Self {
        Self {
            period: 0,
            mode: VolumeEnvelopeMode::Decrease,
            volume: 0,
        }
    }

    fn step(&mut self) {
        if self.period > 0 {
            self.period -= 1;
            match self.mode {
                VolumeEnvelopeMode::Increase if self.volume < 15 => self.volume += 1,
                VolumeEnvelopeMode::Decrease if self.volume > 0 => self.volume -= 1,
                _ => self.period = 0,
            }
        }
    }
}

pub struct Tone1Channel {
    sweep: Byte,
    length: Byte,
    volume: Byte,
    frequency_lo: Byte,
    frequency_hi: Byte,

    pub length_counter: LengthCounter,
    pub volume_envelope: VolumeEnvelope,
    enabled: bool,
}

impl Tone1Channel {
    fn new() -> Self {
        Self {
            sweep: Byte::zero(),
            length: Byte::zero(),
            volume: Byte::zero(),
            frequency_lo: Byte::zero(),
            frequency_hi: Byte::zero(),

            length_counter: LengthCounter::new(),
            volume_envelope: VolumeEnvelope::new(),
            enabled: false,
        }
    }

    pub fn get_frequency(&self) -> u32 {
        let hi = ((self.frequency_hi.get() & 0b0000_0111) as u32) << 8;
        let lo = self.frequency_lo.get() as u32;
        let freq = hi | lo;
        131072 / (2048 - freq)
    }

    fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0x00 => self.sweep.mask_or(0b0111_1111),
            0x01 => self.length.mask_or(0b1100_0000),
            0x02 => self.volume,
            0x03 => Byte::new(0xFF),
            0x04 => self.frequency_hi.mask_or(0b0100_0000),

            _ => unreachable!(),
        }
    }

    fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0x00 => self.sweep = byte.mask_or(0b0111_1111),
            0x01 => {
                self.length = byte;
                self.length_counter.counter = self.length.get_value_of_bits(0..=5);
            },
            0x02 => {
                self.volume = byte;
                self.volume_envelope.period = self.volume.get_value_of_bits(0..=2);
                self.volume_envelope.volume = self.volume.get_value_of_bits(4..=7);
                self.volume_envelope.mode = VolumeEnvelopeMode::from_byte(self.volume);
            },
            0x03 => self.frequency_lo = byte,
            0x04 => {
                self.frequency_hi = byte.mask_or(0b1100_0111);
                self.length_counter.enabled = self.frequency_hi.get_bit(6);

                // Call triggered event
                if self.frequency_hi.get_bit(7) {
                    self.trigger();
                }
            },

            _ => unreachable!(),
        }
    }

    fn trigger(&mut self) {
        self.enabled = true;
        if self.length_counter.counter == 0 {
            self.length_counter.counter = 64;
        }
        self.volume_envelope.period = self.volume.get_value_of_bits(0..=2);
        self.volume_envelope.volume = self.volume.get_value_of_bits(4..=7);

        // Instantly disable channel again, when it's DAC is off
        if !self.dac_enabled() {
            self.enabled = false;
        }
    }

    /// This represents the status of the FF26 (NR52) register "Sound n ON flag"
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn get_channel(&self) -> SoundChannel {
        SoundChannel::Tone1
    }

    // TODO: any time the DAC is off the channel is kept disabled -> to it EVERYWHERE!!!!
    fn dac_enabled(&self) -> bool {
        self.volume.get_value_of_bits(3..=7) != 0
    }

    fn get_tone(&mut self) -> Option<Tone> {
        if !self.enabled {
            return None;
        }

        Some(Tone {
            frequency: self.get_frequency(),
            volume: self.volume_envelope.volume,
        })
    }
}

pub struct Tone2Channel {
    length: Byte,
    volume: Byte,
    frequency_lo: Byte,
    frequency_hi: Byte,
    triggered: bool,
}

impl Tone2Channel {
    fn new() -> Self {
        Self {
            length: Byte::zero(),
            volume: Byte::zero(),
            frequency_lo: Byte::zero(),
            frequency_hi: Byte::zero(),
            triggered: false,
        }
    }

    pub fn get_frequency(&self) -> u32 {
        let hi = ((self.frequency_hi.get() & 0b0000_0111) as u32) << 8;
        let lo = self.frequency_lo.get() as u32;
        let freq = hi | lo;
        131072 / (2048 - freq)
    }

    fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            // TODO: This is only a placeholder implementation
            0x06 => self.length,
            0x07 => self.volume,
            0x08 => self.frequency_lo,
            0x09 => self.frequency_hi,

            _ => unreachable!(),
        }
    }

    fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            // TODO: This is only a placeholder implementation
            0x06 => self.length = byte,
            0x07 => self.volume = byte,
            0x08 => self.frequency_lo = byte,
            0x09 => {
                self.frequency_hi = byte;
                self.triggered = self.frequency_hi.get_bit(7);
            },

            _ => unreachable!(),
        }
    }

    /// This represents the status of the FF26 (NR52) register "Sound n ON flag"
    pub fn is_enabled(&self) -> bool {
        // TODO: implement the right way
        self.triggered
    }

    pub fn get_channel(&self) -> SoundChannel {
        SoundChannel::Tone2
    }

    fn get_tone(&mut self) -> Option<Tone> {
//        if !self.triggered {
//            return None;
//        }
//
//        self.triggered = false;
//        Some(Tone {
//            frequency: self.get_frequency(),
//        })
        None
    }
}

pub struct WaveChannel {
    on_off: Byte,
    length: Byte,
    output_level: Byte,
    frequency_lo: Byte,
    frequency_hi: Byte,
    wave: Memory,
    triggered: bool,
}

impl WaveChannel {
    fn new() -> Self {
        Self {
            on_off: Byte::zero(),
            length: Byte::zero(),
            output_level: Byte::zero(),
            frequency_lo: Byte::zero(),
            frequency_hi: Byte::zero(),
            wave: Memory::zeroed(Word::new(0x10)),
            triggered: false,
        }
    }

    pub fn get_frequency(&self) -> u32 {
        let hi = ((self.frequency_hi.get() & 0b0000_0111) as u32) << 8;
        let lo = self.frequency_lo.get() as u32;
        let freq = hi | lo;
        65536 / (2048 - freq)
    }

    pub fn is_on(&self) -> bool {
        self.on_off.get_bit(7)
    }

    /// This represents the status of the FF26 (NR52) register "Sound n ON flag"
    pub fn is_enabled(&self) -> bool {
        // TODO: implement the right way
        self.triggered
    }

    pub fn get_channel(&self) -> SoundChannel {
        SoundChannel::Wave
    }

    fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0x0A => self.on_off.mask_or(0b1000_0000),
            0x0B => Byte::new(0xFF),
            0x0C => self.output_level.mask_or(0b0110_0000),
            0x0D => Byte::new(0xFF),
            0x0E => self.frequency_hi.mask_or(0b1100_0111),

            0x20..0x30 => self.wave[addr],

            _ => unreachable!(),
        }
    }

    fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0x0A => self.on_off = byte.mask_or(0b1000_0000),
            0x0B => self.length = byte,
            0x0C => self.output_level = byte.mask_or(0b0110_0000),
            0x0D => self.frequency_lo = byte,
            0x0E => {
                self.frequency_hi = byte.mask_or(0b1100_0111);
                self.triggered = self.frequency_hi.get_bit(7);
            },

            0x20..0x30 => self.wave[addr - 0x20] = byte,

            _ => unreachable!(),
        }
    }

    fn get_tone(&mut self) -> Option<Tone> {
//        if !self.triggered {
//            return None;
//        }

//        self.triggered = false;
//        Some(Tone {
//            frequency: self.get_frequency(),
//        })
        None
    }
}

pub struct NoiseChannel {
    length: Byte,
    volume: Byte,
    polynomial_counter: Byte,
    counter: Byte,
    triggered: bool,
}

impl NoiseChannel {
    fn new() -> Self {
        Self {
            length: Byte::zero(),
            volume: Byte::zero(),
            polynomial_counter: Byte::zero(),
            counter: Byte::zero(),
            triggered: false,
        }
    }

    fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            // TODO: This is only a placeholder implementation
            0x10 => self.length,
            0x11 => self.volume,
            0x12 => self.polynomial_counter,
            0x13 => self.counter,

            _ => unreachable!(),
        }
    }

    fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            // TODO: This is only a placeholder implementation
            0x10 => self.length = byte,
            0x11 => self.volume = byte,
            0x12 => self.polynomial_counter = byte,
            0x13 => self.counter = byte,

            _ => unreachable!(),
        }
    }

    /// This represents the status of the FF26 (NR52) register "Sound n ON flag"
    pub fn is_enabled(&self) -> bool {
        // TODO: implement the right way
        self.triggered
    }

    pub fn get_channel(&self) -> SoundChannel {
        SoundChannel::Noise
    }

    fn get_tone(&mut self) -> Option<Tone> {
        if !self.triggered {
            return None;
        }

        self.triggered = false;
        None
    }
}
