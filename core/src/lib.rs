//! Mahboi!

#![feature(exclusive_range_pattern)]


use crate::{
    env::Peripherals,
    cartridge::{Cartridge},
    machine::Machine,
    primitives::CYCLES_PER_FRAME,
    log::*,
};


#[macro_use]
pub mod instr;

pub mod mbc;
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
    cycles_in_frame: u64,
}

impl Emulator {
    pub fn new(cartridge: Cartridge) -> Self {
        info!("Creating emulator");

        Self {
            machine: Machine::new(cartridge),
            cycles_in_frame: 0,
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
                self.machine.dma_step();
            }

            // Handle input
            self.machine.input_controller.handle_input(
                peripherals.input(),
                &mut self.machine.interrupt_controller,
            );

            self.cycles_in_frame += cycles_spent as u64;
            self.machine.cycle_counter += cycles_spent;
            if self.cycles_in_frame >= CYCLES_PER_FRAME {
                self.cycles_in_frame -= CYCLES_PER_FRAME;
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
