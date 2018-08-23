//! Contains code to actually execute instructions.

use super::Machine;
use crate::{
    Disruption,
    primitives::{Byte, Word},
    log::*,
    instr::{INSTRUCTIONS, PREFIXED_INSTRUCTIONS},
};


impl Machine {
    /// Executes one (the next) operation.
    pub(crate) fn step(&mut self) -> Result<(), Disruption> {
        // Variable initializsation (before macros, so they can be used there)
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

        // ============================
        // ========== MACROS ==========
        // ============================

        /// This is a convenience macro for all single line instructions. This basically adds
        /// the false return value.
        macro_rules! no_branch {
            ($x:expr) => {{
                $x;

                false
            }};
        }

        /// This is a template macro for all DEC instructions. Which can be used by passing
        /// the register in which should be decremented.
        macro_rules! dec {
            ($x:expr) => {{
                let (_, half_carry) = $x.sub_with_carries(Byte::new(1));
                let zero = $x == 0;
                set_flags!(self.cpu.f => zero 1 half_carry -);

                false
            }}
        }

        /// This is a template macro for all INC instructions. Which can be used by passing
        /// the register in which should be incremented.
        macro_rules! inc {
            ($x:expr) => {{
                let (_, half_carry) = $x.add_with_carries(Byte::new(1));
                let zero = $x == 0;
                set_flags!(self.cpu.f => zero 0 half_carry -);

                false
            }}
        }

        /// This is a template macro for all LD r, d8 instructions (where `r` can be one of:
        /// B, C, A, E, L, D, H). Which can be used by passing the register
        /// to which `arg_byte` should be loaded (e.g.: `ld_d8!(self.cpu.a);`).
        macro_rules! ld_d8 {
            ($x:expr) => {{
                $x = arg_byte;

                false
            }}
        }

        /// This is a template macro for all LD r, s instructions (where `r` and `s` can be one of:
        /// B, C, A, E, L, D, H). Which can be used by passing the registers in
        /// (e.g.: `ld!(self.cpu.a, self.cpu.b);`).
        macro_rules! ld {
            ($lhs:expr, $rhs:expr) => {{
                $lhs = $rhs;

                false
            }}
        }

        // Normal method stuff starts here
        let action_taken = match op_code.get() {
            // ========== LD ==========
            opcode!("LD B, d8") => {
                self.cpu.b = arg_byte;

                false
            }
            opcode!("LD C, d8") => {
                self.cpu.c = arg_byte;

                false
            }
            opcode!("LD A, d8") => {
                self.cpu.a = arg_byte;

                false
            }

            opcode!("LD C, A") => {
                self.cpu.c = self.cpu.a;

                false
            }
            opcode!("LD A, E") => {
                self.cpu.a = self.cpu.e;

                false
            }

            opcode!("LD A, (DE)") => {
                let val = self.load_byte(self.cpu.de());
                self.cpu.a = val;

                false
            }

            opcode!("LD HL, d16") => {
                self.cpu.set_hl(arg_word);

                false
            }
            opcode!("LD DE, d16") => {
                self.cpu.set_de(arg_word);

                false
            }
            opcode!("LD SP, d16") => {
                self.cpu.sp = arg_word;

                false
            }

            opcode!("LD (C), A") => {
                let dst = Word::new(0xFF00) + self.cpu.c;
                self.store_byte(dst, self.cpu.a);

                false
            }
            opcode!("LD (a16), A") => {
                self.store_byte(arg_word, self.cpu.a);

                false
            }
            opcode!("LDH (a8), A") => {
                let dst = Word::new(0xFF00) + arg_byte;
                self.store_byte(dst, self.cpu.a);

                false
            }
            opcode!("LD (HL), A") => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);

                false
            }
            opcode!("LD (HL+), A") => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);
                self.cpu.set_hl(dst + 1u16);

                false
            }
            opcode!("LD (HL-), A") => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);
                self.cpu.set_hl(dst - 1);

                false
            }

            // ========== DEC ==========
            opcode!("DEC B") => dec!(self.cpu.b),
            opcode!("DEC A") => dec!(self.cpu.a),

            // ========== INC ==========
            opcode!("INC DE") => {
                self.cpu.set_de(self.cpu.de() + 1u16);

                false
            }
            opcode!("INC HL") => {
                self.cpu.set_hl(self.cpu.hl() + 1u16);

                false
            }

            // ========== SUB ==========
            opcode!("SUB L") => {
                self.cpu.a -= self.cpu.l;

                false
            }

            // ========== XOR ==========
            opcode!("XOR A") => {
                self.cpu.a ^= self.cpu.a;
                set_flags!(self.cpu.f => 1 0 0 0);

                false
            }

            // ========== CP ==========
            opcode!("CP d8") => {
                // Subtract the value in d8 from A and set flags accordingly, but don't store
                // the result.
                let mut copy = self.cpu.a;
                let (carry, half_carry) = copy.sub_with_carries(arg_byte);
                let zero = copy == Byte::zero();
                set_flags!(self.cpu.f => zero 1 half_carry carry);

                false
            }

            // ========== JR ==========
            opcode!("JR NZ, r8") => {
                if !self.cpu.zero() {
                    self.cpu.pc += arg_byte.get() as i8;

                    true
                } else {
                    false
                }
            }

            // ========== POP/PUSH ==========
            opcode!("POP BC") => {
                let val = self.load_word(self.cpu.sp);
                self.cpu.sp += 2u16;
                self.cpu.set_bc(val);

                false
            }
            opcode!("PUSH BC") => {
                self.cpu.sp -= 2u16;
                self.store_word(self.cpu.sp, self.cpu.bc());

                false
            }

            // ========== CALL/RET ==========
            opcode!("CALL a16") => {
                self.cpu.sp -= 2u16;
                self.store_word(self.cpu.sp, self.cpu.pc);
                self.cpu.pc = arg_word;

                false
            }
            opcode!("RET") => {
                let val = self.load_word(self.cpu.sp);
                self.cpu.pc = val;
                self.cpu.sp += 2u16;

                false
            }

            // ========== miscellaneous ==========
            opcode!("RLA") => {
                let carry = self.cpu.a.rotate_left_through_carry(self.cpu.carry());
                set_flags!(self.cpu.f => 0 0 0 carry);

                false
            }

            opcode!("PREFIX CB") => {
                let instr_start = self.cpu.pc + 1u16;
                let op_code = self.load_byte(instr_start);
                let instr = PREFIXED_INSTRUCTIONS[op_code];
                self.cpu.pc += instr.len as u16;

                match op_code.get() {
                    // ========== RL ==========
                    prefixed_opcode!("RL C") => {
                        let carry = self.cpu.c.rotate_left_through_carry(self.cpu.carry());
                        let zero = self.cpu.c == Byte::zero();
                        set_flags!(self.cpu.f => zero 0 0 carry);
                    }

                    // ========== BIT ==========
                    prefixed_opcode!("BIT 7, H") => {
                        let zero = (self.cpu.h.get() & 0b1000_0000) == 0;
                        set_flags!(self.cpu.f => zero 0 1 -);
                    }

                    _ => {
                        debug!(
                            "Template:\n\
                            prefixed_opcode!(\"{}\") => {{\
                            \n\
                            \n\
                            \n\
                            false\n\
                            }}",
                            instr.mnemonic,
                        );
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

            _ => {
                debug!(
                    "Template:\n\
                    opcode!(\"{}\") => {{\
                    \n\
                    \n\
                    \n\
                    false\n\
                    }}",
                    instr.mnemonic,
                );
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
