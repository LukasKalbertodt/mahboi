//! Mahboi!


use crate::{
    env::Peripherals,
    cartridge::{Cartridge},
    machine::{
        Machine,
        ppu::Mode,
    },
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


/// Different kinds of BIOS (boot ROMs) that can be loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BiosKind {
    /// The original BIOS scrolling in the Nintendo logo.
    Original,

    /// A BIOS just setting up internals without showing anything. Saves time!
    Minimal,
}


pub struct Emulator {
    machine: Machine,
}

impl Emulator {
    pub fn new(cartridge: Cartridge, bios: BiosKind) -> Self {
        info!("Creating emulator");

        Self {
            machine: Machine::new(cartridge, bios),
        }
    }

    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    /// Executes until the end of one frame (in most cases exactly 17,556 cycles)
    ///
    /// After executing this once, the emulator has written a new frame via the display
    /// (defined as peripherals) and the display buffer can be written to the actual display.
    #[inline(never)]
    pub fn execute_frame(
        &mut self,
        peripherals: &mut impl Peripherals,
        mut should_pause: impl FnMut(&Machine) -> bool,
    ) -> Result<(), Disruption> {
        let mut cycles = 0;
        loop {
            if should_pause(&self.machine) {
                return Err(Disruption::Paused);
            }

            // Let the CPU execute one instruction
            let cycles_spent = self.machine.step()?;

            // Let other subsystems run for the same number of cycles as the
            // CPU did.
            let vblank_before = self.machine.ppu.regs().mode() == Mode::VBlank;
            for _ in 0..cycles_spent {
                // Timer
                self.machine.timer.step(&mut self.machine.interrupt_controller);

                // PPU
                self.machine.ppu.step(peripherals, &mut self.machine.interrupt_controller);

                // OAM DMA
                self.machine.dma_step();
            }

            // Handle input
            //
            // TODO: It's a bit wasteful to check this every cycle. Normal
            // users probably wouldn't notice any difference if we would check
            // this only once per frame. However, sub frame inputs are a thing
            // in speed running. We could make this configurable.
            self.machine.input_controller.handle_input(
                peripherals,
                &mut self.machine.interrupt_controller,
            );

            // If we just entered V-Blank, we will return. This is here to get
            // the PPU and real Display synchronized.
            if !vblank_before && self.machine.ppu.regs().mode() == Mode::VBlank {
                break;
            }

            // This is just a fallback for the case that the LCD is disabled
            // the whole time or repeatedly which would mean no V-Blank is ever
            // entered. To avoid spending too many cycles in this method, we
            // return after a fixed number of cycles regardless.
            cycles += cycles_spent as u64;
            if cycles >= CYCLES_PER_FRAME {
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
