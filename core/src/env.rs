use crate::{
    primitives::{PixelColor, PixelPos},
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
   fn set_pixel(&mut self, pos: PixelPos, color: PixelColor);
}

pub trait Sound {

}

pub trait Input {

}
