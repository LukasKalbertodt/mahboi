use crate::{
    env::Display,
    primitives::{Byte, Word, Memory},
};


/// Pixel processing unit.
pub(crate) struct Ppu {
    pub vram: Memory,
    pub oam: Memory,

    // Stores the line we are currently drawing (including v-blank lines). This
    // value is always between 0 and 154 (exclusive).
    current_line: u8,
}

impl Ppu {
    pub(crate) fn new() -> Self {
        Self {
            vram: Memory::zeroed(Word::new(0x2000)),
            oam: Memory::zeroed(Word::new(0xA0)),

            current_line: 0,
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

    /// Returns in what phase the PPU currently is.
    pub fn phase(&self) -> Phase {
        if self.current_line >= 144 {
            Phase::VBlank
        } else {
            // TODO
            Phase::HBlank
        }
    }

    pub(crate) fn step(&mut self, _display: &mut impl Display) {
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
    OamSearch,
    PixelTransfer,
    HBlank,
    VBlank,
}
