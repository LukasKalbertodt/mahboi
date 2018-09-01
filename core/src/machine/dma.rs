//! Contains code to actually execute instructions.

use super::Machine;
use crate::{
    primitives::{Byte, Word},
};


impl Machine {
    /// Executes one DMA step if any DMA operations are currently ongoing.
    pub(crate) fn dma_step(&mut self) {
        // OAM DMA
        if let Some(src_addr) = self.ppu.oam_dma_status {
            if src_addr.into_bytes().1 < Byte::new(0xF1) {
                let dst_addr = Word::new(src_addr.get() % 0x100 + 0xFE00);
                let b = self.load_byte(src_addr);
                self.store_byte(dst_addr, b);
            }
        }
        if let Some(src_addr) = &mut self.ppu.oam_dma_status {
            *src_addr += 1u8;
        }
        if self.ppu.oam_dma_status.map(|addr| addr.into_bytes().1) == Some(Byte::new(0xF2)) {
            self.ppu.oam_dma_status = None;
        }
    }
}
