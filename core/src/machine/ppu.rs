//! Everything related to the pixel processing unit (PPU).

use std::{
    fmt,
    ops::Range,
};

use crate::{
    SCREEN_HEIGHT, SCREEN_WIDTH,
    env::Display,
    log::*,
    primitives::{Byte, Word, Memory, PixelPos, PixelColor},
};
use super::interrupt::{InterruptController, Interrupt};



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



    // ===== State of the pixel transfer operation ======
    // All of the following fields are state of the pixel transfer state
    // machine. Outside of pixel transfer, these fields' values are useless.

    fifo: PixelFifo,

    /// Stores whether or not an fetch operation has already been started. This
    /// boolean usually flips every cycle during pixel transfer.
    fetch_started: bool,

    /// This is the x position of the next tile to fetch. This is in tile space
    /// (0--31) and not pixel space (0--256)!
    fetch_tile_x: u8,

    /// This stores the offset of the line of interest in the current tile. A
    /// tile has 8 lines, each stored in two bytes. So this value is the line
    /// number in the tile times two.
    line_in_tile_offset: u8,

    /// This is the address to the first tile in the tile map of the current
    /// line (relative to the beginning of VRAM). This means that
    /// `fetch_map_line_offset + fetch_tile_x` is the address of the next tile
    /// index in the tile map we need to fetch. We store those separated
    /// because the x value needs to be able to overflow and wrap around
    /// independently.
    fetch_map_line_offset: Word,

    /// Sometimes, we need to throw away some pixels from the pixel FIFO. We
    /// calculate the number of pixels we need to throw away at the beginning
    /// of each line. And store it. During the line this is always decreased to
    /// 0.
    num_throw_away_pixels: u8,

    /// ...
    current_column: u8,

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

            fifo: PixelFifo::new(),
            fetch_started: false,
            fetch_tile_x: 0,
            line_in_tile_offset: 0,
            fetch_map_line_offset: Word::zero(),
            num_throw_away_pixels: 0,

            current_column: 0,
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
                // TODO: Bit 2 has to be generated somewhere
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
                        self.current_column = 0;
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
                let src_addr = Word::new((byte.get() as u16) * 0x100 - 1);
                self.oam_dma_status = Some(src_addr);
            },
            0xFF47 => self.registers.background_palette = byte,
            0xFF48 => self.registers.sprite_palette_0 = byte,
            0xFF49 => self.registers.sprite_palette_1 = byte,
            0xFF4A => self.registers.scroll_win_y = byte,
            0xFF4B => self.registers.scroll_win_x = byte,
            _ => panic!("called `Ppu::store_io_byte` with invalid address"),
        }
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

        // Check if we're currently in V-Blank or not.
        if self.regs().current_line.get() >= SCREEN_HEIGHT as u8 {
            // ===== V-Blank =====
            if self.regs().current_line == SCREEN_HEIGHT as u8 && self.cycle_in_line == 0 {
                self.registers.set_mode(Mode::VBlank);
                interrupt_controller.request_interrupt(Interrupt::Vblank);

                if self.regs().vblank_interrupt() {
                    interrupt_controller.request_interrupt(Interrupt::Vblank);
                }
            }
        } else {
            // ===== Not in V-Blank =====
            match (self.cycle_in_line, self.current_column) {
                (0..20, 0) => {
                    if self.cycle_in_line == 0 {
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
                    }
                    // TODO: OAM Search
                }
                (20..114, col) if col < SCREEN_WIDTH as u8 => {
                    if self.cycle_in_line == 20 {
                        self.registers.set_mode(Mode::PixelTransfer);
                        self.prepare_pixel_transfer();
                    }
                    self.pixel_transfer_step(display, interrupt_controller);
                }
                (43..114, col)
                    if self.regs().mode() == Mode::HBlank && col == SCREEN_WIDTH as u8
                => {
                    // We don't have to do anything in H-Blank. This match arm
                    // just exists to make sure we never reach an invalid state
                    // (with the next arm)
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
            self.registers.current_line += 1;
            self.cycle_in_line = 0;
            self.current_column = 0;
            self.fifo.clear();

            // Reset line if we reached the last one.
            if self.regs().current_line == 154 {
                self.registers.current_line = Byte::new(0);
            }
        }
    }

    fn prepare_pixel_transfer(&mut self) {
        // We remove all pixels from the FIFO that may still be inside. This
        // might already happen at an earlier stage, but it's not observable to
        // the user, so we can do this right before starting the pixel
        // transfer.
        self.fifo.clear();
        self.fetch_started = false;

        // We calculate the x coordinate of the tile we need to fetch first.
        // This can simply be increased by 1 after each fetch.
        self.fetch_tile_x = self.regs().scroll_bg_x.get() / 8;

        // Since we always fetch full 8 pixel tiles but can scroll pixel
        // perfect, we might have to discard a few pixels we load into the
        // FIFO. We can calculate the number of pixels we have to throw away
        // here.
        self.num_throw_away_pixels = self.regs().scroll_bg_x.get() % 8;

        // Calculate the Y position and the offset within one tile.
        let pos_y = (self.regs().scroll_bg_y + self.regs().current_line).get();
        self.line_in_tile_offset = (pos_y % 8) * 2;

        // Here we precompute the address offset to the first tile of the
        // current line in the tile map. This means that we can easily compute
        // the address of the real tile later as `fetch_map_line_offset +
        // fetch_tile_x`.
        let tile_y = pos_y / 8;
        let map_offset = self.regs().bg_tile_map_address().start();
        self.fetch_map_line_offset = map_offset + 32 * tile_y as u16;
    }

    /// Performs one step of the pixel transfer phase. This involves fetching
    /// new tile data and emitting the pixels.
    fn pixel_transfer_step(
        &mut self,
        display: &mut impl Display,
        interrupt_controller: &mut InterruptController,
    ) {
        // Push out up to four new pixels if we have enough data in the FIFO.
        let mut pixel_pushed = 0;
        while self.fifo.len() > 8 && pixel_pushed < 4 {
            // Check if the next pixel should be discarded or not.
            if self.num_throw_away_pixels == 0 {
                self.push_pixel(display);

                // We just bumped our `current_column`. Now we check if we
                // reached the start of the window.
                let window_trigger = self.regs().is_window_enabled()
                    && self.regs().scroll_win_x.get() >= 7
                    && self.current_column == self.regs().scroll_win_x.get() - 7;
                if window_trigger {
                    // We need to basically reset the whole state.
                    self.fifo.clear();
                    self.fetch_started = false;

                    // The following is nearly the same as in
                    // `prepare_pixel_transfer`.
                    self.fetch_tile_x = 0;

                    // Calculate the Y position and the offset within one tile.
                    let pos_y = (self.regs().scroll_win_y + self.regs().current_line).get();
                    self.line_in_tile_offset = (pos_y % 8) * 2;

                    // Here we precompute the address offset to the first tile
                    // of the current line in the tile map. This means that we
                    // can easily compute the address of the real tile later as
                    // `fetch_map_line_offset + fetch_tile_x`.
                    let tile_y = pos_y / 8;
                    let map_offset = self.regs().window_tile_map_address().start();
                    self.fetch_map_line_offset = map_offset + 32 * tile_y as u16;
                }
            } else {
                let _ = self.fifo.emit();
                self.num_throw_away_pixels -= 1;
            }

            pixel_pushed += 1;

            // We are at the end of the line, stop everything and go to
            // H-Blank.
            if self.current_column == SCREEN_WIDTH as u8 {
                self.registers.set_mode(Mode::HBlank);

                // Trigger H-Blank interrupt if enabled.
                if self.regs().hblank_interrupt() {
                    interrupt_controller.request_interrupt(Interrupt::LcdStat);
                }

                return;
            }
        }

        // Fetch new data. We need two steps to perform one fetch. We just
        // don't do anything the first time `step()` is called, except for
        // setting `fetch_start = true`. The actual work is done in the second
        // step.
        if !self.fetch_started {
            self.fetch_started = true;
        } else {
            // We lookup the index of our tile in the tile map here.
            let tile_idx = self.vram[self.fetch_map_line_offset + self.fetch_tile_x];

            // We calculate the start address of the tile we want to load from.
            // This depends on the addressing mode used for the
            // background/window tiles.
            let tile_start = self.regs().bg_window_tile_data_address().index(tile_idx);

            // We only need to load one line (two bytes), so we need to
            // calculate that offset.
            let line_offset = tile_start + self.line_in_tile_offset;

            // Load the two bytes and add all pixels to the FIFO.
            let lo = self.vram[line_offset].get();
            let hi = self.vram[line_offset + 1u8].get();
            self.fifo.add_data(hi, lo, PixelSource::Background);

            // if self.current_line == 0 {
            //     debug!(
            //         "[ppu] fetched 8 pixels. current_col: {} ,pos_x: {}, pos_y: {}, \
            //             scroll_bg_x: {}, scroll_bg_y: {}, tile_x: {}, tile_y: {}, bg_addr: {}, \
            //             tile_id: {}, tile_start: {}, line_offset: {}, new FIFO len: {}",
            //         self.current_column,
            //         pos_x,
            //         pos_y,
            //         self.scroll_bg_x,
            //         self.scroll_bg_y,
            //         tile_x,
            //         tile_y,
            //         background_addr,
            //         tile_id,
            //         tile_start,
            //         line_offset,
            //         self.fifo.len()
            //     );
            // }

            // Reset status flag and bump the x tile position
            self.fetch_started = false;
            self.fetch_tile_x += 1;
        }
    }

    /// Takes the first pixel from the pixel FIFO, calculate its real color (by
    /// palette lookup) and writes that color to the display.
    fn push_pixel(&mut self, display: &mut impl Display) {
        // Converts the color number to a real color depending on the given
        // palette.
        #[inline(always)]
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
            PixelSource::Background => self.regs().background_palette,
            PixelSource::Sprite0 => self.regs().sprite_palette_0,
            PixelSource::Sprite1 => self.regs().sprite_palette_1,
        };

        // Convert to real color
        let color = pattern_to_color(pattern, palette);

        // Write to display
        let pos = PixelPos::new(self.current_column, self.regs().current_line.get());
        display.set_pixel(pos, color);

        self.current_column += 1;
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
        self.colors_hi = 0;
        self.colors_lo = 0;
        self.sources = 0;
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

    #[test]
    fn fifo_clear() {
        // The same as in `fifo_simple`
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

        // Now we clear the FIFO and only fill it with zeroes.
        fifo.clear();
        assert_eq!(fifo.len(), 0);

        fifo.add_data(0, 0, PixelSource::Background);
        fifo.add_data(0, 0, PixelSource::Background);
        assert_eq!(fifo.len(), 16);

        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
        assert_eq!(fifo.emit(), (0b00, PixelSource::Background));
    }
}
