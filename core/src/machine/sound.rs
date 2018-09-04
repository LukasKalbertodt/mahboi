use crate::env::{Sound, Sample};

pub(crate) struct SoundController {
    clock: u32,
}

impl SoundController {
    pub(crate) fn new() -> Self {
        Self {
            clock: 0,
        }
    }

    pub(crate) fn step(&mut self, sound: &mut impl Sound) {
        sound.accept_sample(Sample((self.clock as f32 * 6.283185307).sin()));
        self.clock = self.clock.wrapping_add(1);
        println!("{}", (self.clock as f32 * 6.283185307).sin());
    }
}
