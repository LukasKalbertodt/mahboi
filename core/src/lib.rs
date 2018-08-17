//! Mahboi!

#![feature(rust_2018_preview)]
#![feature(exclusive_range_pattern)]


use crate::{
    primitives::{Byte, Addr, Memory, CycleCounter},
    env::{Peripherals, Debugger, EventLevel},
    cartridge::{Cartridge},
};


mod primitives;
pub mod env;
pub mod cartridge;


/// Width of the Game Boy screen in pixels.
pub const SCREEN_WIDTH: usize = 160;

/// Height of the Game Boy screen in pixels.
pub const SCREEN_HEIGHT: usize = 144;


pub struct Machine {
    pub cpu: Cpu,

    pub cartridge: Cartridge,

    // TODO These should be arrays!
    pub bios: Memory,
    pub vram: Memory,
    pub wram: Memory,
    pub oam: Memory,

    // TODO IO register??? 0x80 bytes
    // Register with flag for mounting/unmounting the BIOS (this is an IO register).
    // Currently this is implemented as a single bool representing the flag.
    pub bios_mounted: bool,

    pub hram: Memory,
    pub ie: Byte,

    pub cycle_counter: CycleCounter,
}

impl Machine {
    fn new(cartridge: Cartridge) -> Self {
        Self {
            cpu: Cpu::new(),
            cartridge,
            bios: Memory::from_bytes(include_bytes!("../data/DMG_BIOS_ROM.bin")),
            vram: Memory::zeroed(Addr::new(0x2000)),
            wram: Memory::zeroed(Addr::new(0x1000)),
            oam: Memory::zeroed(Addr::new(0xA0)),
            hram: Memory::zeroed(Addr::new(0x7F)),
            ie: Byte::zero(),
            cycle_counter: CycleCounter::zero(),
            bios_mounted: true,
        }
    }

    fn load_byte(&self, addr: Addr) -> Byte {
        // TODO :(
        match addr.get() {

            // ROM mounted switch
            0x0000..0x0100 if self.bios_mounted => self.bios[addr],

            0x0000..0x8000 => unimplemented!(), // Cartridge
            0x8000..0xA000 => unimplemented!(), // vram
            0xA000..0xC000 => unimplemented!(), // exram
            0xC000..0xE000 => self.wram[addr - Addr::new(0xC000)], // wram
            0xE000..0xFE00 => self.wram[addr - Addr::new(0xC000 - 0x2000)], // wram echo
            0xFE00..0xFEA0 => unimplemented!(), // oam
            0xFEA0..0xFF00 => unimplemented!(), // not usable (random ram, maybe use as rng???)
            0xFF00..0xFF80 => unimplemented!(), // IO registers
            0xFF80..0xFFFF => unimplemented!(), // hram
            0xFFFF => self.ie, // ie
            _ => unreachable!(),
        }
    }

    /// Executes one (the next) operation.
    fn step(&mut self) {
        let op_code = self.load_byte(self.cpu.pc);
        match op_code {
            _ => panic!("Unknown instruction {} in position: {}", op_code, self.cpu.pc),
        }

        self.cycle_counter.inc();
    }
}

pub struct Cpu {
    // general purpose registers
    pub a: Byte, // accumulator
    pub f: Byte, // flags: 7 = zero, 6 = substract, 5 = half carry, 4 = carry
    pub b: Byte,
    pub c: Byte,
    pub d: Byte,
    pub e: Byte,
    pub h: Byte,
    pub l: Byte,

    // addressing registers
    pub sp: Addr,
    pub pc: Addr,
}

impl Cpu {
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
            sp: Addr::zero(),
            pc: Addr::zero(),
        }
    }
}

pub struct Emulator<'a, P: 'a + Peripherals, D: 'a + Debugger> {
    machine: Machine,
    debug: &'a mut D,
    peripherals: &'a mut P,
}

impl<'a, P: 'a + Peripherals, D: 'a + Debugger> Emulator<'a, P, D> {
    pub fn new(cartridge: Cartridge, peripherals: &'a mut P, debug: &'a mut D) -> Self {
        debug.post_event(EventLevel::Trace, "Creating emulator.".into());

        let mut out = Self {
            machine: Machine::new(cartridge),
            debug,
            peripherals,
        };

        out.debug().post_event(EventLevel::Trace, "Emulator created.".into());
        out
    }

    fn display(&mut self) -> &mut P::Display {
        self.peripherals.display()
    }

    fn sound(&mut self) -> &mut P::Sound {
        self.peripherals.sound()
    }

    fn input(&mut self) -> &mut P::Input {
        self.peripherals.input()
    }

    fn debug(&mut self) -> &mut D {
        self.debug
    }

    pub fn machine(&self) -> &Machine {
        &self.machine
    }

    /// Executes until the end of one frame (in most cases exactly 17,556 cycles)
    ///
    /// After executing this once, the emulator has written a new frame via the display
    /// (defined as peripherals) and the display buffer can be written to the actual display.
    pub fn execute_frame(&mut self) {
        while !self.machine.cycle_counter.at_end_of_frame() {
            self.machine.step();
        }
    }
}
