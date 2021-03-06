//! Contains code to actually execute instructions.

use super::Machine;
use crate::{
    primitives::{Byte, Word},
    log::*,
};


impl Machine {
    /// Executes one DMA step if any DMA operations are currently ongoing.
    pub(crate) fn dma_step(&mut self) {
        // TODO: pause DMA when LCD is disabled
        // OAM DMA
        if let Some(src_addr) = self.ppu.oam_dma_status {
            let lsb = src_addr.into_bytes().0;
            if lsb < Byte::new(0xA0) {
                let dst_addr = Word::new(0xFE00) + lsb;
                let b = self.load_byte_bypass_dma(src_addr);
                self.ppu.store_oam_byte(dst_addr, b);
            }

            // Advance the source address. If we reached 0xXXF1, we copied the
            // last byte and can stop.
            self.ppu.oam_dma_status = if lsb == Byte::new(0xA0) {
                trace!("DMA finished");
                None
            } else {
                Some(src_addr + 1u8)
            }
        }
    }
}
