use crate::{
    BiosKind,
    primitives::{Byte, Word, Memory},
    cartridge::{Cartridge},
};
use self::{
    ppu::Ppu,
    interrupt::{InterruptController, Interrupt},
    input::InputController,
};


#[macro_use]
mod macros;

mod dma;
mod mm;
pub mod ppu;
mod step;
mod interrupt;
pub mod input;


pub struct Machine {
    pub cpu: Cpu,

    pub cartridge: Cartridge,

    // TODO These should be arrays!
    pub bios: Memory,
    pub wram: Memory,
    pub io: Memory,

    pub ppu: Ppu,

    pub hram: Memory,

    pub(crate) interrupt_controller: InterruptController,
    pub(crate) input_controller: InputController,

    /// Because the EI instruction enables the interrupts during the next cycle we have to store
    /// the request for doing this. This is the purpose of this variable.
    pub enable_interrupts_next_step: bool,

    // TODO: HALT bug is not implemented!
    // An incomplete version can be found in the previous commit (58dccd7).

    /// Indicates if the machine is in HALT mode. This mode can be exited in three ways:
    ///
    /// IME is set to true
    ///     1. The CPU jumps to the next enabled and requested interrupt
    ///
    /// IME is set to false
    ///     2. (IE & IF & 0x1F) == 0 -> The CPU resumes to normal, when an enabled interrupt is
    ///                                 requested but doesn't jump to the ISR.
    ///     3. (IE & IF & 0x1F) != 0 -> HALT bug occurs: The CPU fails to increase PC when
    ///                                 executing the next instruction, so it is executed twice.
    ///                                 Examples are given in chapter 4.10. of [1].
    ///
    /// [1]: https://github.com/AntonioND/giibiiadvance/blob/master/docs/TCAGBD.pdf
    pub halt: bool,
}

impl Machine {
    pub(crate) fn new(cartridge: Cartridge, bios_kind: BiosKind) -> Self {
        let bios_bytes = match bios_kind {
            BiosKind::Original => include_bytes!(
                concat!(env!("CARGO_MANIFEST_DIR"), "/data/DMG_BIOS_ROM.bin")
            ),
            BiosKind::Minimal => include_bytes!(
                concat!(env!("CARGO_MANIFEST_DIR"), "/data/minimal-bios.bin")
            ),
        };

        Self {
            cpu: Cpu::new(),
            cartridge,
            bios: Memory::from_bytes(bios_bytes),
            wram: Memory::zeroed(Word::new(0x2000)),
            ppu: Ppu::new(),
            io: Memory::zeroed(Word::new(0x80)),
            hram: Memory::zeroed(Word::new(0x7F)),
            interrupt_controller: InterruptController::new(),
            input_controller: InputController::new(),
            enable_interrupts_next_step: false,
            halt: false,
        }
    }

    pub fn interrupt_controller(&self) -> &InterruptController {
        &self.interrupt_controller
    }

    pub fn load_word(&self, addr: Word) -> Word {
        // TODO: Check what happens on DMG hardware in this case
        if addr.get() == 0xffff {
            panic!("Index out of bounds!");
        }

        let lsb = self.load_byte(addr);
        let msb = self.load_byte(addr + 1u16);

        Word::from_bytes(lsb, msb)
    }

    pub fn store_word(&mut self, addr: Word, word: Word) {
        // TODO: Check what happens on DMG hardware in this case
        if addr.get() == 0xffff {
            panic!("Index out of bounds!");
        }

        let (lsb, msb) = word.into_bytes();
        self.store_byte(addr, lsb);
        self.store_byte(addr + 1u16, msb);
    }

    pub fn bios_mounted(&self) -> bool {
        (self.load_byte(Word::new(0xFF50)).get() & 0b0000_0001) == 0
    }

    /// Convenience method to load the value, which is stored behind the adress in HL.
    pub fn load_hl(&self) -> Byte {
        self.load_byte(self.cpu.hl())
    }

    /// Convenience method to store a value, to the adress in HL.
    pub fn store_hl(&mut self, byte: Byte) {
        self.store_byte(self.cpu.hl(), byte);
    }

    /// Pushes the given word onto the stack.
    pub fn push(&mut self, word: Word) {
        self.cpu.sp -= 2u16;
        self.store_word(self.cpu.sp, word);
    }

    /// Pops the topmost word from the stack and returns it.
    pub fn pop(&mut self) -> Word {
        let val = self.load_word(self.cpu.sp);
        self.cpu.sp += 2u16;
        val
    }

    /// Jumps to the interrupt service routine of the given interrupt and returns the number
    /// of clocks used for the jump.
    pub(crate) fn isr(&mut self, interrupt: Interrupt) -> u8 {
        // push pc onto stack
        self.push(self.cpu.pc);

        // jump to address
        self.cpu.pc = interrupt.addr();

        // reset interrupts
        self.interrupt_controller.ime = false;
        self.interrupt_controller.reset_interrupt_flag(interrupt);

        // It takes 20 clocks to dispatch a normal interrupt + 4 clocks when returning
        // from HALT mode.
        if self.halt {
            // Exit HALT mode if we are in it
            self.halt = false;
            24
        } else {
            20
        }
    }
}

pub struct Cpu {
    /// Accumulator
    pub a: Byte,

    /// Flag register.
    ///
    /// Bit 7 = zero, bit 6 = substract, bit 5 = half carry, bit 4 = carry. To
    /// access single flags, use the corresponding methods on `Cpu`. To set
    /// flags, you probably want to use the `set_flags` macro. The four lower
    /// bits have to be 0 at all times.
    pub f: Byte,

    // General purpose register
    pub b: Byte,
    pub c: Byte,
    pub d: Byte,
    pub e: Byte,
    pub h: Byte,
    pub l: Byte,

    /// Stack pointer.
    pub sp: Word,

    /// Programm counter.
    pub pc: Word,
}

impl Cpu {
    /// Returns a new CPU with all registers set to 0.
    fn new() -> Self {
        Self {
            a: Byte::zero(),
            f: Byte::zero(),
            b: Byte::zero(),
            c: Byte::zero(),
            d: Byte::zero(),
            e: Byte::zero(),
            h: Byte::zero(),
            l: Byte::zero(),
            sp: Word::zero(),
            pc: Word::zero(),
        }
    }

    pub fn hl(&self) -> Word {
        Word::from_bytes(self.l, self.h)
    }

    pub fn de(&self) -> Word {
        Word::from_bytes(self.e, self.d)
    }

    pub fn bc(&self) -> Word {
        Word::from_bytes(self.c, self.b)
    }

    pub fn af(&self) -> Word {
        Word::from_bytes(self.f, self.a)
    }

    pub fn set_hl(&mut self, word: Word) {
        let (lsb, msb) = word.into_bytes();
        self.l = lsb;
        self.h = msb;
    }

    pub fn set_de(&mut self, word: Word) {
        let (lsb, msb) = word.into_bytes();
        self.e = lsb;
        self.d = msb;
    }

    pub fn set_bc(&mut self, word: Word) {
        let (lsb, msb) = word.into_bytes();
        self.c = lsb;
        self.b = msb;
    }

    pub fn set_af(&mut self, word: Word) {
        // Only the four most significant bits are set in `F`
        let (lsb, msb) = word.into_bytes();
        self.f = Byte::new(lsb.get() & 0xF0);
        self.a = msb;
    }

    pub fn zero(&self) -> bool {
        (self.f.get() & 0b1000_0000) != 0
    }

    pub fn subtract(&self) -> bool {
        (self.f.get() & 0b0100_0000) != 0
    }

    pub fn half_carry(&self) -> bool {
        (self.f.get() & 0b0010_0000) != 0
    }

    pub fn carry(&self) -> bool {
        (self.f.get() & 0b0001_0000) != 0
    }

    /// The DAA instruction adjusts the contents of the accumulator
    /// depending on which arithmetic instruction was executed
    /// before. If SUB or SBC was executed before, the `subtract`
    /// flag is set to 1, and if ADD or ADC was used, it is set to
    /// 0.
    ///
    /// This instruction assumes that both operands of the previous
    /// operation were already in BCD form.
    ///
    /// This implementation is based on information from these
    /// sources:
    /// - https://forums.nesdev.com/viewtopic.php?f=20&t=15944
    /// - https://ehaskins.com/2018-01-30%20Z80%20DAA/
    pub(crate) fn daa(&mut self) -> bool {
        // The carry flag is only set in one specific case.
        let mut carry = false;

        if self.subtract() {
            // Subtraction: we will subtract 0, 6, 0x60 or 0x66 from
            // the accumulator. We can determine this for each digit
            // seperately.
            if self.carry() {
                self.a -= 0x60;
                carry = true;
            }

            if self.half_carry() {
                self.a -= 0x6;
            }
        } else {
            // Addition: we will add 0, 6, 0x60 or 0x66 to the
            // accumulator. We can determine this for each digit
            // seperately.
            let a_lo = self.a.get() & 0x0F;
            let a_hi = self.a.get() >> 4;

            if self.half_carry() || a_lo > 0x9 {
                self.a += 0x6;
            }

            if self.carry() || (a_hi > 0x9 && a_lo < 0xA) || (a_hi > 0x8 && a_lo > 0x9) {
                self.a += 0x60;
                carry = true;
            }
        }

        carry
    }
}

#[cfg(test)]
mod test {
    use super::*;


    #[test]
    fn test_cpu_daa() {
        fn run(sub: bool, cy: bool, h: bool, a: u8) -> (i8, bool) {
            let mut cpu = Cpu::new();
            cpu.a = Byte::new(a);
            set_flags!(cpu.f => 0 sub h cy);
            let carry = cpu.daa();
            ((cpu.a.get().wrapping_sub(a)) as i8, carry)
        }

        // ========== ADD ==========
        // CY: 0, high nybble: 0-9, H: 0, low nybble: 0-9, added: 0x00, CY result: 0
        assert_eq!(run(false, false, false, 0x00), (0x00, false));
        assert_eq!(run(false, false, false, 0x09), (0x00, false));
        assert_eq!(run(false, false, false, 0x90), (0x00, false));
        assert_eq!(run(false, false, false, 0x99), (0x00, false));

        // CY: 0, high nybble: 0-8, H: 0, low nybble: A-F, added: 0x06, CY result: 0
        assert_eq!(run(false, false, false, 0x0A), (0x06, false));
        assert_eq!(run(false, false, false, 0x0F), (0x06, false));
        assert_eq!(run(false, false, false, 0x8A), (0x06, false));
        assert_eq!(run(false, false, false, 0x8F), (0x06, false));

        // CY: 0, high nybble: 0-9, H: 1, low nybble: 0-3, added: 0x06, CY result: 0
        assert_eq!(run(false, false, true, 0x00), (0x06, false));
        assert_eq!(run(false, false, true, 0x03), (0x06, false));
        assert_eq!(run(false, false, true, 0x90), (0x06, false));
        assert_eq!(run(false, false, true, 0x93), (0x06, false));

        // CY: 0, high nybble: A-F, H: 0, low nybble: 0-9, added: 0x60, CY result: 1
        assert_eq!(run(false, false, false, 0xA0), (0x60, true));
        assert_eq!(run(false, false, false, 0xA9), (0x60, true));
        assert_eq!(run(false, false, false, 0xF0), (0x60, true));
        assert_eq!(run(false, false, false, 0xF9), (0x60, true));

        // CY: 0, high nybble: 9-F, H: 0, low nybble: A-F, added: 0x66, CY result: 1
        assert_eq!(run(false, false, false, 0x9A), (0x66, true));
        assert_eq!(run(false, false, false, 0x9F), (0x66, true));
        assert_eq!(run(false, false, false, 0xFF), (0x66, true));
        assert_eq!(run(false, false, false, 0xFF), (0x66, true));

        // CY: 0, high nybble: A-F, H: 1, low nybble: 0-3, added: 0x66, CY result: 1
        assert_eq!(run(false, false, true, 0xA0), (0x66, true));
        assert_eq!(run(false, false, true, 0xA3), (0x66, true));
        assert_eq!(run(false, false, true, 0xF0), (0x66, true));
        assert_eq!(run(false, false, true, 0xF3), (0x66, true));

        // CY: 1, high nybble: 0-2, H: 0, low nybble: 0-9, added: 0x60, CY result: 1
        assert_eq!(run(false, true, false, 0x00), (0x60, true));
        assert_eq!(run(false, true, false, 0x09), (0x60, true));
        assert_eq!(run(false, true, false, 0x20), (0x60, true));
        assert_eq!(run(false, true, false, 0x29), (0x60, true));

        // CY: 1, high nybble: 0-2, H: 0, low nybble: A-F, added: 0x66, CY result: 1
        assert_eq!(run(false, true, false, 0x0A), (0x66, true));
        assert_eq!(run(false, true, false, 0x0F), (0x66, true));
        assert_eq!(run(false, true, false, 0x2A), (0x66, true));
        assert_eq!(run(false, true, false, 0x2F), (0x66, true));

        // CY: 1, high nybble: 0-3, H: 1, low nybble: 0-3, added: 0x66, CY result: 1
        assert_eq!(run(false, true, true, 0x00), (0x66, true));
        assert_eq!(run(false, true, true, 0x03), (0x66, true));
        assert_eq!(run(false, true, true, 0x30), (0x66, true));
        assert_eq!(run(false, true, true, 0x33), (0x66, true));

        // ========== SUB ==========
        // CY: 0, high nybble: 0-9, H: 0, low nybble: 0-9, added: 0x00, CY result: 0
        assert_eq!(run(true, false, false, 0x00), (0x00, false));
        assert_eq!(run(true, false, false, 0x09), (0x00, false));
        assert_eq!(run(true, false, false, 0x90), (0x00, false));
        assert_eq!(run(true, false, false, 0x99), (0x00, false));

        // CY: 0, high nybble: 0-8, H: 1, low nybble: 6-F, added: -0x06, CY result: 0
        assert_eq!(run(true, false, true, 0x06), (-0x06, false));
        assert_eq!(run(true, false, true, 0x0F), (-0x06, false));
        assert_eq!(run(true, false, true, 0x86), (-0x06, false));
        assert_eq!(run(true, false, true, 0x8F), (-0x06, false));

        // CY: 1, high nybble: 7-F, H: 0, low nybble: 0-9, added: -0x60, CY result: 1
        assert_eq!(run(true, true, false, 0x70), (-0x60, true));
        assert_eq!(run(true, true, false, 0x79), (-0x60, true));
        assert_eq!(run(true, true, false, 0xF0), (-0x60, true));
        assert_eq!(run(true, true, false, 0xF9), (-0x60, true));

        // CY: 1, high nybble: 6-F, H: 1, low nybble: 6-F, added: -0x66, CY result: 1
        assert_eq!(run(true, true, true, 0x66), (-0x66, true));
        assert_eq!(run(true, true, true, 0x6F), (-0x66, true));
        assert_eq!(run(true, true, true, 0xF6), (-0x66, true));
        assert_eq!(run(true, true, true, 0xFF), (-0x66, true));
    }
}
