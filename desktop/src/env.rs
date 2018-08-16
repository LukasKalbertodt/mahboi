use mahboi::env::{
    Peripherals as PeripheralsT,
    Display as DisplayT,
    Input,
    Sound as SoundT
};

pub(crate) struct Peripherals {

}

// TODO Input should be hot swapable (e.g. keyboard to controller)
impl PeripheralsT for Peripherals {
    type Display = Display;
    type Sound = Sound;
    type Input = Keyboard;

    fn display(&mut self) -> &mut crate::env::Display {
        unimplemented!()
    }

    fn sound(&mut self) -> &mut crate::env::Sound {
        unimplemented!()
    }

    fn input(&mut self) -> &mut Keyboard {
        unimplemented!()
    }
}

pub(crate) struct Display {

}

impl DisplayT for Display {

}

pub(crate) struct Keyboard {

}

impl Input for Keyboard {

}

pub(crate) struct Controller {

}

impl Input for Controller {

}

pub(crate) struct Sound {

}

impl SoundT for Sound {

}
