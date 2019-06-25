//! Everything related to the pixel processing unit (PPU).

use std::{
    fmt,
    ops::Range,
};

use crate::{
    SCREEN_HEIGHT, SCREEN_WIDTH,
    env::Display,
    log::*,
    primitives::{Byte, Word, Memory, PixelColor},
};
use super::interrupt::{InterruptController, Interrupt};



/// Number of 1MHz cycles per line.
const CYCLES_PER_LINE: u8 = 114;

/// Number of lines including the "V-Blank lines". After drawing the 144 lines
/// on the LCD, the PPU has a V-Blank phase which lasts exactly
/// `10 * CYCLES_PER_LINE`. These are the counted as lines, too, despite no
/// lines being drawn.
const NUM_LINES: u8 = 154;

/// The number of tiles in a background or window map in each dimension.
/// Meaning: the background map is 32 * 32 tiles large.
const MAP_SIZE: u8 = 32;

/// The width of a sprite in pixels.
const SPRITE_WIDTH: u8 = 8;



/// The (public) registers inside of the PPU.
pub struct PpuRegisters {
    /// `0xFF40`: LCD control. All bits can be written.
    ///
    /// Each bit is used for a different purpose:
    /// - 7: LCD display enable (0=Off, 1=On)
    /// - 6: window tile map select (0=9800-9BFF, 1=9C00-9FFF)
    /// - 5: window display enable (0=Off, 1=On)
    /// - 4: background and window tile data select (0=8800-97FF, 1=8000-8FFF)
    /// - 3: background tile map select (0=9800-9BFF, 1=9C00-9FFF)
    /// - 2: sprite size (0=8x8, 1=8x16)
    /// - 1: sprite display enable (0=Off, 1=On)
    /// - 0: different meaning depending on Gameboy model
    pub lcd_control: Byte,

    /// `0xFF41`: LCD/PPU status. Bits 3, 4, 5 and 6 can be written.
    ///
    /// Purpose of each bit:
    /// - 7: always 1, writes are ignored.
    /// - 6: LYC=LY coincidence interrupt (1=enabled)
    /// - 5: OAM search interrupt (1=enabled)
    /// - 4: V-Blank interrupt (1=enabled)
    /// - 3: H-Blank interrupt (1=enabled)
    /// - 2: coincidence flag (0=LYC!=LY, 1=LYC==LY). Read only.
    /// - 1 & 0: current PPU mode. Modes 0 -- 3, see [`Mode`] for more
    ///   information. Read only.
    pub status: Byte,

    /// `0xFF42`: y scroll position of background.
    pub scroll_bg_y: Byte,

    /// `0xFF43`: x scroll position of background.
    pub scroll_bg_x: Byte,

    /// `0xFF44`: LY. Stores the line we are currently drawing (including
    /// V-blank lines). This value is always between 0 and 154 (exclusive).
    /// Read only.
    pub current_line: Byte,

    /// `0xFF45`: LY compare. Is compared to `current_line` all the time. If
    /// both values are equal, things happen (see `status` register).
    pub lyc: Byte,

    /// `0xFF46`: OAM DMA transfer start address register. This value times
    /// `0x100` is the start address from which OAM data is read during the the
    /// DMA transfer. Writing to this triggers DMA.
    pub oam_dma_start: Byte,

    /// `0xFF47`: background palette data.
    pub background_palette: Byte,

    /// `0xFF48`: sprite palette 0 data.
    pub sprite_palette_0: Byte,

    /// `0xFF49`: sprite palette 1 data.
    pub sprite_palette_1: Byte,

    /// `0xFF4A`: Y window position
    pub scroll_win_y: Byte,

    /// `0xFF4B`: X window position
    pub scroll_win_x: Byte,
}

impl PpuRegisters {
    fn new() -> Self {
        Self {
            lcd_control: Byte::zero(),
            status: Byte::zero(),
            scroll_bg_y: Byte::zero(),
            scroll_bg_x: Byte::zero(),
            current_line: Byte::zero(),
            lyc: Byte::zero(),
            oam_dma_start: Byte::zero(),
            background_palette: Byte::zero(),
            sprite_palette_0: Byte::zero(),
            sprite_palette_1: Byte::zero(),
            scroll_win_y: Byte::zero(),
            scroll_win_x: Byte::zero(),
        }
    }

    /// Returns bit 7 of the LCD control register which determines if the LCD
    /// is enabled.
    pub fn is_lcd_enabled(&self) -> bool {
        self.lcd_control.get() & 0b1000_0000 != 0
    }

    /// Returns bit 5 of the LCD control register which determines if the
    /// window layer is enabled.
    pub fn is_window_enabled(&self) -> bool {
        self.lcd_control.get() & 0b0010_0000 != 0
    }

    /// Returns bit 1 of the LCD control register which determines if sprite
    /// rendering is enabled.
    pub fn are_sprites_enabled(&self) -> bool {
        self.lcd_control.get() & 0b0000_0010 != 0
    }

    /// Returns the height of all sprites. This can either be 8 or 16,
    /// controlled by bit 3 of the LCD control register.
    pub fn sprite_height(&self) -> u8 {
        if self.lcd_control.get() & 0b0000_0100 == 0 {
            8
        } else {
            16
        }
    }

    /// Returns the memory area of the tile map for the window layer (as
    /// determined by LCD control bit 6).
    pub fn window_tile_map_address(&self) -> TileMapArea {
        if self.lcd_control.get() & 0b0100_0000 == 0 {
            TileMapArea::Low
        } else {
            TileMapArea::High
        }
    }

    /// Returns the memory area of the tile map for the background layer (as
    /// determined by LCD control bit 3).
    pub fn bg_tile_map_address(&self) -> TileMapArea {
        if self.lcd_control.get() & 0b0000_1000 == 0 {
            TileMapArea::Low
        } else {
            TileMapArea::High
        }
    }

    /// Returns the memory area of the tile data for the background and window
    /// layer (as determined by LCD control bit 4).
    pub fn bg_window_tile_data_address(&self) -> TileDataArea {
        // Yes, 0 means the higher address range.
        if self.lcd_control.get() & 0b0001_0000 == 0 {
            TileDataArea::High
        } else {
            TileDataArea::Low
        }
    }

    /// Returns if large sprites (8x16) are enabled (instead of 8x8 sprites).
    /// This is determined by bit 2 of the LCD control register.
    pub fn large_sprites_enabled(&self) -> bool {
        self.lcd_control.get() & 0b0000_0100 != 0
    }

    /// Returns `true` if the LY=LYC coincidence interrupt is enabled (as
    /// determined by bit 6 of the LCD stat register).
    pub fn coincidence_interrupt(&self) -> bool {
        self.status.get() & 0b0100_0000 != 0
    }

    /// Returns `true` if the OAM search interrupt is enabled (as determined by
    /// bit 5 of the LCD stat register).
    pub fn oam_search_interrupt(&self) -> bool {
        self.status.get() & 0b0010_0000 != 0
    }

    /// Returns `true` if the V-Blank interrupt is enabled (as determined by
    /// bit 4 of the LCD stat register). Note that this interrupt is part of
    /// the 0x48 LCD status interrupt. There is another V-Blank interrupt
    /// (0x40) that is independent from this.
    pub fn vblank_interrupt(&self) -> bool {
        self.status.get() & 0b0001_0000 != 0
    }

    /// Returns `true` if the H-Blank interrupt is enabled (as determined by
    /// bit 3 of the LCD stat register).
    pub fn hblank_interrupt(&self) -> bool {
        self.status.get() & 0b0000_1000 != 0
    }

    /// Returns the mode of the PPU (as determined by bits 1 & 0 from the LCD
    /// stat register). See [`Mode`] for more information.
    pub fn mode(&self) -> Mode {
        match self.status.get() & 0b11 {
            0 => Mode::HBlank,
            1 => Mode::VBlank,
            2 => Mode::OamSearch,
            3 => Mode::PixelTransfer,
            _ => unreachable!(),
        }
    }

    /// Sets the given mode (updates bits 1 & 0 in the LCD stat register).
    fn set_mode(&mut self, mode: Mode) {
        let v = mode as u8;
        self.status = self.status.map(|b| (b & 0b1111_1100) | v);
    }

    fn set_coincidence_flag(&mut self, v: bool) {
        self.status = self.status.map(|b| {
            if v {
                b | 0b0000_0100
            } else {
                b & 0b1111_1011
            }
        });
    }
}

/// The memory area in VRAM where a tile map is stored (the index into the tile
/// data array).
pub enum TileMapArea {
    /// Stored in `0x9800` - `0x9BFF`.
    Low,
    /// Stored in `0x9C00` - `0x9FFF`.
    High,
}

impl TileMapArea {
    /// Returns the memory range (absolute addresses).
    pub fn absolute(&self) -> Range<Word> {
        match self {
            TileMapArea::Low  => Word::new(0x9800)..Word::new(0x9C00),
            TileMapArea::High => Word::new(0x9C00)..Word::new(0xA000),
        }
    }

    /// Returns the start of this memory area, relative to the beginning of
    /// VRAM.
    fn start(&self) -> Word {
        match self {
            TileMapArea::Low  => Word::new(0x1800),
            TileMapArea::High => Word::new(0x1C00),
        }
    }
}

impl fmt::Display for TileMapArea {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let range = self.absolute();
        write!(f, "{:04x}-{:04x}", range.start.get(), range.end.get() - 1)
    }
}

/// The memory area in VRAM where tile data is stored (the actual pixel data
/// for the 8x8 tiles).
pub enum TileDataArea {
    /// Stored in `0x8000` - `0x8FFF`.
    Low,
    /// Stored in `0x8800` - `0x97FF`.
    High,
}

impl TileDataArea {
    /// Returns the memory range (absolute addresses).
    pub fn absolute(&self) -> Range<Word> {
        match self {
            TileDataArea::Low  => Word::new(0x8000)..Word::new(0x9000),
            TileDataArea::High => Word::new(0x9000)..Word::new(0x9800),
        }
    }

    /// Returns the address (relative to the beginning of VRAM) of the tile
    /// with the given index.
    ///
    /// This implements the difference between the two addressing modes. If
    /// `self` is `High`, the given byte is used as signed offset from `0x9000`
    /// as base pointer.
    fn index(&self, idx: Byte) -> Word {
        match self {
            TileDataArea::Low => {
                // Simple indexing: we start at the very beginning of the VRAM
                // and each tile needs 16 byte.
                Word::new(idx.get() as u16 * 16)
            }
            TileDataArea::High => {
                // In 8800 addressing mode, things are more complicated: we use
                // `0x9000` as base address and the `idx` is now used as signed
                // index.
                let offset = ((idx.get() as i8) as i16) * 16;
                Word::new((0x1000 + offset) as u16)
            }
        }
    }
}

impl fmt::Display for TileDataArea {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let range = self.absolute();
        write!(f, "{:04x}-{:04x}", range.start.get(), range.end.get() - 1)
    }
}

/// Pixel processing unit.
pub struct Ppu {
    pub vram: Memory,
    pub oam: Memory,

    /// How many cycles did we already spent in this line?
    cycle_in_line: u8,

    /// The cycle of the line in which hblank starts. This is updated for each
    /// line after the pixel transfer mode.
    hblank_trigger: u8,

    sprites_on_line: [Sprite; 10],

    /// If an DMA is ongoing, this stores the address of the next source byte.
    /// The DMA copies from 0xXX00 to 0xXXF1. The first cycle of the DMA
    /// procedure is spent preparing. Starting with the second cycles, one byte
    /// is copied per cycle. When the DMA is freshly triggered, the value in
    /// this `Option` is 0xXXFF: one less than the real start address. That's
    /// for the setup time.
    pub(crate) oam_dma_status: Option<Word>,

    /// All registers. If you want to read registers, use the `regs()` method
    /// instead. That way, we can avoid accidental mutation of any registers.
    registers: PpuRegisters,
}


impl Ppu {
    pub(crate) fn new() -> Self {
        Self {
            vram: Memory::zeroed(Word::new(0x2000)),
            oam: Memory::zeroed(Word::new(0xA0)),

            cycle_in_line: 0,

            // It will be overwritten with a smaller number before becoming
            // relevant.
            hblank_trigger: 255,
            sprites_on_line: [Sprite::invisible(); 10],

            oam_dma_status: None,
            registers: PpuRegisters::new(),
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
        match self.regs().mode() {
            Mode::PixelTransfer if self.regs().is_lcd_enabled() => Byte::new(0xff),
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
        match self.regs().mode() {
            Mode::PixelTransfer if self.regs().is_lcd_enabled() => {},
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
        match self.regs().mode() {
            Mode::PixelTransfer | Mode::OamSearch
                if self.regs().is_lcd_enabled() => Byte::new(0xff),
            _ => self.oam[addr - 0xFE00],
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
        match self.regs().mode() {
            Mode::PixelTransfer | Mode::OamSearch if self.regs().is_lcd_enabled() => {},
            _ => self.oam[addr - 0xFE00] = byte,
        }
    }

    /// Loads a byte from the IO port range `0xFF40..0xFF4B`.
    ///
    /// The given address has to be in `0xFF40..0xFF4B`, otherwise this
    /// function panics!
    pub(crate) fn load_io_byte(&self, addr: Word) -> Byte {
        match addr.get() {
            0xFF40 => self.regs().lcd_control,
            // Bit 7 is always 1
            0xFF41 => self.regs().status.map(|mut b| {
                // Bit 7 always returns 1
                b |= 0b1000_0000;
                if !self.regs().is_lcd_enabled() {
                    // Bit 0, 1 and 2 return 0 when LCD is off
                    b &= 0b1111_1000;
                }

                b
            }),
            0xFF42 => self.regs().scroll_bg_y,
            0xFF43 => self.regs().scroll_bg_x,
            0xFF44 => self.regs().current_line,
            0xFF45 => self.regs().lyc,
            0xFF46 => self.regs().oam_dma_start,
            0xFF47 => self.regs().background_palette,
            0xFF48 => self.regs().sprite_palette_0,
            0xFF49 => self.regs().sprite_palette_1,
            0xFF4A => self.regs().scroll_win_y,
            0xFF4B => self.regs().scroll_win_x,
            _ => panic!("called `Ppu::load_io_byte` with invalid address"),
        }
    }

    /// Stores a byte in the IO port range `0xFF40..0xFF4B`.
    ///
    /// The given address has to be in `0xFF40..0xFF4B`, otherwise this
    /// function panics!
    pub(crate) fn store_io_byte(&mut self, addr: Word, byte: Byte) {
        match addr.get() {
            0xFF40 => {
                let was_enabled = self.regs().is_lcd_enabled();
                self.registers.lcd_control = byte;
                match (was_enabled, self.regs().is_lcd_enabled()) {
                    (false, true) => {
                        info!("[ppu] LCD was enabled");
                        self.registers.set_mode(Mode::OamSearch);
                        self.cycle_in_line = 0;
                        // TODO: also reset other stuff?
                    }
                    (true, false) => {
                        info!("[ppu] LCD was disabled");
                        self.registers.current_line = Byte::new(0);
                    }
                    _ => {}
                }
            }
            0xFF41 => {
                // Only bit 3 to 6 are writable
                let v = self.regs().status.get() & 0b0000_0111 | byte.get() & 0b0111_1000;
                self.registers.status = Byte::new(v);
            },
            0xFF42 => self.registers.scroll_bg_y = byte,
            0xFF43 => self.registers.scroll_bg_x = byte,
            0xFF44 => {}, // read only
            0xFF45 => self.registers.lyc = byte,
            0xFF46 => {
                self.registers.oam_dma_start = byte;
                let src_addr = Word::new((byte.get() as u16) * 0x100) - 1;
                self.oam_dma_status = Some(src_addr);
                trace!("DMA started from {:?}", src_addr);
            },
            0xFF47 => self.registers.background_palette = byte,
            0xFF48 => self.registers.sprite_palette_0 = byte,
            0xFF49 => self.registers.sprite_palette_1 = byte,
            0xFF4A => self.registers.scroll_win_y = byte,
            0xFF4B => self.registers.scroll_win_x = byte,
            _ => panic!("called `Ppu::store_io_byte` with invalid address"),
        }
    }

    /// Disables the LCD by writing 0 to `FF40.7`.
    pub fn disable(&mut self) {
        let new_val = self.regs().lcd_control.map(|b| b & 0b0111_1111);
        self.store_io_byte(Word::new(0xFF40), new_val);
    }

    /// Enables the LCD by writing 1 to `FF40.7`.
    pub fn enable(&mut self) {
        let new_val = self.regs().lcd_control.map(|b| b | 0b1000_0000);
        self.store_io_byte(Word::new(0xFF40), new_val);
    }

    /// Returns an immutable reference to all public registers.
    pub fn regs(&self) -> &PpuRegisters {
        &self.registers
    }

    /// Executes one machine cycle (1 Mhz).
    pub(crate) fn step(
        &mut self,
        display: &mut impl Display,
        interrupt_controller: &mut InterruptController,
    ) {
        // If the whole LCD is disabled, the PPU does nothing
        if !self.regs().is_lcd_enabled() {
            return;
        }

        let line = self.regs().current_line.get();
        match self.cycle_in_line {
            // ===== Start of OAM search =====================================
            0 if line < SCREEN_HEIGHT as u8 => {
                self.registers.set_mode(Mode::OamSearch);

                // Potentially trigger LCD stat interrupt. TODO: this
                // might be only correct for line 0. This might happen
                // one cycle earlier for lines 1--143. Check cycle
                // accurate gameboy docs later.
                if self.regs().oam_search_interrupt() {
                    interrupt_controller.request_interrupt(Interrupt::LcdStat);
                }

                // Check if we just started the line with the same
                // number as LYC.
                if self.regs().current_line == self.regs().lyc {
                    self.registers.set_coincidence_flag(true);

                    // Potentially trigger interrupt. TODO: this might
                    // be only correct for line 0. This might happen
                    // one cycle earlier for lines 1--143. Check cycle
                    // accurate gameboy docs later.
                    if self.regs().coincidence_interrupt() {
                        interrupt_controller.request_interrupt(Interrupt::LcdStat);
                    }
                } else {
                    self.registers.set_coincidence_flag(false);
                }

                // The real hardware performs this in the following 20
                // cycles, but we can do it in one step as the result of
                // this operation is not observable before pixel transfer
                // and OAM memory cannot be written during the OAM search
                // phase.
                self.do_oam_search();
            }

            // ===== Start of pixel transfer =================================
            20 if line < SCREEN_HEIGHT as u8 => {
                // TODO: trigger STAT interrupt here?
                self.registers.set_mode(Mode::PixelTransfer);
                let cycles = self.do_pixel_transfer(display);
                self.hblank_trigger = 20 + cycles;
            }

            // ===== Start of H-Blank ========================================
            _ if line < SCREEN_HEIGHT as u8 && self.cycle_in_line == self.hblank_trigger => {
                self.registers.set_mode(Mode::HBlank);

                // Trigger H-Blank interrupt if enabled.
                if self.regs().hblank_interrupt() {
                    interrupt_controller.request_interrupt(Interrupt::LcdStat);
                }
            }

            // ===== Start of V-Blank ========================================
            0 if line == SCREEN_HEIGHT as u8 => {
                self.registers.set_mode(Mode::VBlank);

                // The V-Blank interrupt is always triggered now
                interrupt_controller.request_interrupt(Interrupt::Vblank);

                // If the corresponding bit is set, we also trigger an LCD stat
                // interrupt.
                if self.regs().vblank_interrupt() {
                    interrupt_controller.request_interrupt(Interrupt::LcdStat);
                }
            }

            // During one mode, meaning we don't have to do anything. We just
            // need to act if a mode is starting.
            _ => {}
        }


        // Update cycles and line
        self.cycle_in_line += 1;
        if self.cycle_in_line == CYCLES_PER_LINE {
            // Bump the line and reset a bunch of values.
            self.registers.current_line += 1;
            self.cycle_in_line = 0;

            // Reset line if we reached the last one.
            if self.regs().current_line == NUM_LINES {
                self.registers.current_line = Byte::new(0);
            }
        }
    }

    /// Performs the OAM search.
    ///
    /// Looks through all 40 sprites in the OAM and extracts the first (up to)
    /// 10 that are drawn on the current line. These are stored in the
    /// `sprites_on_line` array. If there are fewer than 10 sprites on the
    /// current line, the remaining entries are `Sprite::invisible`.
    fn do_oam_search(&mut self) {
        let mut next_idx = 0;

        for sprite in self.oam.as_slice().chunks(4) {
            let sprite = Sprite {
                y: sprite[0],
                x: sprite[1],
                tile_idx: sprite[2],
                flags: sprite[3],
            };

            let line = self.regs().current_line + 16;
            if sprite.x != 0 && line >= sprite.y && line < sprite.y + self.regs().sprite_height() {
                self.sprites_on_line[next_idx] = sprite;
                next_idx += 1;

                // If we already found 10 sprites, we just stop OAM search. Any
                // other sprites are not drawn.
                if next_idx == 10 {
                    break;
                }
            }
        }

        // Fill the remaining entries with invisble sprites.
        for idx in next_idx..10 {
            self.sprites_on_line[idx] = Sprite::invisible();
        }

        // We sort them here to make drawing them easier. It has to be stable
        // sort to retain the original order of sprites with the same x
        // coordinate. We also have to sort them backwards so that sprites that
        // are more left are drawn on top of others.
        self.sprites_on_line.sort_by(|sa, sb| sa.x.cmp(&sb.x).reverse());
    }

    /// Performs the whole pixel transfer step at once.
    ///
    /// Usually, four roughly four pixels are pushed per 1MHz cycle and a bunch
    /// of internal stuff happens, but for the sake of simplicity, we do not
    /// model this here. This makes the emulator less precise and means that
    /// graphical effects based on changing some PPU registers during a line
    /// won't work.
    ///
    /// Returns the number of 1MHz cycles this phase took. This varies
    /// depending on the `scroll_x % 8`, on the window position and on the
    /// number of sprites. This number is only an approximation as apparently
    /// no one exactly knows how to determine the number of cycles. It's
    /// between 43 and 72 cycles.
    fn do_pixel_transfer(&self, display: &mut impl Display) -> u8 {
        // ===== Preparations ================================================

        /// Helper to fetch background and window tiles.
        struct Fetcher<'a> {
            // Reference to the whole PPU.
            ppu: &'a Ppu,

            /// The address in the VRAM of the current line of tiles in the
            /// tile map. For example, if the background is not scrolled (i.e.
            /// at 0, 0), this is either 0x1800 or 0x1C00. The address is
            /// relative to the VRAM memory block which is mapped to 0x8000.
            map_addr: Word,

            /// The x coordinate in the 32*32 tile map. `map_addr + map_x` is
            /// the address to the current tile.
            map_x: u8,

            /// The offset to the required line in the 16 byte tile bitmaps.
            bitmap_offset: u8,
        }

        impl<'a> Fetcher<'a> {
            /// Creates a fetcher that is not properly initialized yet and
            /// cannot be used to fetch tiles. Call `prime` before fetching any
            /// tiles.
            fn unprimed(ppu: &'a Ppu) -> Self {
                Self {
                    ppu,
                    map_addr: Word::zero(),
                    map_x: 0,
                    bitmap_offset: 0,
                }
            }

            /// Prime the prefetcher to start fetching from the map at address
            /// `map_base`, with the `x` and `y` pixel coordinates.
            fn prime(&mut self, map_base: Word, x: u8, y: u8) {
                self.map_x = x / 8;

                // Each line in the bitmap is stored using 2 bytes, so we have
                // an offset of 2 per line in the bitmap.
                self.bitmap_offset = (y % 8) * 2;

                self.map_addr = map_base + MAP_SIZE as u16 * (y / 8) as u16;
            }

            /// Advances to the next tile (in the x dimension, "right").
            fn advance_one_tile(&mut self) {
                self.map_x = (self.map_x + 1) % MAP_SIZE;
            }

            /// Fetches the current line of the current tile.
            fn fetch_tile_line(&self) -> [u8; 8] {
                // Lookup the tile index of the current tile in the tile map.
                let tile_idx = self.ppu.vram[self.map_addr + self.map_x];

                // We calculate the start address of the tile we want to load from.
                // This depends on the addressing mode used for the background/window
                // tiles.
                let tile_start = self.ppu.regs().bg_window_tile_data_address().index(tile_idx);

                // We only need to load one line (two bytes), so we need to
                // calculate that offset.
                let line_offset = tile_start + self.bitmap_offset;

                // Load the two bytes encoding the 8 pixels.
                double_byte_to_pixels(
                    self.ppu.vram[line_offset],
                    self.ppu.vram[line_offset + 1u8],
                )
            }
        }

        #[inline(always)]
        fn double_byte_to_pixels(lo: Byte, hi: Byte) -> [u8; 8] {
            let lo = lo.get();
            let hi = hi.get();

            [
                ((hi >> 6) & 0b10) | ((lo >> 7) & 0b1),
                ((hi >> 5) & 0b10) | ((lo >> 6) & 0b1),
                ((hi >> 4) & 0b10) | ((lo >> 5) & 0b1),
                ((hi >> 3) & 0b10) | ((lo >> 4) & 0b1),
                ((hi >> 2) & 0b10) | ((lo >> 3) & 0b1),
                ((hi >> 1) & 0b10) | ((lo >> 2) & 0b1),
                ((hi >> 0) & 0b10) | ((lo >> 1) & 0b1),
                ((hi << 1) & 0b10) | ((lo >> 0) & 0b1),
            ]
        }

        /// Converts the color number to a real color depending on the given
        /// palette.
        #[inline(always)]
        fn pattern_to_color(pattern: u8, palette: Byte) -> PixelColor {
            // The palette contains four color values. Bit0 and bit1 define the
            // color for the color number 0, bit2 and bit3 for color number 1
            // and so on.
            let color = (palette.get() >> (pattern * 2)) & 0b11;
            PixelColor::from_greyscale(color)
        }


        // ===== Draw ========================================================
        let mut line = [PixelColor::from_greyscale(0); SCREEN_WIDTH];
        let mut background_zero = [true; SCREEN_WIDTH]; // TODO: maybe use bit array


        // ----- Draw the background and window ------------------------------
        let window_visible = self.regs().is_window_enabled()
            && self.regs().scroll_win_y <= self.regs().current_line;
        let win_scroll_x = self.regs().scroll_win_x.get();

        // Create and prime the prefetcher to fetch background tiles
        let mut fetcher = Fetcher::unprimed(self);
        fetcher.prime(
            self.regs().bg_tile_map_address().start(),
            self.regs().scroll_bg_x.get(),
            (self.regs().scroll_bg_y + self.regs().current_line).get()
        );


        let mut tile_line = [0; 8]; // This value will never be read
        let mut needs_update = true;
        let mut pixel_in_line = (self.regs().scroll_bg_x.get() as usize) % 8;

        // For each pixel in this line...
        for col in 0..SCREEN_WIDTH {
            // Check if the window starts here
            if window_visible && win_scroll_x.saturating_sub(7) == col as u8 {
                // Reset the fetcher to now fetch from window tiles.
                pixel_in_line = 7u8.saturating_sub(win_scroll_x) as usize;
                fetcher.prime(
                    self.regs().window_tile_map_address().start(),
                    0,
                    (self.regs().current_line - self.regs().scroll_win_y).get(),
                );
                needs_update = true;
            }

            // If necessary, get new tile.
            if needs_update {
                tile_line = fetcher.fetch_tile_line();
                needs_update = false;
            }

            // Transfer pixel from tile to LCD
            background_zero[col] = tile_line[pixel_in_line] == 0;
            line[col] = pattern_to_color(tile_line[pixel_in_line], self.regs().background_palette);

            // Advance
            pixel_in_line = (pixel_in_line + 1) % 8;
            if pixel_in_line == 0 {
                fetcher.advance_one_tile();
                needs_update = true;
            }
        }

        // ----- Draw sprites ------------------------------------------------
        let sprite_height = self.regs().sprite_height();
        for sprite in &self.sprites_on_line {
            let x = sprite.x.get();
            let y = sprite.y.get();

            // We need to load the correct line of the correct tile bitmap. For
            // 8x16 sprites, there are two tiles involved. We first obtain the
            // address to the start of the tile (or the first tile, in the 8x16
            // case).
            let tile_id = if sprite_height == 8 {
                sprite.tile_idx.get()
            } else {
                sprite.tile_idx.get() & 0xFE
            };
            let tile_start = Word::new(tile_id as u16 * 16);

            // Next we find out which line of the sprite we need to draw. If
            // the y coordinate is 16, the upper edge of the sprite is exactly
            // at the top screen border (for both sprite sizes). So we have to
            // substract 16. We also need to adjust the line if the sprite is
            // flipped. Luckily it's fairly easy and even works for the 8x16
            // case.
            let mut line_in_sprite = self.regs().current_line.get() + 16 - y;
            if sprite.is_y_flipped() {
                line_in_sprite = (sprite_height - 1) - line_in_sprite;
            }

            // We offset the base address with the line of the sprite (times 2,
            // because we need two bytes per line of sprite data).
            let line_addr = tile_start + 2 * line_in_sprite as u16;
            let pixels = double_byte_to_pixels(self.vram[line_addr], self.vram[line_addr + 1u8]);


            // Here we need to figure out which of the 8 tile pixels we just
            // loaded are actually drawn. Usually all are drawn, but sprites
            // can be clipped on the left or right side of the screen.
            let (start, end) = match x {
                // Clipped left
                0..8 => (SPRITE_WIDTH - x, SPRITE_WIDTH),
                // Fully visible
                8..161 => (0, SPRITE_WIDTH),
                // Clipped right
                161..169 => (0, SPRITE_WIDTH + SCREEN_WIDTH as u8 - x),
                // Offscreen
                _ => continue,
            };

            // Just obtain the palette for this sprite.
            let palette = match sprite.palette0() {
                true => self.regs().sprite_palette_0,
                false => self.regs().sprite_palette_1,
            };

            // For all relevant pixels of the tile line, we will draw that
            // pixel into the buffer.
            for mut col_of_sprite in start..end {
                // Determine the screen x coordinate.
                let screen_col = x as usize + col_of_sprite as usize - 8;

                // Get the pattern from the sprite data (considering x flip).
                if sprite.is_x_flipped() {
                    col_of_sprite = 7 - col_of_sprite;
                }
                let pattern = pixels[col_of_sprite as usize];

                // If the pattern is 0, the pixel is translucent and is not
                // drawn.
                if pattern != 0 && (sprite.is_always_at_top() || background_zero[screen_col]) {
                    let color = pattern_to_color(pattern, palette);
                    line[screen_col] = color;
                }
            }
        }


        // ===== Send the line to the actual display =========================
        display.set_line(self.regs().current_line.get(), &line);

        // TODO: make more precise
        43
    }
}

/// Specifies which mode the PPU is in.
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
/// in length for different lines. This is due to window and sprites
/// interrupting the normal process of fetching data.
///
/// Duration of some things:
/// - One line: 20 + 43 + 51 = 114 cycles
/// - V-Blank: 10 * one line = 1140 cycles
/// - One frame: one line * 154 = 17_556 cycles
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Also called "Mode 2": PPU determines which sprites are visible on the
    /// current line.
    OamSearch = 2,

    /// Also called "Mode 3": pixels are transferred to the LCD screen.
    PixelTransfer = 3,

    /// Also called "Mode 0": time after pixel transfer when the PPU is waiting
    /// to start a new line.
    HBlank = 0,

    /// Also called "Mode 1": time after the last line has been drawn and
    /// before the next frame begins.
    VBlank = 1,
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Mode::OamSearch => "OAM search",
            Mode::PixelTransfer => "pixel transfer",
            Mode::HBlank => "H-Blank",
            Mode::VBlank => "V-Blank",
        }.fmt(f)
    }
}

/// Describes a sprite. The OAM stores exactly this information for up to 40
/// sprites.
#[derive(Copy, Clone, Debug)]
struct Sprite {
    y: Byte,
    x: Byte,
    tile_idx: Byte,
    flags: Byte,
}

impl Sprite {
    /// Returns an instance that has an x value of 255, making it invisble. All
    /// other fields are 0.
    fn invisible() -> Self {
        Self {
            y: Byte::zero(),
            x: Byte::new(255),
            tile_idx: Byte::zero(),
            flags: Byte::zero(),
        }
    }

    fn is_y_flipped(&self) -> bool {
        (self.flags.get() & 0b0100_0000) != 0
    }

    fn is_x_flipped(&self) -> bool {
        (self.flags.get() & 0b0010_0000) != 0
    }

    fn palette0(&self) -> bool {
        (self.flags.get() & 0b0001_0000) == 0
    }

    fn is_always_at_top(&self) -> bool {
        (self.flags.get() & 0b1000_0000) == 0
    }
}
