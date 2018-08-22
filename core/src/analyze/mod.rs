use std::{
    collections::BTreeMap,
};

// use slotmap::{Key, SlotMap};

use crate::{
    log::*,
    machine::{
        Machine,
    },
    primitives::{Memory, Word},
};
use self::{
    cfg::{Function, Block},
    instr::{InstrExt, RawInstr, decode_instr},
};

mod cfg;
mod util;
mod instr;


pub struct CodeMap {
    fns: BTreeMap<Word, Function>,

    /// For now, we only inspect the memory from 0 to 0x4000. This is read-only
    /// and basically guaranteed to not change. We capture this memory while
    /// the boot rom is still mounted. All of this will probably change later.
    mem: Memory,
}

impl CodeMap {
    pub fn new(machine: &Machine) -> Self {
        // Create the memory and fill it from the machine (only boot rom for now)
        let mut mem = Memory::zeroed(Word::new(0x100));
        for i in 0..0x100 {
            let addr = Word::new(i);
            mem[addr] = machine.load_byte(addr);
        }

        Self {
            fns: BTreeMap::new(),
            mem,
        }
    }

    pub fn add_entry_point(&mut self, entry_point: Word) {
        // We we already know about this entry point, do nothing
        if self.fns.contains_key(&entry_point) {
            return;
        }

        // Start analyzing the function.
        let mut blocks: Vec<Block> = vec![];
        let mut block_start_points = vec![entry_point];
        let mut counter = 3;

        while let Some(start) = block_start_points.pop() {
            trace!("Block start: {}", start);

            // Check if the start point is within an already existing block
            if let Some(idx) = blocks.iter_mut().position(|b| b.span.contains(start)) {
                let new_block = blocks[idx].split_off(start);
                blocks.push(new_block);
                continue;
            }

            // Start a new block
            let mut new_block = Block::new(start);

            let mut offset = start;
            loop {
                let instr = decode_instr([self.mem[offset], self.mem[offset + 1u8]])
                    .expect("tried to decode invalid opcode");

                let raw_instr = RawInstr::from_bytes(&self.mem[offset..offset + instr.len]);
                new_block.add_instr(raw_instr);


                if instr.jumps() {
                    // Add jump targets to the stack. If the jump is
                    // conditional, we add the the next instruction as start
                    // point.
                    if !instr.always_jumps() {
                        block_start_points.push(offset + instr.len);
                    }

                    // TODO: calculate jump destination
                    if let Some(target) = raw_instr.jump_target(offset) {
                        block_start_points.push(target);
                    }

                    break;
                }

                offset += instr.len;
            }

            blocks.push(new_block);

            counter -= 1;
            if counter == 0 {
                break;
            }
        }


        // print
        println!("{:#?}", blocks);
        println!("{:#?}", self.fns);
    }
}
