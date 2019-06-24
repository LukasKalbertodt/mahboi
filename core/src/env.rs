use crate::{
    SCREEN_WIDTH,
    primitives::PixelColor,
    machine::input::Keys,
};

pub trait Peripherals {
    type Display: Display;
    type Sound: Sound;
    type Input: Input;

    fn display(&mut self) -> &mut Self::Display;
    fn sound(&mut self) -> &mut Self::Sound;
    fn input(&mut self) -> &mut Self::Input;
}

pub trait Display {
    fn set_line(&mut self, line_idx: u8, pixels: &[PixelColor; SCREEN_WIDTH]);
}

pub trait Sound {

}

pub trait Input {
    fn get_pressed_keys(&self) -> Keys;
}
