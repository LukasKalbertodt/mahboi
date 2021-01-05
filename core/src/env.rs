use crate::{
    SCREEN_WIDTH,
    primitives::PixelColor,
    machine::input::Keys,
};

pub trait Peripherals {
    /// Write one line of pixels to the Gameboy's LCD. The `line_idx` parameter
    /// determines the line (from 0 to 159 inclusive).
    fn write_lcd_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]);

    /// Returns all currently pressed keys. The emulator calls this method
    /// frequently, so the implementing type should "cache" key presses in some
    /// way to allow fast access.
    fn get_pressed_keys(&self) -> Keys;

    /// Is called regularly by the emulator (without fixed frequency, but on
    /// average above 100Mhz) to let the peripherals request an audio sample. It
    /// can call `f` at its own sample rate. It has to provide the sample rate
    /// to the function for certain audio filters within the emulator.
    fn offer_sound_sample(&mut self, f: impl FnOnce(f32) -> f32);
}
