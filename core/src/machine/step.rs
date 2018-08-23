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

        /// This is a template macro for all SUB instructions. Input should be a [`Byte`].
        macro_rules! sub {
            ($x:expr) => {{
                let (carry, half_carry) = self.cpu.a.sub_with_carries($x);
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 1 half_carry carry);

                false
            }}
        }

        /// This is a template macro for all SBC instructions. Input should be a [`Byte`].
        macro_rules! sbc {
            ($x:expr) => {{
                let val = $x - (self.cpu.carry() as u8);
                sub!(val)
            }}
        }

        /// This is a template macro for all ADD A, b instructions (where `b` should be a [`Byte`]).
        macro_rules! add {
            ($x:expr) => {{
                let (carry, half_carry) = self.cpu.a.add_with_carries($x);
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 0 half_carry carry);

                false
            }}
        }

        /// This is a template macro for all ADC A, b instructions (where `b` should be a [`Byte`]).
        macro_rules! adc {
            ($x:expr) => {{
                let val = $x + (self.cpu.carry() as u8);
                add!(val)
            }}
        }

        /// This is a template macro for all AND b instructions (where `b` should be a [`Byte`]).
        macro_rules! and {
            ($x:expr) => {{
                self.cpu.a &= $x;
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 0 1 0);

                false
            }}
        }

        /// This is a template macro for all XOR b instructions (where `b` should be a [`Byte`]).
        macro_rules! xor {
            ($x:expr) => {{
                self.cpu.a ^= $x;
                set_flags!(self.cpu.f => 1 0 0 0);

                false
            }}
        }

        /// This is a template macro for all OR b instructions (where `b` should be a [`Byte`]).
        macro_rules! or {
            ($x:expr) => {{
                self.cpu.a |= $x;
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 0);

                false
            }}
        }

        /// This is a template macro for all CP b instructions (where `b` should be a [`Byte`]).
        macro_rules! cp {
            ($x:expr) => {{
                // Subtract the value in $x from A and set flags accordingly, but don't store
                // the result.
                let mut copy = self.cpu.a;
                let (carry, half_carry) = copy.sub_with_carries($x);
                let zero = copy == Byte::zero();
                set_flags!(self.cpu.f => zero 1 half_carry carry);

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
            opcode!("LD B, d8") => ld_d8!(self.cpu.b),
            opcode!("LD C, d8") => ld_d8!(self.cpu.c),
            opcode!("LD A, d8") => ld_d8!(self.cpu.a),
            opcode!("LD E, d8") => ld_d8!(self.cpu.e),
            opcode!("LD L, d8") => ld_d8!(self.cpu.l),
            opcode!("LD D, d8") => ld_d8!(self.cpu.d),
            opcode!("LD H, d8") => ld_d8!(self.cpu.h),

            opcode!("LD B, B")      => ld!(self.cpu.b, self.cpu.b),
            opcode!("LD B, C")      => ld!(self.cpu.b, self.cpu.c),
            opcode!("LD B, D")      => ld!(self.cpu.b, self.cpu.d),
            opcode!("LD B, E")      => ld!(self.cpu.b, self.cpu.e),
            opcode!("LD B, H")      => ld!(self.cpu.b, self.cpu.h),
            opcode!("LD B, L")      => ld!(self.cpu.b, self.cpu.l),
            opcode!("LD B, (HL)")   => ld!(self.cpu.b, self.load_hl()),
            opcode!("LD B, A")      => ld!(self.cpu.b, self.cpu.a),

            opcode!("LD C, B")      => ld!(self.cpu.c, self.cpu.b),
            opcode!("LD C, C")      => ld!(self.cpu.c, self.cpu.c),
            opcode!("LD C, D")      => ld!(self.cpu.c, self.cpu.d),
            opcode!("LD C, E")      => ld!(self.cpu.c, self.cpu.e),
            opcode!("LD C, H")      => ld!(self.cpu.c, self.cpu.h),
            opcode!("LD C, L")      => ld!(self.cpu.c, self.cpu.l),
            opcode!("LD C, (HL)")   => ld!(self.cpu.c, self.load_hl()),
            opcode!("LD C, A")      => ld!(self.cpu.c, self.cpu.a),

            opcode!("LD D, B")      => ld!(self.cpu.d, self.cpu.b),
            opcode!("LD D, C")      => ld!(self.cpu.d, self.cpu.c),
            opcode!("LD D, D")      => ld!(self.cpu.d, self.cpu.d),
            opcode!("LD D, E")      => ld!(self.cpu.d, self.cpu.e),
            opcode!("LD D, H")      => ld!(self.cpu.d, self.cpu.h),
            opcode!("LD D, L")      => ld!(self.cpu.d, self.cpu.l),
            opcode!("LD D, (HL)")   => ld!(self.cpu.d, self.load_hl()),
            opcode!("LD D, A")      => ld!(self.cpu.d, self.cpu.a),

            opcode!("LD E, B")      => ld!(self.cpu.e, self.cpu.b),
            opcode!("LD E, C")      => ld!(self.cpu.e, self.cpu.c),
            opcode!("LD E, D")      => ld!(self.cpu.e, self.cpu.d),
            opcode!("LD E, E")      => ld!(self.cpu.e, self.cpu.e),
            opcode!("LD E, H")      => ld!(self.cpu.e, self.cpu.h),
            opcode!("LD E, L")      => ld!(self.cpu.e, self.cpu.l),
            opcode!("LD E, (HL)")   => ld!(self.cpu.e, self.load_hl()),
            opcode!("LD E, A")      => ld!(self.cpu.e, self.cpu.a),

            opcode!("LD H, B")      => ld!(self.cpu.h, self.cpu.b),
            opcode!("LD H, C")      => ld!(self.cpu.h, self.cpu.c),
            opcode!("LD H, D")      => ld!(self.cpu.h, self.cpu.d),
            opcode!("LD H, E")      => ld!(self.cpu.h, self.cpu.e),
            opcode!("LD H, H")      => ld!(self.cpu.h, self.cpu.h),
            opcode!("LD H, L")      => ld!(self.cpu.h, self.cpu.l),
            opcode!("LD H, (HL)")   => ld!(self.cpu.h, self.load_hl()),
            opcode!("LD H, A")      => ld!(self.cpu.h, self.cpu.a),

            opcode!("LD L, B")      => ld!(self.cpu.l, self.cpu.b),
            opcode!("LD L, C")      => ld!(self.cpu.l, self.cpu.c),
            opcode!("LD L, D")      => ld!(self.cpu.l, self.cpu.d),
            opcode!("LD L, E")      => ld!(self.cpu.l, self.cpu.e),
            opcode!("LD L, H")      => ld!(self.cpu.l, self.cpu.h),
            opcode!("LD L, L")      => ld!(self.cpu.l, self.cpu.l),
            opcode!("LD L, (HL)")   => ld!(self.cpu.l, self.load_hl()),
            opcode!("LD L, A")      => ld!(self.cpu.l, self.cpu.a),

            opcode!("LD A, B")      => ld!(self.cpu.a, self.cpu.b),
            opcode!("LD A, C")      => ld!(self.cpu.a, self.cpu.c),
            opcode!("LD A, D")      => ld!(self.cpu.a, self.cpu.d),
            opcode!("LD A, E")      => ld!(self.cpu.a, self.cpu.e),
            opcode!("LD A, H")      => ld!(self.cpu.a, self.cpu.h),
            opcode!("LD A, L")      => ld!(self.cpu.a, self.cpu.l),
            opcode!("LD A, (HL)")   => ld!(self.cpu.a, self.load_hl()),
            opcode!("LD A, A")      => ld!(self.cpu.a, self.cpu.a),

            opcode!("LD (HL), B") => no_branch!(self.store_hl(self.cpu.b)),
            opcode!("LD (HL), C") => no_branch!(self.store_hl(self.cpu.c)),
            opcode!("LD (HL), D") => no_branch!(self.store_hl(self.cpu.d)),
            opcode!("LD (HL), E") => no_branch!(self.store_hl(self.cpu.e)),
            opcode!("LD (HL), H") => no_branch!(self.store_hl(self.cpu.h)),
            opcode!("LD (HL), L") => no_branch!(self.store_hl(self.cpu.l)),
            opcode!("LD (HL), A") => no_branch!(self.store_hl(self.cpu.a)),

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
            opcode!("LDH A, (a8)") => {
                let src = Word::new(0xFF00) + arg_byte;
                self.cpu.a = self.load_byte(src);

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
            opcode!("DEC D") => dec!(self.cpu.d),
            opcode!("DEC H") => dec!(self.cpu.h),
            opcode!("DEC C") => dec!(self.cpu.c),
            opcode!("DEC E") => dec!(self.cpu.e),
            opcode!("DEC L") => dec!(self.cpu.l),
            opcode!("DEC A") => dec!(self.cpu.a),

            opcode!("DEC BC") => no_branch!(self.cpu.set_bc(self.cpu.bc() - 1u16)),
            opcode!("DEC DE") => no_branch!(self.cpu.set_de(self.cpu.de() - 1u16)),
            opcode!("DEC HL") => no_branch!(self.cpu.set_hl(self.cpu.hl() - 1u16)),
            opcode!("DEC SP") => no_branch!(self.cpu.sp -= 1u16),

            // ========== INC ==========
            opcode!("INC B") => inc!(self.cpu.b),
            opcode!("INC D") => inc!(self.cpu.d),
            opcode!("INC H") => inc!(self.cpu.h),
            opcode!("INC C") => inc!(self.cpu.c),
            opcode!("INC E") => inc!(self.cpu.e),
            opcode!("INC L") => inc!(self.cpu.l),
            opcode!("INC A") => inc!(self.cpu.a),

            opcode!("INC BC") => no_branch!(self.cpu.set_bc(self.cpu.bc() + 1u16)),
            opcode!("INC DE") => no_branch!(self.cpu.set_de(self.cpu.de() + 1u16)),
            opcode!("INC HL") => no_branch!(self.cpu.set_hl(self.cpu.hl() + 1u16)),
            opcode!("INC SP") => no_branch!(self.cpu.sp += 1u16),

            // ========== ADD ==========
            opcode!("ADD A, B")     => add!(self.cpu.b),
            opcode!("ADD A, C")     => add!(self.cpu.c),
            opcode!("ADD A, D")     => add!(self.cpu.d),
            opcode!("ADD A, E")     => add!(self.cpu.e),
            opcode!("ADD A, H")     => add!(self.cpu.h),
            opcode!("ADD A, L")     => add!(self.cpu.l),
            opcode!("ADD A, (HL)")  => add!(self.load_hl()),
            opcode!("ADD A, A")     => add!(self.cpu.a),
            opcode!("ADD A, d8")    => add!(arg_byte),

            // ========== ADC ==========
            opcode!("ADC A, B")     => adc!(self.cpu.b),
            opcode!("ADC A, C")     => adc!(self.cpu.c),
            opcode!("ADC A, D")     => adc!(self.cpu.d),
            opcode!("ADC A, E")     => adc!(self.cpu.e),
            opcode!("ADC A, H")     => adc!(self.cpu.h),
            opcode!("ADC A, L")     => adc!(self.cpu.l),
            opcode!("ADC A, (HL)")  => adc!(self.load_hl()),
            opcode!("ADC A, A")     => adc!(self.cpu.a),
            opcode!("ADC A, d8")    => adc!(arg_byte),

            // ========== SUB ==========
            opcode!("SUB B")    => sub!(self.cpu.b),
            opcode!("SUB C")    => sub!(self.cpu.c),
            opcode!("SUB D")    => sub!(self.cpu.d),
            opcode!("SUB E")    => sub!(self.cpu.e),
            opcode!("SUB H")    => sub!(self.cpu.h),
            opcode!("SUB L")    => sub!(self.cpu.l),
            opcode!("SUB (HL)") => sub!(self.load_hl()),
            opcode!("SUB A")    => sub!(self.cpu.a),
            opcode!("SUB d8")   => sub!(arg_byte),

            // ========== SBC ==========
            opcode!("SBC A, B")    => sbc!(self.cpu.b),
            opcode!("SBC A, C")    => sbc!(self.cpu.c),
            opcode!("SBC A, D")    => sbc!(self.cpu.d),
            opcode!("SBC A, E")    => sbc!(self.cpu.e),
            opcode!("SBC A, H")    => sbc!(self.cpu.h),
            opcode!("SBC A, L")    => sbc!(self.cpu.l),
            opcode!("SBC A, (HL)") => sbc!(self.load_hl()),
            opcode!("SBC A, A")    => sbc!(self.cpu.a),
            opcode!("SBC A, d8")   => sbc!(arg_byte),

            // ========== AND ==========
            opcode!("AND B")    => and!(self.cpu.b),
            opcode!("AND C")    => and!(self.cpu.c),
            opcode!("AND D")    => and!(self.cpu.d),
            opcode!("AND E")    => and!(self.cpu.e),
            opcode!("AND H")    => and!(self.cpu.h),
            opcode!("AND L")    => and!(self.cpu.l),
            opcode!("AND (HL)") => and!(self.load_hl()),
            opcode!("AND A")    => and!(self.cpu.a),
            opcode!("AND d8")   => and!(arg_byte),

            // ========== XOR ==========
            opcode!("XOR B")    => xor!(self.cpu.b),
            opcode!("XOR C")    => xor!(self.cpu.c),
            opcode!("XOR D")    => xor!(self.cpu.d),
            opcode!("XOR E")    => xor!(self.cpu.e),
            opcode!("XOR H")    => xor!(self.cpu.h),
            opcode!("XOR L")    => xor!(self.cpu.l),
            opcode!("XOR (HL)") => xor!(self.load_hl()),
            opcode!("XOR A")    => xor!(self.cpu.a),
            opcode!("XOR d8")   => xor!(arg_byte),

            // ========== OR ==========
            opcode!("OR B")    => or!(self.cpu.b),
            opcode!("OR C")    => or!(self.cpu.c),
            opcode!("OR D")    => or!(self.cpu.d),
            opcode!("OR E")    => or!(self.cpu.e),
            opcode!("OR H")    => or!(self.cpu.h),
            opcode!("OR L")    => or!(self.cpu.l),
            opcode!("OR (HL)") => or!(self.load_hl()),
            opcode!("OR A")    => or!(self.cpu.a),
            opcode!("OR d8")   => or!(arg_byte),

            // ========== CP ==========
            opcode!("CP B")    => cp!(self.cpu.b),
            opcode!("CP C")    => cp!(self.cpu.c),
            opcode!("CP D")    => cp!(self.cpu.d),
            opcode!("CP E")    => cp!(self.cpu.e),
            opcode!("CP H")    => cp!(self.cpu.h),
            opcode!("CP L")    => cp!(self.cpu.l),
            opcode!("CP (HL)") => cp!(self.load_hl()),
            opcode!("CP A")    => cp!(self.cpu.a),
            opcode!("CP d8")   => cp!(arg_byte),

            // ========== JR ==========
            opcode!("JR r8") => {
                self.cpu.pc += arg_byte.get() as i8;

                false
            }
            opcode!("JR NZ, r8") => {
                if !self.cpu.zero() {
                    self.cpu.pc += arg_byte.get() as i8;

                    true
                } else {
                    false
                }
            }
            opcode!("JR Z, r8") => {
                if self.cpu.zero() {
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
