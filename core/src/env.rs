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

pub trait Debugger {
    // This will be used by various parts of the emulator to basically "log"
    // events.
    fn post_event(&self, level: EventLevel, msg: String);

//    fn should_pause();
}

pub enum EventLevel {
    Info,
    Debug,
    /// For things that occur extremely often
    Trace,
}
