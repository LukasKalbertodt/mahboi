use crate::{
    primitives::{Byte, Word},
    machine::interrupt::{InterruptController, Interrupt},
};


/// Manages four timer registers and is responsible for triggering the timer
/// interrupt.
pub(crate) struct Timer {
    /// FF04 DIV: Counting up at a rate of 16384Hz.
    divider: Byte,

    /// FF05 TIMA: incremented as specified by `control`.
    counter: Byte,

    /// FF06 TMA: when `counter` overflows, it is replaced with this value.
    modulo: Byte,

    /// FF07: control register
    ///
    /// - Bit 2: timer enable
    /// - Bits 1 & 0: speed of `counter` increase
    control: Byte,

    // This is an internal counter to correctly count up the divider and
    // counter.
    cycle_count: u64,
}

impl Timer {
    pub(crate) fn new() -> Self {
        Timer {
            // TODO: Check if this initialization is correct
            divider: Byte::zero(),
            counter: Byte::zero(),
            modulo: Byte::zero(),
            control: Byte::zero(),
            cycle_count: 0,
        }
    }


    /// Loads one of the timer registers. `addr` has to be between 0xFF04 and
    /// 0xFF07 (inclusive).
    pub(crate) fn load_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0xFF04 => self.divider,
            0xFF05 => self.counter,
            0xFF06 => self.modulo,
            0xFF07 => self.control,
            _ => panic!("called `Timer::load_byte` with invalid address"),
        }
    }

    /// Writes the given value to one of the timer registers. `addr` has to be
    /// between 0xFF04 and 0xFF07 (inclusive).
    pub(crate) fn store_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0xFF04 => {
                self.divider = byte;
                self.cycle_count = 0;
            }
            0xFF05 => self.counter = byte,
            0xFF06 => self.modulo = byte,
            0xFF07 => self.control = byte,
            _ => panic!("called `Timer::load_byte` with invalid address"),
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        (self.control.get() & 0b100) == 0b100
    }

    pub(crate) fn step(&mut self, interrupt_controller: &mut InterruptController) {
        // This counter counts 4Mhz cycles, but this method is only called with
        // 1Mhz.
        self.cycle_count += 4;

        if self.cycle_count % 256 == 0 {
            self.divider += 1;
        }

        if self.is_enabled() {
            let mask = match self.control.get() & 0b11 {
                0b01 => 0b1111, // divider 16
                0b10 => 0b111111, // divider 64
                0b11 => 0b11111111, // divider 256
                0b00 => 0b1111111111, // divider 1024
                _ => unreachable!(),
            };

            if (self.cycle_count & mask) == 0 {
                self.counter += 1;

                // TIMA overflowed
                if self.counter == 0 {
                    self.counter = self.modulo;
                    interrupt_controller.request_interrupt(Interrupt::Timer);
                }
            }
        }
    }
}
