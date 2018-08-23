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
        let instr_start = self.cpu.pc;
        let arg_byte = self.load_byte(instr_start + 1u16);
        let arg_word = self.load_word(instr_start + 1u16);
        let op_code = self.load_byte(instr_start);
        let instr = match INSTRUCTIONS[op_code] {
            Some(v) => v,
            None => {
                terminate!(
                    "Unknown instruction {} in position: {} after: {} cycles",
                    op_code,
                    instr_start,
                    self.cycle_counter,
                );
            }
        };
        self.cpu.pc += instr.len as u16;

        let action_taken = match op_code.get() {
            // ======== 0x0_ ========

            // DEC B
            0x05 => {
                let (_, half_carry) = self.cpu.b.sub_with_carries(Byte::new(1));
                let zero = self.cpu.b == 0;
                set_flags!(self.cpu.f => zero 1 half_carry -);

                false
            }

            // LD C, d8
            0x0E => {
                self.cpu.c = arg_byte;

                false
            }

            // ======== 0x1_ ========

            // LD DE, d16
            0x11 => {
                self.cpu.set_de(arg_word);

                false
            }

            // RLA
            0x17 => {
                let carry = self.cpu.a.rotate_left_through_carry(self.cpu.carry());
                set_flags!(self.cpu.f => 0 0 0 carry);

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
                    self.cpu.pc += arg_byte.get() as i8;

                    true
                } else {
                    false
                }
            }

            // LD HL, d16
            0x21 => {
                let (lsb, msb) = arg_word.into_bytes();
                self.cpu.h = msb;
                self.cpu.l = lsb;

                false
            }

            // LD (HL+), A
            0x22 => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);
                self.cpu.set_hl(dst + 1u16);

                false
            }

            // INC HL
            0x23 => {
                self.cpu.set_hl(self.cpu.hl() + 1u16);

                false
            }

            // ======== 0x3_ ========

            // LD SP, d16
            0x31 => {
                self.cpu.sp = arg_word;

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
                self.cpu.a = arg_byte;

                false
            }

            // ======== 0x7_ ========

            // LD (HL), A
            0x77 => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);

                false
            }

            // ======== 0x9_ ========

            // SUB L
            0x95 => {
                self.cpu.a -= self.cpu.l;

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

            // POP BC
            0xC1 => {
                let val = self.load_word(self.cpu.sp);
                self.cpu.sp += 2u16;
                self.cpu.set_bc(val);

                false
            }

            // PUSH BC
            0xC5 => {
                self.cpu.sp -= 2u16;
                self.store_word(self.cpu.sp, self.cpu.bc());

                false
            }

            // RET
            0xC9 => {
                let val = self.load_word(self.cpu.sp);
                self.cpu.pc = val;
                self.cpu.sp += 2u16;

                false
            }

            // PREFIX CB
            0xCB => {
                let instr_start = self.cpu.pc + 1u16;
                let op_code = self.load_byte(instr_start);
                let instr = PREFIXED_INSTRUCTIONS[op_code];
                self.cpu.pc += instr.len as u16;

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
                                {} cycles!",
                            instr,
                            instr_start,
                            self.cycle_counter,
                        );
                    }
                }

                self.cycle_counter += instr.cycles;

                false
            }

            // CALL a16
            0xCD => {
                self.cpu.sp -= 2u16;
                self.store_word(self.cpu.sp, self.cpu.pc);
                self.cpu.pc = arg_word;

                false
            }

            // ======== 0xE_ ========

            // LDH (a8), A
            0xE0 => {
                let dst = Word::new(0xFF00) + arg_byte;
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
                        {} cycles!",
                    instr,
                    instr_start,
                    self.cycle_counter,
                );
            }
        };

        self.cycle_counter += if action_taken {
            match instr.cycles_taken {
                Some(c) => c,
                None => {
                    terminate!(
                        "Action taken for non-branch instruction {:?} in position: {} after: \
                            {} cycles!",
                        instr,
                        instr_start,
                        self.cycle_counter,
                    );
                }
            }
        } else {
            instr.cycles
        };

        Ok(())
    }
}
