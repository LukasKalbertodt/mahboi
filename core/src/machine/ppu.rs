use crate::{
    env::Display,
    // log::*,
    primitives::{Byte, Word, Memory, PixelPos, PixelColor},
};
use super::interrupt::{InterruptController, Interrupt};


/// Pixel processing unit.
pub(crate) struct Ppu {
    pub vram: Memory,
    pub oam: Memory,

    /// How many cycles did we already spent in this line?
    cycle_in_line: u8,



    // ===== State of the pixel transfer operation ======
    fifo: PixelFifo,

    /// Stores whether or not an fetch operation has already been started. This
    /// boolean usually flips every cycle during pixel transfer.
    fetch_started: bool,

    /// The first pixel of the next 8 pixel the fetcher need to fetch.
    fetch_offset: u8,

    /// ...
    current_column: u8,

    // ===== Registers ======
    /// FF40: LCDC
    lcd_control: Byte,

    /// FF41: LCD status
    status: Byte,

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

    // FF47: Background palette data.
    background_palette: Byte,

    // FF48: Sprite palette 0 data.
    sprite_palette_0: Byte,

    // FF49: Sprite palette 1 data.
    sprite_palette_1: Byte,

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

            fifo: PixelFifo::new(),
            fetch_started: false,
            fetch_offset: 0,
            current_column: 0,

            lcd_control: Byte::zero(),
            status: Byte::zero(),
            scroll_y: Byte::zero(),
            scroll_x: Byte::zero(),
            current_line: Byte::zero(),
            lyc: Byte::zero(),
            background_palette: Byte::zero(),
            sprite_palette_0: Byte::zero(),
            sprite_palette_1: Byte::zero(),
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

    /// Loads a byte from OAM at the given (absolute!) address.
    ///
    /// The given address has to be in `0xFE00..0xFEA0`, otherwise this
    /// function panics!
    ///
    /// This function behaves like the real OAM. Meaning: during pixel
    /// transfer and OAM search, this returns garbage.
    pub(crate) fn load_oam_byte(&self, addr: Word) -> Byte {
        match self.phase() {
            Phase::PixelTransfer => Byte::new(0xff),
            Phase::OamSearch => Byte::new(0xff),
            _ => self.vram[addr - 0xFE00],
        }
    }

    /// Stores a byte to OAM at the given (absolute!) address.
    ///
    /// The given address has to be in `0xFE00..0xFEA0`, otherwise this
    /// function panics!
    ///
    /// This function behaves like the real OAM. Meaning: during pixel
    /// transfer and OAM search, this write is lost (does nothing).
    pub(crate) fn store_oam_byte(&mut self, addr: Word, byte: Byte) {
        match self.phase() {
            Phase::PixelTransfer => {},
            Phase::OamSearch => {},
            _ => self.vram[addr - 0xFE00] = byte,
        }
    }

    /// Loads a byte from the IO port range `0xFF40..0xFF4B`.
    ///
    /// The given address has to be in `0xFF40..0xFF4B`, otherwise this
    /// function panics!
    pub(crate) fn load_io_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0xFF40 => self.lcd_control,
            // Bit 7 is always 1
            0xFF41 => self.status.map(|b| {
                // TODO: bit 0, 1, 2 return 0 when LCD is off
                // TODO: bit 0, 1, 2 have to be generated
                b & 0b1000_0000
            }),
            0xFF42 => self.scroll_y,
            0xFF43 => self.scroll_x,
            0xFF44 => self.current_line,
            0xFF45 => self.lyc,
            0xFF46 => unimplemented!(), // TODO
            0xFF47 => self.background_palette,
            0xFF48 => self.sprite_palette_0,
            0xFF49 => self.sprite_palette_1,
            0xFF4A => self.win_y,
            0xFF4B => self.win_x,
            _ => panic!("called `Ppu::load_io_byte` with invalid address"),
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
                let v = self.status.get() & 0b0000_0111 | byte.get() & 0b0111_1000;
                self.status = Byte::new(v);
            },
            0xFF42 => {
                // debug!("[ppu] scroll_y set to {}", byte);
                self.scroll_y = byte;
            }
            0xFF43 => {
                // debug!("[ppu] scroll_y set to {}", byte);
                self.scroll_x = byte;
            }
            0xFF44 => {}, // read only
            0xFF45 => self.lyc = byte,
            0xFF46 => {}, // TODO
            0xFF47 => self.background_palette = byte,
            0xFF48 => self.sprite_palette_0 = byte,
            0xFF49 => self.sprite_palette_1 = byte,
            0xFF4A => self.win_y = byte,
            0xFF4B => self.win_x = byte,
            _ => panic!("called `Ppu::store_io_byte` with invalid address"),
        }
    }

    /// Returns in what phase the PPU currently is.
    pub fn phase(&self) -> Phase {
        match self.status.get() & 0b11 {
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

        self.status = Byte::new((self.status.get() & 0b1111_1000) | v);
    }

    /// Executes one machine cycle (1 Mhz).
    pub(crate) fn step(
        &mut self,
        display: &mut impl Display,
        interrupt_controller: &mut InterruptController,
    ) {
        // Check if we're currently in V-Blank or not.
        if self.current_line.get() >= 144 {
            // ===== V-Blank =====
            if self.current_line == 144 && self.cycle_in_line == 0 {
                self.set_phase(Phase::VBlank);
                interrupt_controller.request_interrupt(Interrupt::Vblank);
            }
        } else {
            // ===== Not in V-Blank =====
            match (self.cycle_in_line, self.current_column) {
                (0..20, 0) => {
                    // TODO: OAM Search
                }
                (20..144, col) if col < 160 => {
                    self.pixel_transfer_step(display);
                }
                (43..144, _) => {
                    // TODO: H-Blank
                    // debug!("[ppu] hblank. current_line: {}", self.current_line);
                }
                (cycles, col) => {
                    // This state should never occur
                    panic!("internal PPU error: cycle {} of line and col {}", cycles, col);
                }
            }
        }

        // Update cycles and line
        self.cycle_in_line += 1;
        if self.cycle_in_line == 114 {
            // Bump the line and reset a bunch of values.
            self.current_line += 1;
            self.cycle_in_line = 0;
            self.current_column = 0;
            self.fetch_offset = 0;
            self.fifo.clear();

            // Reset line if we reached the last one.
            if self.current_line == 154 {
                self.current_line = Byte::new(0);
            }
        }
    }

    /// Performs one step of the pixel transfer phase. This involves fetching
    /// new tile data and emitting the pixels.
    fn pixel_transfer_step(&mut self, display: &mut impl Display) {
        // Push out up to four new pixels if we have enough data in the FIFO.
        let mut pixel_pushed = 0;
        while self.fifo.len() > 8 && pixel_pushed < 4 {
            self.push_pixel(display);
            pixel_pushed += 1;
        }

        // Fetch new data. We need two steps to perform one fetch. We just
        // don't do anything the first time `step()` is called, except for
        // setting `fetch_start = true`. The actual work is done in the second
        // step.
        if !self.fetch_started {
            self.fetch_started = true;
        } else {
            // TODO: it's a waste to calculate all of these positions every
            // time again. The `y` value doesn't change for the whole line and
            // the `x` value need to be calculate only once and can then be
            // incremented by 1 after each fetch.

            // The position of the first pixel we want to fetch in the
            // background map.
            let pos_x = (self.scroll_x + self.fetch_offset).get();
            let pos_y = (self.scroll_y + self.current_line).get();

            // Dividing by 8 and rounding down to get the position in the 32*32
            // tile grid.
            let tile_x = pos_x / 8;
            let tile_y = pos_y / 8;

            // Background map data is stored in: 0x9800 - 0x9BFF. We have to
            // lookup the index of our tile there.
            let background_addr = Word::new(0x9800 + tile_y as u16 * 32 + tile_x as u16);
            let tile_idx = self.load_vram_byte(background_addr);

            // We calculate the start address of the tile we want to load from.
            // Each tile uses 16 bytes.
            let tile_start = Word::new(0x8000 + tile_idx.get() as u16 * 16);

            // We only need to load one line (two bytes), so we need to
            // calculate that offset.
            let line_offset = tile_start + 2 * (pos_y % 8);

            // Load the two bytes and add all pixels to the FIFO.
            let lo = self.load_vram_byte(line_offset).get();
            let hi = self.load_vram_byte(line_offset + 1u8).get();
            self.fifo.add_data(hi, lo, PixelSource::Background);

            // if self.current_line == 0 {
            //     debug!(
            //         "[ppu] fetched 8 pixels. current_col: {} ,pos_x: {}, pos_y: {}, \
            //             scroll_x: {}, scroll_y: {}, tile_x: {}, tile_y: {}, bg_addr: {}, \
            //             tile_id: {}, tile_start: {}, line_offset: {}, new FIFO len: {}",
            //         self.current_column,
            //         pos_x,
            //         pos_y,
            //         self.scroll_x,
            //         self.scroll_y,
            //         tile_x,
            //         tile_y,
            //         background_addr,
            //         tile_id,
            //         tile_start,
            //         line_offset,
            //         self.fifo.len()
            //     );
            // }

            // Reset status flag and bump the fetch offset
            self.fetch_started = false;
            self.fetch_offset += 8;
        }
    }

    /// Takes the first pixel from the pixel FIFO, calculate its real color (by
    /// palette lookup) and writes that color to the display.
    fn push_pixel(&mut self, display: &mut impl Display) {
        // Converts the color number to a real color depending on the given
        // palette.
        fn pattern_to_color(pattern: u8, palette: Byte) -> PixelColor {
            // The palette contains four color values. Bit0 and bit1 define the
            // color for the color number 0, bit2 and bit3 for color number 1
            // and so on.
            let color = (palette.get() >> (pattern * 2)) & 0b11;
            PixelColor::from_greyscale(color)
        }

        // Receive pixel data from the FIFO
        let (pattern, source) = self.fifo.emit();

        // Determine the correct palette
        let palette = match source {
            PixelSource::Background => self.background_palette,
            PixelSource::Sprite0 => self.sprite_palette_0,
            PixelSource::Sprite1 => self.sprite_palette_1,
        };

        // Convert to real color
        let color = pattern_to_color(pattern, palette);

        // Write to display
        let pos = PixelPos::new(self.current_column, self.current_line.get());
        display.set_pixel(pos, color);

        self.current_column += 1;
    }
}

/// Specifies which phase the PPU is in.
///
/// Breakdown of one frame:
///
/// ```ignore
///    ┌── 20 cycles ──┬─────── 43+ cycles ───────┬─────────── 51- cycles ───────────┐
///    │               │                          |                                  │
///    │               │                             │                               │
///    │               │                           │                                 │
///  144      OAM      │         Pixel                 │         H-Blank             │
/// lines    Search    │        Transfer          │                                  │
///    │               │                            │                                │
///    │               │                             │                               │
///    │               │                          │                                  │
///    ├───────────────┴──────────────────────────┴──────────────────────────────────┤
///    │                                                                             │
///   10                                V-Blank                                      │
/// lines                                                                            │
///    │                                                                             │
///    └─────────────────────────────────────────────────────────────────────────────┘
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


#[derive(Copy, Clone, Debug, PartialEq, Eq)]
#[repr(u8)]
enum PixelSource {
    /// Pixel with background palette
    Background = 0,

    /// Sprite with palette 0
    Sprite0 = 1,

    /// Sprite with palette 1
    Sprite1 = 2,
}


/// The pixel FIFO: stores pixels to be drawn on the LCD.
///
/// The FIFO is stored in the fields `colors_hi`, `colors_lo` and `sources`.
/// Each logical element in the FIFO is a pair of a color number (0-3) and a
/// source/palette (background = 0, sprite0 = 1 or sprite1 = 2). Both of these
/// things can be encoded with 2 bits. The source is encoded in `sources`. The
/// color is encoded in `colors_hi` and `colors_lo`. The former stores the high
/// bit of the two bit number, the latter the low bit.
///
/// The following graph shows a completely-full FIFO. `hNN`, `lNN` and `sNN`
/// refer to the NNth bit of `colors_hi`, `colors_lo` and `sources`
/// respectively.
///
/// ```ignore
///  ┌───────┬───────┬───────┬───────┬───────┬───────┬───────┐
///  │  h15  │  h14  │  h13  │  ...  │  h02  │  h01  │  h00  │
///  ├───────┼───────┼───────┼───────┼───────┼───────┼───────┤
///  │  l15  │  l14  │  l13  │  ...  │  l02  │  l01  │  l00  │
///  ├───────┼───────┼───────┼───────┼───────┼───────┼───────┤
///  │s31&s30│s29&s28│s27&s26│  ...  │s05&s04│s03&s02│s01&s00│
///  └───────┴───────┴───────┴───────┴───────┴───────┴───────┘
///     ^^^                                             ^^^
///    front                                            back
/// ```
///
/// The `len` field stores how many elements are currently in the queue.
struct PixelFifo {
    // Unused bits in `colors_hi`, `colors_lo` and `sources` are always 0.
    colors_hi: u16,
    colors_lo: u16,
    sources: u32,
    len: usize,
}

impl PixelFifo {
    fn new() -> Self {
        Self {
            colors_hi: 0,
            colors_lo: 0,
            sources: 0,
            len: 0,
        }
    }

    /// Returns the current length of the FIFO.
    fn len(&self) -> u8 {
        self.len as u8
    }

    /// Clears all data from the FIFO (sets length to 0).
    fn clear(&mut self) {
        self.len = 0;
    }

    /// Removes the element at the front of the FIFO and returns it.
    ///
    /// The returned tuple contains `(color, palette)`. The color is the color
    /// pattern of the pixel (always <= 3).
    ///
    /// If this function is called when the FIFO is empty, it panics in debug
    /// mode. In release mode, the behavior is unspecified.
    fn emit(&mut self) -> (u8, PixelSource) {
        debug_assert!(self.len() > 0, "Called emit() on empty pixel FIFO");

        // Extract two bits each
        let color = ((self.colors_hi >> 14) & 0b10) | (self.colors_lo >> 15);
        let palette = match self.sources >> 30 {
            0 => PixelSource::Background,
            1 => PixelSource::Sprite0,
            2 => PixelSource::Sprite1,
            _ => panic!("internal pixel FIFO error: 4 as source"),
        };

        // Shift FIFO to the left and reduce len
        self.colors_hi <<= 1;
        self.colors_lo <<= 1;
        self.sources <<= 2;
        self.len -= 1;

        (color as u8, palette)
    }

    /// Adds data for 8 pixels.
    ///
    /// The color data for the new pixels have to be encoded in two separate
    /// bytes: `colors_hi` contains all the high bits and `color_lo` all the
    /// low bits. This means that pixel 7 is: `(hi & 1) * 2 + (lo & 1)`.
    ///
    /// If this function is called when the FIFO has less than 8 free spots, it
    /// panics in debug mode. In release mode, the behavior is unspecified.
    fn add_data(&mut self, colors_hi: u8, colors_lo: u8, source: PixelSource) {
        debug_assert!(self.len() <= 8, "called `add_data` for pixel FIFO with more than 8 pixels");

        // Build the 16 bit value from the single source.
        let mut sources = (source as u8) as u16;
        sources |= sources << 2;
        sources |= sources << 4;
        sources |= sources << 8;

        // Add the data at the correct position and increase the length
        let shift_by = 8 - self.len;
        self.colors_hi |= (colors_hi as u16) << shift_by;
        self.colors_lo |= (colors_lo as u16) << shift_by;
        self.sources |= (sources as u32) << (shift_by * 2);
        self.len += 8;
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fifo_simple() {
        let mut fifo = PixelFifo::new();
        assert_eq!(fifo.len(), 0);

        let color_hi = 0b00_11_00_11u8;
        let color_lo = 0b01_01_10_10u8;
        fifo.add_data(color_hi, color_lo, PixelSource::Background);
        assert_eq!(fifo.len(), 8);

        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b01, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b10, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b11, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b01, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b11, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b10, PixelSource::Background));
    }
}
