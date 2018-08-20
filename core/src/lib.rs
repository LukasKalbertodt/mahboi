//! Mahboi!

#![feature(exclusive_range_pattern)]
#![feature(const_fn)]


use crate::{
    env::Peripherals,
    cartridge::{Cartridge},
    machine::Machine,
    log::*,
};


pub mod log;
mod primitives;
pub mod env;
pub mod cartridge;
pub mod machine;


/// Width of the Game Boy screen in pixels.
pub const SCREEN_WIDTH: usize = 160;

/// Height of the Game Boy screen in pixels.
pub const SCREEN_HEIGHT: usize = 144;


pub struct Emulator<'a, P: 'a + Peripherals> {
    machine: Machine,

    // TODO: Remove
    #[allow(dead_code)]
    peripherals: &'a mut P,
}

impl<'a, P: 'a + Peripherals> Emulator<'a, P> {
    pub fn new(cartridge: Cartridge, peripherals: &'a mut P) -> Self {
        info!("Creating emulator");

        Self {
            machine: Machine::new(cartridge),
            peripherals,
        }
    }

    // TODO: put back in or remove
    // fn display(&mut self) -> &mut P::Display {
    //     self.peripherals.display()
    // }

    // fn sound(&mut self) -> &mut P::Sound {
    //     self.peripherals.sound()
    // }

    // fn input(&mut self) -> &mut P::Input {
    //     self.peripherals.input()
    // }

    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    /// Executes until the end of one frame (in most cases exactly 17,556 cycles)
    ///
    /// After executing this once, the emulator has written a new frame via the display
    /// (defined as peripherals) and the display buffer can be written to the actual display.
    pub fn execute_frame(&mut self) -> Result<(), Disruption> {
        loop {
            self.machine.step()?;
            if self.machine.cycle_counter.is_between_frames() {
                break;
            }
        }

        Ok(())
    }
}


/// Describes the special situation when the emulator stops unexpectedly.
pub enum Disruption {
    /// The emulator was paused, usually due to hitting a breakpoint. This
    /// means the emulator can be resumed.
    Paused,

    /// The emulation was terminated, usually because of a critical error. This
    /// means that the emulator probably can't be resumed in any useful way.
    Terminated,
}
