//! Contains code to actually execute instructions.

use super::{
    Machine,
    instr::{INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
};
use crate::{
    Disruption,
    primitives::{Byte, Word},
    log::*,
};


impl Machine {
    /// Executes one (the next) operation.
    pub(crate) fn step(&mut self) -> Result<(), Disruption> {
        let pc = self.cpu.pc;
        let op_code = self.load_byte(pc);
        let instr = match INSTRUCTIONS[op_code] {
            Some(v) => v,
            None => {
                terminate!(
                    "Unknown instruction {} in position: {} after: {} cycles",
                    op_code,
                    pc,
                    self.cycle_counter,
                );
            }
        };

        let action_taken = match op_code.get() {
            // ======== 0x0_ ========

            // LD C, d8
            0x0E => {
                let immediate = self.load_byte(pc + 1u16);
                self.cpu.c = immediate;

                false
            }

            // ======== 0x1_ ========

            // LD DE, d16
            0x11 => {
                let immediate = self.load_word(pc + 1u16);
                self.cpu.set_de(immediate);

                false
            }

            // LD A, (DE)
            0x1A => {
                let val = self.load_byte(self.cpu.de());
                self.cpu.a = val;

                false
            }

            // ======== 0x2_ ========

            // JR NZ, r8
            0x20 => {
                if !self.cpu.zero() {
                    let immediate = self.load_byte(pc + 1u16);
                    self.cpu.pc += immediate.get() as i8;

                    true
                } else {
                    false
                }
            }

            // LD HL, d16
            0x21 => {
                let immediate = self.load_word(pc + 1u16);
                let (lsb, msb) = immediate.into_bytes();
                self.cpu.h = msb;
                self.cpu.l = lsb;

                false
            }

            // ======== 0x3_ ========

            // LD SP, d16
            0x31 => {
                let immediate = self.load_word(pc + 1u16);
                self.cpu.sp = immediate;

                false
            }

            // LD (HL-), A
            0x32 => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);
                self.cpu.set_hl(dst - 1);

                false
            }

            // LD A, d8
            0x3E => {
                let immediate = self.load_byte(pc + 1u16);
                self.cpu.a = immediate;

                false
            }

            // ======== 0x7_ ========

            // LD (HL), A
            0x77 => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);

                false
            }

            // ======== 0xA_ ========

            // XOR A
            0xAF => {
                self.cpu.a ^= self.cpu.a;
                set_flags!(self.cpu.f => 1 0 0 0);

                false
            }

            // ======== 0xC_ ========

            // PUSH BC
            0xC5 => {
                self.cpu.sp -= 2u16;
                self.store_word(self.cpu.sp, self.cpu.bc());

                false
            }

            // PREFIX CB
            0xCB => {
                let pc = pc + 1u16;
                let op_code = self.load_byte(pc);
                let instr = match PREFIXED_INSTRUCTIONS[op_code] {
                    Some(v) => v,
                    None => {
                        terminate!(
                            "Unknown prefix instruction {} in position: {} after: {} cycles",
                            op_code,
                            pc,
                            self.cycle_counter,
                        );
                    }
                };

                match op_code.get() {
                    // ======== 0x1_ ========

                    // RL C
                    0x11 => {
                        let carry = self.cpu.c.rotate_left_through_carry(self.cpu.carry());
                        let zero = self.cpu.c == Byte::zero();
                        set_flags!(self.cpu.f => zero 0 0 carry);
                    }

                    // ======== 0xA_ ========

                    // BIT 7, H
                    0x7C => {
                        let zero = (self.cpu.h.get() & 0b1000_0000) == 0;
                        set_flags!(self.cpu.f => zero 0 1 -);
                    }

                    _ => {
                        terminate!(
                            "Unimplemented prefix instruction {:?} in position: {} after: \
                                {} cycles with opcode: {}",
                            instr,
                            pc,
                            self.cycle_counter,
                            op_code,
                        );
                    }
                }

                self.cpu.pc += instr.len as u16;
                self.cycle_counter += instr.cycles;

                false
            }

            // CALL a16
            0xCD => {
                let immediate = self.load_word(pc + 1u16);
                self.cpu.sp -= 2u16;
                self.store_word(self.cpu.sp, pc);
                self.cpu.pc = immediate;

                false
            }

            // ======== 0xE_ ========

            // LDH (a8), A
            0xE0 => {
                let immediate = self.load_byte(pc + 1u16);
                let dst = Word::new(0xFF00) + immediate;
                self.store_byte(dst, self.cpu.a);

                false
            }

            // LD (C), A
            0xE2 => {
                let dst = Word::new(0xFF00) + self.cpu.c;
                self.store_byte(dst, self.cpu.a);

                false
            }

            _ => {
                terminate!(
                    "Unimplemented instruction {:?} in position: {} after: \
                        {} cycles with opcode: {}",
                    instr,
                    pc,
                    self.cycle_counter,
                    op_code,
                );
            }
        };

        self.cpu.pc += instr.len as u16;
        self.cycle_counter += if action_taken {
            match instr.cycles_taken {
                Some(c) => c,
                None => {
                    terminate!(
                        "Action taken for non-branch instruction {:?} in position: {} after: \
                            {} cycles with opcode: {}",
                        instr,
                        pc,
                        self.cycle_counter,
                        op_code,
                    );
                }
            }
        } else {
            instr.cycles
        };

        Ok(())
    }
}
