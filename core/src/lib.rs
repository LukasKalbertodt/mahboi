//! Mahboi!

#![feature(exclusive_range_pattern)]
#![feature(const_fn)]


use crate::{
    env::Peripherals,
    cartridge::{Cartridge},
    machine::Machine,
    log::*,
};


#[macro_use]
pub mod instr;

pub mod log;
pub mod primitives;
pub mod env;
pub mod cartridge;
pub mod machine;


/// Width of the Game Boy screen in pixels.
pub const SCREEN_WIDTH: usize = 160;

/// Height of the Game Boy screen in pixels.
pub const SCREEN_HEIGHT: usize = 144;


pub struct Emulator {
    machine: Machine,
}

impl Emulator {
    pub fn new(cartridge: Cartridge) -> Self {
        info!("Creating emulator");

        Self {
            machine: Machine::new(cartridge),
        }
    }

    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    /// Executes until the end of one frame (in most cases exactly 17,556 cycles)
    ///
    /// After executing this once, the emulator has written a new frame via the display
    /// (defined as peripherals) and the display buffer can be written to the actual display.
    pub fn execute_frame(
        &mut self,
        peripherals: &mut impl Peripherals,
        mut should_pause: impl FnMut(&Machine) -> bool,
    ) -> Result<(), Disruption> {
        loop {
            if should_pause(&self.machine) {
                return Err(Disruption::Paused);
            }

            // Let the CPU execute one instruction
            let cycles_spent = self.machine.step()?;

            // Let the PPU run for the same number of cycles as the CPU did.
            for _ in 0..cycles_spent {
                self.machine.ppu.step(
                    peripherals.display(),
                    &mut self.machine.interrupt_controller,
                );
            }

            self.machine.cycle_counter += cycles_spent;
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
