pub trait Peripherals {
    type Display: Display;
    type Sound: Sound;
    type Input: Input;

    fn display(&mut self) -> &mut Self::Display;
    fn sound(&mut self) -> &mut Self::Sound;
    fn input(&mut self) -> &mut Self::Input;
}

pub trait Display {
//    fn set_pixel(...);
}

pub trait Sound {

}

pub trait Input {

}
