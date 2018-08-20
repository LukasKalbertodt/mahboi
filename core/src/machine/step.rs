use super::{
    Machine,
    instr::{INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
};
use crate::{
    Disruption,
    log::*,
};


impl Machine {
    /// Executes one (the next) operation.
    pub(crate) fn step(&mut self) -> Result<(), Disruption> {
        let pc = self.cpu.pc;
        let op_code = self.load_byte(pc);
        let instr = match INSTRUCTIONS[op_code.get() as usize] {
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

            // ======== 0xA_ ========

            // XOR A
            0xAF => {
                self.cpu.a ^= self.cpu.a;
                set_flags!(self.cpu.f => 1 0 0 0);

                false
            }

            // ======== 0xA_ ========

            // PREFIX CB
            0xCB => {
                let pc = pc + 1u16;
                let op_code = self.load_byte(pc);
                let instr = match PREFIXED_INSTRUCTIONS[op_code.get() as usize] {
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
                    // ======== 0xA_ ========

                    // BIT 7, H
                    0x7C => {
                        let zero = (self.cpu.h.get() & 0b1000_0000) == 0;
                        set_flags!(self.cpu.f => zero 0 1 -);
                    }

                    _ => {
                        terminate!(
                            "Unimplemented prefix instruction {:?} in position: {} after: \
                                {} cycles",
                            instr,
                            pc,
                            self.cycle_counter,
                        );
                    }
                }

                self.cpu.pc += instr.len as u16;
                self.cycle_counter += instr.cycles;

                false
            }

            _ => {
                terminate!(
                    "Unimplemented instruction {:?} in position: {} after: {} cycles",
                    instr,
                    pc,
                    self.cycle_counter,
                );
            }
        };

        self.cpu.pc += instr.len as u16;
        self.cycle_counter += if action_taken {
            match instr.cycles_taken {
                Some(c) => c,
                None => {
                    terminate!(
                        "Action taken for non-branch instruction {} in position: {} after: \
                            {} cycles",
                        op_code,
                        pc,
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
