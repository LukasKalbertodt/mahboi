use crate::{
    env::Display,
    log::*,
    primitives::{Byte, Word, Memory, PixelPos, PixelColor},
};


/// Pixel processing unit.
pub(crate) struct Ppu {
    pub vram: Memory,
    pub oam: Memory,

    /// How many cycles did we already spent in this line?
    cycle_in_line: u8,



    // ===== State of the pixel FIFO ======
    fifo: PixelFifo,

    /// Stores whether or not an fetch operation has already been started. This
    /// boolean usually flips every cycle during pixel transfer.
    fetch_started: bool,

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
                debug!("[ppu] scroll_y set to {}", byte);
                self.scroll_y = byte;
            }
            0xFF43 => {
                debug!("[ppu] scroll_y set to {}", byte);
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

    /// Checks if an interrupt should be triggered and if yes, returnes the
    /// address of the interrupt vector.
    pub(crate) fn should_interrupt(&self) -> Option<Byte> {
        match (self.current_line.get(), self.cycle_in_line) {
            // V-Blank interrupt
            (144, 0) => Some(Byte::new(0x40)),

            // TODO: other interrupts

            _ => None,
        }
    }

    /// Executes one machine cycle (1 Mhz).
    pub(crate) fn step(&mut self, display: &mut impl Display) {
        // Check if we're currently in V-Blank or ont.
        if self.current_line.get() >= 144 {
            // ===== V-Blank =====
            if self.current_line == 144 && self.cycle_in_line == 0 {
                self.set_phase(Phase::VBlank);
                // TODO: trigger interrupt
            }
        } else {
            // ===== Not in V-Blank =====
            match (self.cycle_in_line, self.current_column) {
                (0..20, 0) => {
                    // TODO: OAM Search
                }
                (20..144, col) if col < 160 => {
                    self.fifo_step(display);
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

        // match (self.current_line.get(), self.cycle_in_line) {
        //     // New line, we start OAM search
        //     (0..144, 0..20) => {
        //         // TODO: OAM Search
        //         self.set_phase(Phase::OamSearch);
        //     }

        //     // End of OAM search
        //     (0..144, 20..) => {
        //         self.set_phase(Phase::PixelTransfer);
        //     }

        //     // TODO: hblank
        //     // // End of pixel transfer. TODO: this is not fixed!
        //     // (0..144, 63) => self.set_phase(Phase::HBlank),

        //     // Drawn all lines, starting vblank
        //     (144, 0) => {
        //         self.set_phase(Phase::VBlank);
        //         // TODO: Trigger interrupt
        //     }

        //     _ => {}
        // }

        // Update state/phase
        self.cycle_in_line += 1;
        if self.cycle_in_line == 114 {
            self.current_line += 1;
            self.cycle_in_line = 0;
            self.current_column = 0;
            self.fetch_offset = 0;
            self.fifo.clear();
            // debug!("NEW LINE {} ---------------------------------------------", self.current_line);

            if self.current_line == 154 {
                self.current_line = Byte::new(0);
            }
        }


    }

    fn fifo_step(&mut self, display: &mut impl Display) {
        // Push out up to four new pixels if we have enough data in the FIFO.
        let mut pixel_pushed = 0;
        while self.fifo.len() > 8 && pixel_pushed < 4 {
            self.push_pixel(display);
            pixel_pushed += 1;
        }

        // Fetch new data. We need two steps to perform once fetch. We just
        // don't do anything the first time `step()` except for setting
        // `fetch_start = true`. The actual work is done in the second step.
        if !self.fetch_started {
            self.fetch_started = true;
        } else {
            let pos_x = (self.scroll_x + self.fetch_offset).get();
            let pos_y = (self.scroll_y + self.current_line).get();

            let tile_x = pos_x / 8;
            let tile_y = pos_y / 8;

            // Background map data is stored in: 0x9800 - 0x9BFF
            let background_addr = Word::new(0x9800 + tile_y as u16 * 32 + tile_x as u16);
            let tile_id = self.load_vram_byte(background_addr);

            // We calculate the start address of the tile we want to load from.
            // Each tile uses 16 bytes.
            let tile_start = Word::new(0x8000 + tile_id.get() as u16 * 16);

            // We only need to load one line (two bytes), so we need to
            // calculate that offset.
            let line_offset = tile_start + 2 * (pos_y % 8);
            let byte0 = self.load_vram_byte(line_offset).get();
            let byte1 = self.load_vram_byte(line_offset + 1u8).get();

            // The color number of each pixel is split between the bytes:
            // `byte0` defines the lower bit of the color number, while `byte1`
            // defines the upper bit.
            let new_pixels = [
                (ColorPattern::from_byte(((byte0 >> 7) & 0b1) | (((byte1 >> 7) & 0b1) << 1)), PixelSource::Background),
                (ColorPattern::from_byte(((byte0 >> 6) & 0b1) | (((byte1 >> 6) & 0b1) << 1)), PixelSource::Background),
                (ColorPattern::from_byte(((byte0 >> 5) & 0b1) | (((byte1 >> 5) & 0b1) << 1)), PixelSource::Background),
                (ColorPattern::from_byte(((byte0 >> 4) & 0b1) | (((byte1 >> 4) & 0b1) << 1)), PixelSource::Background),
                (ColorPattern::from_byte(((byte0 >> 3) & 0b1) | (((byte1 >> 3) & 0b1) << 1)), PixelSource::Background),
                (ColorPattern::from_byte(((byte0 >> 2) & 0b1) | (((byte1 >> 2) & 0b1) << 1)), PixelSource::Background),
                (ColorPattern::from_byte(((byte0 >> 1) & 0b1) | (((byte1 >> 1) & 0b1) << 1)), PixelSource::Background),
                (ColorPattern::from_byte(((byte0 >> 0) & 0b1) | (((byte1 >> 0) & 0b1) << 1)), PixelSource::Background),
            ];
            // [(ColorPattern, PixelSource); 8]
            self.fifo.add_data(new_pixels);
            // if self.current_line == 0 {
            //     debug!(
            //         "[ppu] fetched 8 pixels. current_col: {} ,pos_x: {}, pos_y: {}, scroll_x: {}, scroll_y: {}, tile_x: {}, tile_y: {}\
            //             , bg_addr: {}, tile_id: {}, tile_start: {}, line_offset: {}, new FIFO len: {}",
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

            // Reset status flag
            self.fetch_started = false;
            self.fetch_offset += 8;
        }
    }

    fn push_pixel(&mut self, display: &mut impl Display) {
        fn pattern_to_color(pattern: ColorPattern, palette: Byte) -> PixelColor {
            let color = (palette.get() >> (pattern.as_byte() * 2)) & 0b11;

            PixelColor::from_greyscale(color)
        }

        let (pattern, source) = self.fifo.emit();
        let color = match source {
            PixelSource::Background => pattern_to_color(pattern, self.background_palette),
            PixelSource::Sprite0 => pattern_to_color(pattern, self.sprite_palette_0),
            PixelSource::Sprite1 => pattern_to_color(pattern, self.sprite_palette_1),
        };

        let pos = PixelPos::new(self.current_column, self.current_line.get());
        display.set_pixel(pos, color);
        self.current_column += 1;
        // if pattern != ColorPattern::Pat00 {
        //     debug!("[ppu] pushed pixel {:?} to pattern {:?}", pos, pattern);
        // }
    }
}

/// Specifies which phase the PPU is in.
///
/// Breakdown of one frame:
///
/// ```ignore
///   ___|-- 20 cycles --|------- 43+ cycles -------|----------- 51- cycles -----------|
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

#[derive(Copy, Clone, Debug, PartialEq)]
enum ColorPattern {
    Pat00,
    Pat01,
    Pat10,
    Pat11,
}

impl ColorPattern {
    fn from_byte(b: u8) -> Self {
        match b {
            0 => ColorPattern::Pat00,
            1 => ColorPattern::Pat01,
            2 => ColorPattern::Pat10,
            3 => ColorPattern::Pat11,
            _ => panic!("called `ColorPattern::from_byte` with byte > 3"),
        }
    }

    fn as_byte(&self) -> u8 {
        match self {
            ColorPattern::Pat00 => 0,
            ColorPattern::Pat01 => 1,
            ColorPattern::Pat10 => 2,
            ColorPattern::Pat11 => 3,
        }
    }
}

#[derive(Copy, Clone, Debug)]
enum PixelSource {
    /// Pixel with background palette
    Background,

    /// Sprite with palette 0
    Sprite0,

    /// Sprite with palette 1
    Sprite1,
}

struct PixelFifo {
    data: [(ColorPattern, PixelSource); 16],
    start: usize,
    len: usize,
}

impl PixelFifo {
    fn new() -> Self {
        Self {
            // Dummy data
            data: [(ColorPattern::Pat00, PixelSource::Background); 16],
            start: 0,
            len: 0,
        }
    }

    fn len(&self) -> u8 {
        self.len as u8
    }

    fn clear(&mut self) {
        self.start = 0;
        self.len = 0;
    }

    fn emit(&mut self) -> (ColorPattern, PixelSource) {
        assert!(self.len() > 0, "Called emit() on empty pixel FIFO");

        let out = self.data[self.start];
        self.len -= 1;
        self.start += 1;
        if self.start == self.data.len() {
            self.start = 0;
        }

        out
    }

    fn add_data(&mut self, data: [(ColorPattern, PixelSource); 8]) {
        assert!(self.len() <= 8, "called `add_data` for pixel FIFO with more than 8 pixels");

        for (i, &elem) in data.iter().enumerate() {
            self.data[(self.start + self.len + i) % 16] = elem;
        }
        self.len += 8;
    }
}
