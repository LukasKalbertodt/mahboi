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
    pub(crate) fn step(&mut self) -> Result<u8, Disruption> {
        // Check if an interrupt was requested
        if let Some(interrupt) = self.interrupt_controller.should_interrupt() {
            debug!("Interrupt triggered: {:?}", interrupt);
            return Ok(self.isr(interrupt) / 4);
        }

        // Check if we are in HALT mode
        if self.halt {
            // If no interrupt was triggered (otherwise we wouldn't have gotten here) but at least
            // one interrupt was requested -> exit HALT mode.
            if self.interrupt_controller.is_interrupt_requested() {
                self.halt = false;
            }

            // Executed 1 cycle doing nothing ＼(＾O＾)／
            return Ok(1);
        }

        // Variable initialization
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

        // TODO: Check if this position for enable_interrupts_next_step check is a good choice.
        // Why? According to [1] the IME is set in the cycle AFTER the EI instruction. It is
        // not clear when exactly this happens during the next cycle. The timing here is
        // important, because some instructions (like DI) access the IME. If this check is done
        // after the opcode match block the behavior of some opcodes would change!
        //
        // [1]: https://github.com/AntonioND/giibiiadvance/blob/master/docs/TCAGBD.pdf

        // Check if interrupts should be enabled during this cycle so they will be active in
        // the next cylce.
        if self.enable_interrupts_next_step {
            self.interrupt_controller.ime = true;
            self.enable_interrupts_next_step = false;
        }

        // Check if a branch was taken in the opcode. This needs to be set by opcodes which have
        // a `Some` in their `clocks_taken` field.
        let mut action_taken: Option<bool> = None;

        // ============================
        // ========== MACROS ==========
        // ============================

        /// This is a template macro for all DEC instructions. Which can be used by passing
        /// the register in which should be decremented.
        macro_rules! dec {
            ($x:expr) => {{
                let (_, half_carry) = $x.sub_with_carries(Byte::new(1));
                let zero = $x == 0;
                set_flags!(self.cpu.f => zero 1 half_carry -);
            }}
        }

        /// This is a template macro for all INC instructions. Which can be used by passing
        /// the register in which should be incremented.
        macro_rules! inc {
            ($x:expr) => {{
                let (_, half_carry) = $x.add_with_carries(Byte::new(1));
                let zero = $x == 0;
                set_flags!(self.cpu.f => zero 0 half_carry -);
            }}
        }

        /// This is a template macro for all SUB instructions. Input should be a [`Byte`].
        macro_rules! sub {
            ($x:expr) => {{
                let (carry, half_carry) = self.cpu.a.sub_with_carries($x);
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 1 half_carry carry);
            }}
        }

        /// This is a template macro for all SBC instructions. Input should be a [`Byte`].
        macro_rules! sbc {
            ($x:expr) => {{
                let val = $x - (self.cpu.carry() as u8);
                sub!(val);
            }}
        }

        /// This is a template macro for all ADD A, b instructions (where `b` should be a [`Byte`]).
        macro_rules! add {
            ($x:expr) => {{
                let (carry, half_carry) = self.cpu.a.add_with_carries($x);
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 0 half_carry carry);
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
            }}
        }

        /// This is a template macro for all XOR b instructions (where `b` should be a [`Byte`]).
        macro_rules! xor {
            ($x:expr) => {{
                self.cpu.a ^= $x;
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 0);
            }}
        }

        /// This is a template macro for all OR b instructions (where `b` should be a [`Byte`]).
        macro_rules! or {
            ($x:expr) => {{
                self.cpu.a |= $x;
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 0);
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
            }}
        }

        /// This is a template macro for all LD r, d8 instructions (where `r` can be one of:
        /// B, C, A, E, L, D, H). Which can be used by passing the register
        /// to which `arg_byte` should be loaded (e.g.: `ld_d8!(self.cpu.a);`).
        macro_rules! ld_d8 {
            ($x:expr) => {
                $x = arg_byte
            }
        }

        /// This is a template macro for all LD r, s instructions (where `r` and `s` can be one of:
        /// B, C, A, E, L, D, H). Which can be used by passing the registers in
        /// (e.g.: `ld!(self.cpu.a, self.cpu.b);`).
        macro_rules! ld {
            ($lhs:expr, $rhs:expr) => {
                $lhs = $rhs
            }
        }

        /// This is a template macro for all RLC b instructions (where `b` should be a [`Byte`]).
        macro_rules! rlc {
            ($x:expr) => {{
                let carry = $x.rotate_left();
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all RRC b instructions (where `b` should be a [`Byte`]).
        macro_rules! rrc {
            ($x:expr) => {{
                let carry = $x.rotate_right();
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all RL b instructions (where `b` should be a [`Byte`]).
        macro_rules! rl {
            ($x:expr) => {{
                let carry = $x.rotate_left_through_carry(self.cpu.carry());
                let zero = self.cpu.c == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all RR b instructions (where `b` should be a [`Byte`]).
        macro_rules! rr {
            ($x:expr) => {{
                let carry = $x.rotate_right_through_carry(self.cpu.carry());
                let zero = self.cpu.c == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all SLA b instructions (where `b` should be a [`Byte`]).
        macro_rules! sla {
            ($x:expr) => {{
                let carry = $x.shift_left();
                let zero = self.cpu.c == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all SRL b instructions (where `b` should be a [`Byte`]).
        macro_rules! srl {
            ($x:expr) => {{
                let carry = $x.shift_right();
                let zero = self.cpu.c == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        // Execute the fetched instruction
        match op_code.get() {
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

            opcode!("LD (HL), B") => self.store_hl(self.cpu.b),
            opcode!("LD (HL), C") => self.store_hl(self.cpu.c),
            opcode!("LD (HL), D") => self.store_hl(self.cpu.d),
            opcode!("LD (HL), E") => self.store_hl(self.cpu.e),
            opcode!("LD (HL), H") => self.store_hl(self.cpu.h),
            opcode!("LD (HL), L") => self.store_hl(self.cpu.l),
            opcode!("LD (HL), A") => self.store_hl(self.cpu.a),

            opcode!("LD A, (DE)") => {
                let val = self.load_byte(self.cpu.de());
                self.cpu.a = val;
            }

            opcode!("LD HL, d16") => self.cpu.set_hl(arg_word),
            opcode!("LD DE, d16") => self.cpu.set_de(arg_word),
            opcode!("LD SP, d16") => self.cpu.sp = arg_word,

            opcode!("LD (C), A") => {
                let dst = Word::new(0xFF00) + self.cpu.c;
                self.store_byte(dst, self.cpu.a);
            }
            opcode!("LD (a16), A") => self.store_byte(arg_word, self.cpu.a),
            opcode!("LDH (a8), A") => {
                let dst = Word::new(0xFF00) + arg_byte;
                self.store_byte(dst, self.cpu.a);
            }
            opcode!("LDH A, (a8)") => {
                let src = Word::new(0xFF00) + arg_byte;
                self.cpu.a = self.load_byte(src);
            }

            opcode!("LD (HL+), A") => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);
                self.cpu.set_hl(dst + 1u16);
            }
            opcode!("LD (HL-), A") => {
                let dst = self.cpu.hl();
                self.store_byte(dst, self.cpu.a);
                self.cpu.set_hl(dst - 1);
            }

            // ========== DEC ==========
            opcode!("DEC B") => dec!(self.cpu.b),
            opcode!("DEC D") => dec!(self.cpu.d),
            opcode!("DEC H") => dec!(self.cpu.h),
            opcode!("DEC C") => dec!(self.cpu.c),
            opcode!("DEC E") => dec!(self.cpu.e),
            opcode!("DEC L") => dec!(self.cpu.l),
            opcode!("DEC A") => dec!(self.cpu.a),

            opcode!("DEC BC") => self.cpu.set_bc(self.cpu.bc() - 1u16),
            opcode!("DEC DE") => self.cpu.set_de(self.cpu.de() - 1u16),
            opcode!("DEC HL") => self.cpu.set_hl(self.cpu.hl() - 1u16),
            opcode!("DEC SP") => self.cpu.sp -= 1u16,

            // ========== INC ==========
            opcode!("INC B") => inc!(self.cpu.b),
            opcode!("INC D") => inc!(self.cpu.d),
            opcode!("INC H") => inc!(self.cpu.h),
            opcode!("INC C") => inc!(self.cpu.c),
            opcode!("INC E") => inc!(self.cpu.e),
            opcode!("INC L") => inc!(self.cpu.l),
            opcode!("INC A") => inc!(self.cpu.a),

            opcode!("INC BC") => self.cpu.set_bc(self.cpu.bc() + 1u16),
            opcode!("INC DE") => self.cpu.set_de(self.cpu.de() + 1u16),
            opcode!("INC HL") => self.cpu.set_hl(self.cpu.hl() + 1u16),
            opcode!("INC SP") => self.cpu.sp += 1u16,

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
            opcode!("JR r8") => self.cpu.pc += arg_byte.get() as i8,
            opcode!("JR NZ, r8") => {
                if !self.cpu.zero() {
                    self.cpu.pc += arg_byte.get() as i8;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("JR Z, r8") => {
                if self.cpu.zero() {
                    self.cpu.pc += arg_byte.get() as i8;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }

            // ========== POP/PUSH ==========
            opcode!("POP BC") => {
                let val = self.load_word(self.cpu.sp);
                self.cpu.sp += 2u16;
                self.cpu.set_bc(val);
            }
            opcode!("PUSH BC") => self.push(self.cpu.bc()),
            opcode!("PUSH DE") => self.push(self.cpu.de()),
            opcode!("PUSH HL") => self.push(self.cpu.hl()),
            opcode!("PUSH AF") => self.push(self.cpu.af()),

            // ========== CALL/RET ==========
            opcode!("CALL a16") => {
                self.push(self.cpu.pc);
                self.cpu.pc = arg_word;
            }
            opcode!("RET") => {
                let val = self.load_word(self.cpu.sp);
                self.cpu.pc = val;
                self.cpu.sp += 2u16;
            }
            opcode!("RETI") => {
                // Return
                let val = self.load_word(self.cpu.sp);
                self.cpu.pc = val;
                self.cpu.sp += 2u16;

                // Enable interrupts
                self.interrupt_controller.ime = true;
            }

            // ========== miscellaneous ==========
            opcode!("RLA") => {
                let carry = self.cpu.a.rotate_left_through_carry(self.cpu.carry());
                set_flags!(self.cpu.f => 0 0 0 carry);
            }
            opcode!("DI") => self.interrupt_controller.ime = false,
            opcode!("EI") => self.enable_interrupts_next_step = true,
            opcode!("HALT") => self.halt = true,

            opcode!("PREFIX CB") => {
                let instr_start = self.cpu.pc + 1u16;
                let op_code = self.load_byte(instr_start);
                let instr = PREFIXED_INSTRUCTIONS[op_code];
                self.cpu.pc += instr.len as u16;

                match op_code.get() {
                    // ========== RLC ==========
                    prefixed_opcode!("RLC B") => rlc!(self.cpu.b),
                    prefixed_opcode!("RLC C") => rlc!(self.cpu.c),
                    prefixed_opcode!("RLC D") => rlc!(self.cpu.d),
                    prefixed_opcode!("RLC E") => rlc!(self.cpu.e),
                    prefixed_opcode!("RLC H") => rlc!(self.cpu.h),
                    prefixed_opcode!("RLC L") => rlc!(self.cpu.l),
                    prefixed_opcode!("RLC (HL)") => {
                        let mut val = self.load_byte(self.cpu.hl());
                        rlc!(val);
                        self.store_byte(self.cpu.hl(), val);
                    },
                    prefixed_opcode!("RLC A") => rlc!(self.cpu.a),

                    // ========== RRC ==========
                    prefixed_opcode!("RRC B") => rrc!(self.cpu.b),
                    prefixed_opcode!("RRC C") => rrc!(self.cpu.c),
                    prefixed_opcode!("RRC D") => rrc!(self.cpu.d),
                    prefixed_opcode!("RRC E") => rrc!(self.cpu.e),
                    prefixed_opcode!("RRC H") => rrc!(self.cpu.h),
                    prefixed_opcode!("RRC L") => rrc!(self.cpu.l),
                    prefixed_opcode!("RRC (HL)") => {
                        let mut val = self.load_byte(self.cpu.hl());
                        rrc!(val);
                        self.store_byte(self.cpu.hl(), val);
                    },
                    prefixed_opcode!("RRC A") => rrc!(self.cpu.a),

                    // ========== RL ==========
                    prefixed_opcode!("RL B") => rl!(self.cpu.b),
                    prefixed_opcode!("RL C") => rl!(self.cpu.c),
                    prefixed_opcode!("RL D") => rl!(self.cpu.d),
                    prefixed_opcode!("RL E") => rl!(self.cpu.e),
                    prefixed_opcode!("RL H") => rl!(self.cpu.h),
                    prefixed_opcode!("RL L") => rl!(self.cpu.l),
                    prefixed_opcode!("RL (HL)") => {
                        let mut val = self.load_byte(self.cpu.hl());
                        rl!(val);
                        self.store_byte(self.cpu.hl(), val);
                    },
                    prefixed_opcode!("RL A") => rl!(self.cpu.a),

                    // ========== RR ==========
                    prefixed_opcode!("RR B") => rr!(self.cpu.b),
                    prefixed_opcode!("RR C") => rr!(self.cpu.c),
                    prefixed_opcode!("RR D") => rr!(self.cpu.d),
                    prefixed_opcode!("RR E") => rr!(self.cpu.e),
                    prefixed_opcode!("RR H") => rr!(self.cpu.h),
                    prefixed_opcode!("RR L") => rr!(self.cpu.l),
                    prefixed_opcode!("RR (HL)") => {
                        let mut val = self.load_byte(self.cpu.hl());
                        rr!(val);
                        self.store_byte(self.cpu.hl(), val);
                    },
                    prefixed_opcode!("RR A") => rr!(self.cpu.a),

                    // ========== SLA ==========
                    prefixed_opcode!("SLA B") => sla!(self.cpu.b),
                    prefixed_opcode!("SLA C") => sla!(self.cpu.c),
                    prefixed_opcode!("SLA D") => sla!(self.cpu.d),
                    prefixed_opcode!("SLA E") => sla!(self.cpu.e),
                    prefixed_opcode!("SLA H") => sla!(self.cpu.h),
                    prefixed_opcode!("SLA L") => sla!(self.cpu.l),
                    prefixed_opcode!("SLA (HL)") => {
                        let mut val = self.load_byte(self.cpu.hl());
                        sla!(val);
                        self.store_byte(self.cpu.hl(), val);
                    },
                    prefixed_opcode!("SLA A") => sla!(self.cpu.a),

                    // ========== SRL ==========
                    prefixed_opcode!("SRL B") => srl!(self.cpu.b),
                    prefixed_opcode!("SRL C") => srl!(self.cpu.c),
                    prefixed_opcode!("SRL D") => srl!(self.cpu.d),
                    prefixed_opcode!("SRL E") => srl!(self.cpu.e),
                    prefixed_opcode!("SRL H") => srl!(self.cpu.h),
                    prefixed_opcode!("SRL L") => srl!(self.cpu.l),
                    prefixed_opcode!("SRL (HL)") => {
                        let mut val = self.load_byte(self.cpu.hl());
                        srl!(val);
                        self.store_byte(self.cpu.hl(), val);
                    },
                    prefixed_opcode!("SRL A") => srl!(self.cpu.a),

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
            }

            _ => {
                debug!(
                    "Template:\n\
                    opcode!(\"{}\") => {{\
                    \n\
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
        }

        // Unwrap the action_taken `Option` to check, if it was set when we get a branch instruction
        let action_taken = match (instr.clocks_taken, action_taken) {
            (Some(_), Some(b)) => b,
            (Some(_), None) => {
                terminate!(
                    "action_taken not set for branch instruction {:?} in position: \
                        {} after: {} cycles!",
                    instr,
                    instr_start,
                    self.cycle_counter,
                );
            }
            (None, Some(_)) => {
                terminate!(
                    "action_taken set for non-branch instruction {:?} in position: \
                        {} after: {} cycles!",
                    instr,
                    instr_start,
                    self.cycle_counter,
                );
            }
            (None, None) => false,
        };

        let clocks_spent = if op_code.get() == opcode!("PREFIX CB") {
            PREFIXED_INSTRUCTIONS[op_code].clocks
        } else if action_taken {
            match instr.clocks_taken {
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
            instr.clocks
        };

        // Internally, we work with 4Mhz clocks. All instructions take a
        // multiple of 4 many clocks. The rest of the emulator works with 1Mhz
        // cycles, so we can simply divide by 4.
        Ok(clocks_spent / 4)
    }
}
