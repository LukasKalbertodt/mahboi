use mahboi::env;

pub(crate) struct Peripherals {

}

// TODO Input should be hot swapable (e.g. keyboard to controller)
impl env::Peripherals for Peripherals {
    type Display = Display;
    type Sound = Sound;
    type Input = Input;

    fn display(&mut self) -> &mut Self::Display {
        unimplemented!()
    }

    fn sound(&mut self) -> &mut Self::Sound {
        unimplemented!()
    }

    fn input(&mut self) -> &mut Self::Input {
        unimplemented!()
    }
}

pub(crate) struct Display {

}

impl env::Display for Display {

}

pub(crate) struct Input {

}

impl env::Input for Input {

}

pub(crate) struct Sound {

}

impl env::Sound for Sound {

}
