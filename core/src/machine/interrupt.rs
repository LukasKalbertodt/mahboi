use crate::primitives::{Byte, Word};


/// Manages the IE and IF register as well as the IME flag. This type is also responsible for
/// requesting interrupts and giving information about when an interrupt should be executed.
pub struct InterruptController {
    /// Register to enable certain interrupts. The bits in the register belong to the following
    /// interrupts:
    ///   7   6   5   4   3   2   1   0    <- Bits
    /// +---+---+---+---+---+---+---+---+
    /// | X | X | X |   |   |   |   |   |
    /// +---+---+---+---+---+---+---+---+
    ///                               ↑
    ///                           ↑   +---- V-Blank
    ///                       ↑   +---- LCD STAT
    ///                   ↑   +---- Timer
    ///               ↑   +---- Serial
    ///               +---- Joypad
    pub interrupt_enable: Byte,

    /// Register to request certain interrupts. The bit <-> interrupt relation in this register
    /// is the same as in `interrupt_enable`.
    interrupt_flag: Byte,

    /// Interrupt master enable (controlled by DI and EI instructions)
    pub ime: bool,
}

impl InterruptController {
    pub(crate) fn new() -> Self {
        InterruptController {
            // TODO: Check if this initialization is correct
            interrupt_enable: Byte::zero(),
            interrupt_flag: Byte::zero(),
            ime: false,
        }
    }

    /// Checks if an interrupt should be triggered and returns that interrupt or `None` if
    /// no interrupt should be triggered.
    pub(crate) fn should_interrupt(&self) -> Option<Interrupt> {
        if !self.ime {
            return None;
        }

        self.requested_interrupt()
    }

    /// Returns an interrupt if one is requested and enabled regardless if the IME is set,
    /// otherwise it returns `None`.
    pub(crate) fn requested_interrupt(&self) -> Option<Interrupt> {
        // Convert IE and IF register to u8 and bitwise and them both, to check, if the interrupt
        // was enabled AND requested, then mask them, to get the 5 lowest bits.
        let interrupt_enable = self.interrupt_enable.get();
        let interrupt_flag = self.interrupt_flag.get();
        let masked_interrupts = (interrupt_enable & interrupt_flag) & 0b0001_1111;

        // Match the result against the register mapping (see [`Machine::interrupt_enable`]). Due
        // to how match works, this respects the interrupt priority from the DMG CPU.
        match () {
            () if (0b0000_0001 & masked_interrupts) == 1 => Some(Interrupt::Vblank),
            () if (0b0000_0010 & masked_interrupts) == 1 => Some(Interrupt::LcdStat),
            () if (0b0000_0100 & masked_interrupts) == 1 => Some(Interrupt::Timer),
            () if (0b0000_1000 & masked_interrupts) == 1 => Some(Interrupt::Serial),
            () if (0b0001_0000 & masked_interrupts) == 1 => Some(Interrupt::Joypad),
            _ => None,
        }
    }

    /// Returns true, if at least one interrupt is enabled and requested regardless if the IME is
    /// set, otherwise it returns `false`.
    pub(crate) fn is_interrupt_requested(&self) -> bool {
        match self.requested_interrupt() {
            None => false,
            _ => true,
        }
    }

    /// Resets the corresponding flag in the IF register for the given interrupt.
    pub(crate) fn reset_interrupt_flag(&mut self, interrupt: Interrupt) {
        let mut reset_bit = |mask: u8| {
            self.interrupt_flag = self.interrupt_flag.map(|b| b & mask);
        };

        match interrupt {
            Interrupt::Vblank => reset_bit(0b0001_1110),
            Interrupt::LcdStat => reset_bit(0b0001_1101),
            Interrupt::Timer => reset_bit(0b0001_1011),
            Interrupt::Serial => reset_bit(0b0001_0111),
            Interrupt::Joypad => reset_bit(0b0000_1111),
        };
    }

    /// Returns the IF register.
    pub(crate) fn load_if(&self) -> Byte {
        // Only the 5 lower bits of this register are (R/W), the others return '1'
        // always when read.
        self.interrupt_flag.map(|b| b | 0b1110_0000)
    }

    /// Sets the given byte to the IF register.
    pub(crate) fn store_if(&mut self, byte: Byte) {
        // Only the 5 lower bits of this register are (R/W).
        self.interrupt_flag = byte.map(|b| b & 0b0001_1111);
    }

    /// This requests the given interrupt by setting the corresponding IF register bit.
    pub(crate) fn request_interrupt(&mut self, interrupt: Interrupt) {
        let mut set_bit = |mask: u8| {
            self.interrupt_flag = self.interrupt_flag.map(|b| b | mask);
        };

        match interrupt {
            Interrupt::Vblank => set_bit(0b0000_0001),
            Interrupt::LcdStat => set_bit(0b0000_0010),
            Interrupt::Timer => set_bit(0b0000_0100),
            Interrupt::Serial => set_bit(0b0000_1000),
            Interrupt::Joypad => set_bit(0b0001_0000),
        };
    }
}

/// This represents all interrupts which can occur.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Interrupt {
    Vblank,
    LcdStat,
    Timer,
    Serial,
    Joypad,
}

impl Interrupt {
    /// Returns the address of the interrupt service routine used by this interrupt.
    pub(crate) fn addr(&self) -> Word {
        match self {
            Interrupt::Vblank => Word::new(0x40),
            Interrupt::LcdStat => Word::new(0x48),
            Interrupt::Timer => Word::new(0x50),
            Interrupt::Serial => Word::new(0x58),
            Interrupt::Joypad => Word::new(0x60),
        }
    }
}
