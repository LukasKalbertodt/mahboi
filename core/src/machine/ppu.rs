use crate::{
    env::Display,
    primitives::{Byte, Word, Memory},
};


/// Pixel processing unit.
pub(crate) struct Ppu {
    pub vram: Memory,
    pub oam: Memory,

    /// How many cycles did we already spent in this line?
    cycle_in_line: u8,

    // ===== Registers ======
    /// FF40: LCDC
    lcd_control: Byte,

    /// FF41: LCD status
    stat: Byte,

    /// FF42: y scroll position of background
    scroll_y: Byte,

    /// FF43: x scroll position of background
    scroll_x: Byte,

    /// FF44: LY. Stores the line we are currently drawing (including v-blank
    /// lines). This value is always between 0 and 154 (exclusive).
    current_line: Byte,

    /// FF45: LY compare. Is compared to `current_line` all the time. If both
    /// values are equal, things happen.
    lyc: Byte,

    /// FF4A: Y window position
    win_y: Byte,

    /// FF4B: X window position
    win_x: Byte,
}

impl Ppu {
    pub(crate) fn new() -> Self {
        Self {
            vram: Memory::zeroed(Word::new(0x2000)),
            oam: Memory::zeroed(Word::new(0xA0)),

            cycle_in_line: 0,

            lcd_control: Byte::zero(),
            stat: Byte::zero(),
            scroll_y: Byte::zero(),
            scroll_x: Byte::zero(),
            current_line: Byte::zero(),
            lyc: Byte::zero(),
            win_y: Byte::zero(),
            win_x: Byte::zero(),
        }
    }

    /// Loads a byte from VRAM at the given (absolute!) address.
    ///
    /// The given address has to be in `0x8000..0xA000`, otherwise this
    /// function panics!
    ///
    /// This function behaves like the real VRAM. Meaning: during pixel
    /// transfer, this returns garbage.
    pub(crate) fn load_vram_byte(&self, addr: Word) -> Byte {
        match self.phase() {
            Phase::PixelTransfer => Byte::new(0xff),
            _ => self.vram[addr - 0x8000],
        }
    }

    /// Stores a byte to VRAM at the given (absolute!) address.
    ///
    /// The given address has to be in `0x8000..0xA000`, otherwise this
    /// function panics!
    ///
    /// This function behaves like the real VRAM. Meaning: during pixel
    /// transfer, this write is lost (does nothing).
    pub(crate) fn store_vram_byte(&mut self, addr: Word, byte: Byte) {
        match self.phase() {
            Phase::PixelTransfer => {},
            _ => self.vram[addr - 0x8000] = byte,
        }
    }

    /// Loads a byte from the IO port range `0xFF40..0xFF4B`.
    ///
    /// The given address has to be in `0xFF40..0xFF4B`, otherwise this
    /// function panics!
    pub(crate) fn load_io_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0xFF40 => self.lcd_control,
            0xFF41 => self.stat,
            0xFF42 => self.scroll_y,
            0xFF43 => self.scroll_x,
            0xFF44 => self.current_line,
            0xFF45 => self.lyc,
            0xFF46 => unimplemented!(), // TODO
            0xFF47 => unimplemented!(), // TODO
            0xFF48 => unimplemented!(), // TODO
            0xFF49 => unimplemented!(), // TODO
            0xFF4A => self.win_y,
            0xFF4B => self.win_x,
            _ => panic!("called `Ppu::store_io_byte` with invalid address"),
        }
    }

    /// Stores a byte in the IO port range `0xFF40..0xFF4B`.
    ///
    /// The given address has to be in `0xFF40..0xFF4B`, otherwise this
    /// function panics!
    pub(crate) fn store_io_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0xFF40 => self.lcd_control = byte,
            0xFF41 => {
                // Only bit 3 to 6 are writable
                let v = self.stat.get() & 0b0000_0111 | byte.get() & 0b0111_1000;
                self.stat = Byte::new(v);
            },
            0xFF42 => self.scroll_y = byte,
            0xFF43 => self.scroll_x = byte,
            0xFF44 => {}, // read only
            0xFF45 => self.lyc = byte,
            0xFF46 => {}, // TODO
            0xFF47 => {}, // TODO
            0xFF48 => {}, // TODO
            0xFF49 => {}, // TODO
            0xFF4A => self.win_y = byte,
            0xFF4B => self.win_x = byte,
            _ => panic!("called `Ppu::store_io_byte` with invalid address"),
        }
    }

    /// Returns in what phase the PPU currently is.
    pub fn phase(&self) -> Phase {
        match self.stat.get() & 0b11 {
            0 => Phase::HBlank,
            1 => Phase::VBlank,
            2 => Phase::OamSearch,
            3 => Phase::PixelTransfer,
            _ => unreachable!(),
        }
    }

    fn set_phase(&mut self, phase: Phase) {
        let v = match phase {
            Phase::HBlank => 0,
            Phase::VBlank => 1,
            Phase::OamSearch => 2,
            Phase::PixelTransfer => 3,
        };

        self.stat = Byte::new((self.stat.get() & 0b1111_1000) | v);
    }

    /// Executes one machine cycle (1 Mhz).
    pub(crate) fn step(&mut self, _display: &mut impl Display) {
        // Update state/phase
        self.cycle_in_line += 1;
        if self.cycle_in_line == 114 {
            self.current_line += 1;
            self.cycle_in_line = 0;

            if self.current_line == 154 {
                self.current_line = Byte::new(0);
            }
        }

        match (self.current_line.get(), self.cycle_in_line) {
            // New line
            (line, 0) if line < 144 => self.set_phase(Phase::OamSearch),

            // End of OAM search
            (line, 20) if line < 144 => self.set_phase(Phase::PixelTransfer),

            // End of pixel transfer. TODO: this is not fixed!
            (line, 63) if line < 144 => self.set_phase(Phase::HBlank),

            // Drawn all lines, starting vblank
            (144, 0) => self.set_phase(Phase::VBlank),

            _ => {}
        }

        // TODO
    }
}

/// Specifies which phase the PPU is in.
///
/// Breakdown of one frame:
///
/// ```
///   ___|-- 20 clocks --|------- 43+ clocks -------|----------- 51- clocks -----------|
///    |                 |                          |                                  |
///    |                 |                             |                               |
///    |                 |                           |                                 |
///  144       OAM       |         Pixel                 |         H-Blank             |
/// lines     Search     |        Transfer          |                                  |
///    |                 |                            |                                |
///    |                 |                             |                               |
///    |                 |                          |                                  |
///   -+-----------------+--------------------------+----------------------------------+
///    |                                                                               |
///   10                                  V-Blank                                      |
/// lines                                                                              |
///    |                                                                               |
///    +-------------------------------------------------------------------------------+
/// ```
///
/// All cycles are machine-cycles (1 Mhz = 1_048_576). Pixel transfer can vary
/// in length for different lines.
///
/// Some length:
/// - One line: 20 + 43 + 51 = 114
/// - V-Blank: 10 * one line = 1140
/// - One frame: one line * 154 = 17_556
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Also called "Mode 2": PPU determines which sprites are visible on the
    /// current line.
    OamSearch,

    /// Also called "Mode 3": Pixels are transferred to the LCD screen.
    PixelTransfer,

    /// Also called "Mode 0": Time after pixel transfer when the PPU is waiting
    /// to start a new line.
    HBlank,

    /// Also called "Mode 1": Time after the last line has been drawn and
    /// before the next frame begins.
    VBlank,
}
