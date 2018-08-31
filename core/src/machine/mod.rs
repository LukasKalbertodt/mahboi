use crate::{
    primitives::{Byte, Word, Memory, CycleCounter},
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
mod input;


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

    pub cycle_counter: CycleCounter,

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
    pub(crate) fn new(cartridge: Cartridge) -> Self {
        Self {
            cpu: Cpu::new(),
            cartridge,
            bios: Memory::from_bytes(
                include_bytes!(
                    concat!(env!("CARGO_MANIFEST_DIR"), "/data/DMG_BIOS_ROM.bin")
                )
            ),
            wram: Memory::zeroed(Word::new(0x2000)),
            ppu: Ppu::new(),
            io: Memory::zeroed(Word::new(0x80)),
            hram: Memory::zeroed(Word::new(0x7F)),
            interrupt_controller: InterruptController::new(),
            input_controller: InputController::new(),
            cycle_counter: CycleCounter::zero(),
            enable_interrupts_next_step: false,
            halt: false,
        }
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
    /// flags, you probably want to use the `set_flags` macro.
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
        let (lsb, msb) = word.into_bytes();
        self.f = lsb;
        self.a = msb;
    }

    pub fn zero(&self) -> bool {
        (self.f.get() & 0b1000_0000) != 0
    }

    pub fn substract(&self) -> bool {
        (self.f.get() & 0b0100_0000) != 0
    }

    pub fn half_carry(&self) -> bool {
        (self.f.get() & 0b0010_0000) != 0
    }

    pub fn carry(&self) -> bool {
        (self.f.get() & 0b0001_0000) != 0
    }
}
