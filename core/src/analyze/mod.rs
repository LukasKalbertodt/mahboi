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
    util::Span,
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
        // Analyze this function and any other function called by this
        // function.
        let mut fn_entry_points = vec![entry_point];
        while let Some(entry_point) = fn_entry_points.pop() {
            let new_fns = self.add_function_at(entry_point);
            fn_entry_points.extend(new_fns);
        }

        println!("[codemap] {:#?}", self.fns);
    }

    /// Analyzes the function that starts at `start` and adds stores the
    /// result. Returns all references to other functions.
    fn add_function_at(&mut self, start: Word) -> &[Word] {
        trace!("[codemap] Adding function at {}", start);

        // We we already know about this entry point, so we do nothing and
        // return early. We return an empty list, because those methods were
        // already analyzed before.
        if self.fns.contains_key(&start) {
            return &self.fns[&start].foreign_calls;
        }

        // TODO: check if any other function we know contains the entry point.

        // List of the addresses of all
        let mut foreign_calls = vec![];

        // Collecting all blocks of this function.
        let mut blocks: Vec<Block> = vec![];

        // Stack of starting points of blocks. We have to visit all of them.
        let mut block_start_points = vec![start];

        let mut counter = 3;

        while let Some(start) = block_start_points.pop() {
            // Check if the start point is within an already existing block.
            if let Some(idx) = blocks.iter_mut().position(|b| b.span.contains(start)) {
                // So we jumped into an already existing block. So this block
                // is not a block in our definition and we thus have to split
                // it into two blocks.
                let new_block = blocks[idx].split_off(start);
                blocks.push(new_block);
                continue;
            }

            // Start a new block
            let mut new_block = Block::new(start);

            // Collect intstructions into this block until we hit a jumping
            // instruction.
            let mut offset = start;
            loop {
                // We load the instruction and add it to our block.
                let instr = decode_instr([self.mem[offset], self.mem[offset + 1u8]])
                    .expect("tried to decode invalid opcode");

                let raw_instr = RawInstr::from_bytes(&self.mem[offset..offset + instr.len]);
                new_block.add_instr(raw_instr);


                // If this is an instruction that jumps, our block will end.
                // But we also have to inspect the kind of jump and potentially
                // add new jump targets to the `block_start_points` stack.
                if instr.jumps() {
                    // If the jump is conditional, we add the address of the
                    // next instruction as start point.
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

        foreign_calls.sort();
        let new_fn = Function {
            span: Span::empty_at(start),
            blocks,
            foreign_calls,
        };
        self.fns.insert(start, new_fn);

        &self.fns[&start].foreign_calls
    }
}
