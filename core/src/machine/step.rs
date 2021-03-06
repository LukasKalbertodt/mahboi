//! Contains code to actually execute instructions.

use super::{Machine, State};
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
        if self.state == State::Halted {
            // If no interrupt was triggered (otherwise we wouldn't have gotten here) but at least
            // one interrupt was requested -> exit HALT mode.
            if self.interrupt_controller.is_interrupt_requested() {
                debug!("Interrupt in HALT mode: CPU woke up");
                self.state = State::Normal;
            }

            // Executed 1 cycle doing nothing ＼(＾O＾)／
            return Ok(1);
        } else if self.state == State::Stopped {
            // If any selected button is pressed, we exit STOP mode. I'm not
            // 100% sure this is the correct behavior. Manuals mention it like
            // that but the `cpu_instr.gb` combined ROM disables all buttons
            // and executes `STOP`... which should freeze everything?
            if self.input_controller.load_register().get() & 0b1111 != 0b1111 {
                self.state = State::Normal;
                self.ppu.enable();
            }

            return Ok(1);
        }

        // Variable initialization
        let instr_start = self.cpu.pc;
        let arg_byte = self.load_byte(instr_start + 1u16);
        let arg_word = self.load_word(instr_start + 1u16);
        let op_code = self.load_byte(instr_start);
        let mut instr = match INSTRUCTIONS[op_code] {
            Some(v) => v,
            None => {
                // TODO: we might want to treat this just as a NOP instruction
                // (i.e. ignore the problem) or exit more gracefully or freeze
                // the emulator. Not quite clear what's supposed to happen.
                terminate!("Invalid opcode {} at position {}", op_code, instr_start);
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
                // let val = $x - (self.cpu.carry() as u8);
                // sub!(val);
                let (carry, half_carry) = self.cpu.a.full_sub_with_carries($x, self.cpu.carry());
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 1 half_carry carry);
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

        /// This is a template macro for all ADD HL, w instructions (where `w` should
        /// be a [`Word`]).
        macro_rules! add_hl {
            ($x:expr) => {{
                let mut val = self.cpu.hl();
                let (carry, half_carry) = val.add_with_carries($x);
                set_flags!(self.cpu.f => - 0 half_carry carry);
                self.cpu.set_hl(val);
            }}
        }

        /// This is a template macro for all ADC A, b instructions (where `b` should be a [`Byte`]).
        macro_rules! adc {
            ($x:expr) => {{
                let (carry, half_carry) = self.cpu.a.full_add_with_carries($x, self.cpu.carry());
                let zero = self.cpu.a == Byte::zero();
                set_flags!(self.cpu.f => zero 0 half_carry carry);
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
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all RR b instructions (where `b` should be a [`Byte`]).
        macro_rules! rr {
            ($x:expr) => {{
                let carry = $x.rotate_right_through_carry(self.cpu.carry());
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all SLA b instructions (where `b` should be a [`Byte`]).
        macro_rules! sla {
            ($x:expr) => {{
                let carry = $x.shift_left();
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all SRL b instructions (where `b` should be a [`Byte`]).
        macro_rules! srl {
            ($x:expr) => {{
                let carry = $x.shift_right();
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all SRA b instructions (where `b` should be a [`Byte`]).
        macro_rules! sra {
            ($x:expr) => {{
                let carry = $x.arithmetic_shift_right();
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 carry);
            }}
        }

        /// This is a template macro for all SWAP b instructions (where `b` should be a [`Byte`]).
        macro_rules! swap {
            ($x:expr) => {{
                $x = $x.swap_nybbles();
                let zero = $x == Byte::zero();
                set_flags!(self.cpu.f => zero 0 0 0);
            }}
        }

        /// This is a convenience macro for all RET-like instructions to reduce duplicate code.
        macro_rules! ret {
            () => {{
                self.cpu.pc = self.pop();
            }}
        }

        /// This is a convenience macro for all CALL-like instructions to reduce duplicate code.
        macro_rules! call {
            ($x:expr) => {{
                self.push(self.cpu.pc);
                self.cpu.pc = $x;
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
            opcode!("LD (HL), d8") => self.store_hl(arg_byte),

            opcode!("LD BC, d16") => self.cpu.set_bc(arg_word),
            opcode!("LD DE, d16") => self.cpu.set_de(arg_word),
            opcode!("LD HL, d16") => self.cpu.set_hl(arg_word),
            opcode!("LD SP, d16") => self.cpu.sp = arg_word,
            opcode!("LD SP, HL") => self.cpu.sp = self.cpu.hl(),
            opcode!("LD HL, SP+r8") => {
                let mut src = self.cpu.sp;
                let (carry, half_carry) = src.add_i8_with_carries(arg_byte.get() as i8);
                set_flags!(self.cpu.f => 0 0 half_carry carry);
                self.cpu.set_hl(src);
            }
            opcode!("LD (a16), SP") => self.store_word(arg_word, self.cpu.sp),

            opcode!("LD (C), A") => {
                let dst = Word::new(0xFF00) + self.cpu.c;
                self.store_byte(dst, self.cpu.a);
            }
            opcode!("LD A, (C)") => {
                self.cpu.a = self.load_byte(Word::new(0xFF00) + self.cpu.c);
            }
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
            opcode!("LD A, (HL+)") => {
                let dst = self.cpu.hl();
                self.cpu.a = self.load_byte(dst);
                self.cpu.set_hl(dst + 1u16);
            }
            opcode!("LD A, (HL-)") => {
                let dst = self.cpu.hl();
                self.cpu.a = self.load_byte(dst);
                self.cpu.set_hl(dst - 1u16);
            }
            opcode!("LD A, (DE)") => self.cpu.a = self.load_byte(self.cpu.de()),
            opcode!("LD A, (BC)") => self.cpu.a = self.load_byte(self.cpu.bc()),
            opcode!("LD A, (a16)") => self.cpu.a = self.load_byte(arg_word),
            opcode!("LD (DE), A") => self.store_byte(self.cpu.de(), self.cpu.a),
            opcode!("LD (BC), A") => self.store_byte(self.cpu.bc(), self.cpu.a),
            opcode!("LD (a16), A") => self.store_byte(arg_word, self.cpu.a),

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
            opcode!("DEC (HL)") => {
                let mut val = self.load_hl();
                dec!(val);
                self.store_hl(val);
            }

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
            opcode!("INC (HL)") => {
                let mut val = self.load_hl();
                inc!(val);
                self.store_hl(val);
            }

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

            opcode!("ADD HL, BC") => add_hl!(self.cpu.bc()),
            opcode!("ADD HL, DE") => add_hl!(self.cpu.de()),
            opcode!("ADD HL, HL") => add_hl!(self.cpu.hl()),
            opcode!("ADD HL, SP") => add_hl!(self.cpu.sp),

            opcode!("ADD SP, r8") => {
                let (carry, half_carry) = self.cpu.sp.add_i8_with_carries(arg_byte.get() as i8);
                set_flags!(self.cpu.f => 0 0 half_carry carry);
            }

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

            // ========== RST ==========
            opcode!("RST 00H") => call!(Word::new(0x00)),
            opcode!("RST 08H") => call!(Word::new(0x08)),
            opcode!("RST 10H") => call!(Word::new(0x10)),
            opcode!("RST 18H") => call!(Word::new(0x18)),
            opcode!("RST 20H") => call!(Word::new(0x20)),
            opcode!("RST 28H") => call!(Word::new(0x28)),
            opcode!("RST 30H") => call!(Word::new(0x30)),
            opcode!("RST 38H") => call!(Word::new(0x38)),

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
            opcode!("JR NC, r8") => {
                if !self.cpu.carry() {
                    self.cpu.pc += arg_byte.get() as i8;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("JR C, r8") => {
                if self.cpu.carry() {
                    self.cpu.pc += arg_byte.get() as i8;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }

            // ========== JP ==========
            opcode!("JP a16") => self.cpu.pc = arg_word,
            opcode!("JP HL") => self.cpu.pc = self.cpu.hl(),
            opcode!("JP Z, a16") => {
                if self.cpu.zero() {
                    self.cpu.pc = arg_word;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("JP C, a16") => {
                if self.cpu.carry() {
                    self.cpu.pc = arg_word;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("JP NZ, a16") => {
                if !self.cpu.zero() {
                    self.cpu.pc = arg_word;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("JP NC, a16") => {
                if !self.cpu.carry() {
                    self.cpu.pc = arg_word;
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }

            // ========== POP/PUSH ==========
            opcode!("POP BC") => {
                let val = self.pop();
                self.cpu.set_bc(val);
            }
            opcode!("POP DE") => {
                let val = self.pop();
                self.cpu.set_de(val);
            },
            opcode!("POP HL") => {
                let val = self.pop();
                self.cpu.set_hl(val);
            },
            opcode!("POP AF") => {
                let val = self.pop();
                self.cpu.set_af(val);
            },
            opcode!("PUSH BC") => self.push(self.cpu.bc()),
            opcode!("PUSH DE") => self.push(self.cpu.de()),
            opcode!("PUSH HL") => self.push(self.cpu.hl()),
            opcode!("PUSH AF") => self.push(self.cpu.af()),

            // ========== CALL ==========
            opcode!("CALL a16") => call!(arg_word),
            opcode!("CALL NZ, a16") => {
                if !self.cpu.zero() {
                    call!(arg_word);
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("CALL Z, a16") => {
                if self.cpu.zero() {
                    call!(arg_word);
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("CALL NC, a16") => {
                if !self.cpu.carry() {
                    call!(arg_word);
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("CALL C, a16") => {
                if self.cpu.carry() {
                    call!(arg_word);
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }

            // ========== RET ==========
            opcode!("RET") => ret!(),
            opcode!("RET NZ") => {
                if !self.cpu.zero() {
                    ret!();
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("RET NC") => {
                if !self.cpu.carry() {
                    ret!();
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("RET Z") => {
                if self.cpu.zero() {
                    ret!();
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("RET C") => {
                if self.cpu.carry() {
                    ret!();
                    action_taken = Some(true);
                } else {
                    action_taken = Some(false);
                }
            }
            opcode!("RETI") => {
                ret!();
                // Enable interrupts
                self.interrupt_controller.ime = true;
            }

            // ========== Non-prefix rotate instructions ==========
            opcode!("RLA") => {
                let carry = self.cpu.a.rotate_left_through_carry(self.cpu.carry());
                set_flags!(self.cpu.f => 0 0 0 carry);
            }
            opcode!("RRA") => {
                let carry = self.cpu.a.rotate_right_through_carry(self.cpu.carry());
                set_flags!(self.cpu.f => 0 0 0 carry);
            }
            opcode!("RLCA") => {
                let carry = self.cpu.a.rotate_left();
                set_flags!(self.cpu.f => 0 0 0 carry);
            }
            opcode!("RRCA") => {
                let carry = self.cpu.a.rotate_right();
                set_flags!(self.cpu.f => 0 0 0 carry);
            }

            // ========== miscellaneous ==========
            opcode!("SCF") => {
                set_flags!(self.cpu.f => - 0 0 1);
            }
            opcode!("CCF") => {
                let carry = !self.cpu.carry();
                set_flags!(self.cpu.f => - 0 0 carry);
            }
            opcode!("DAA") => {
                let carry = self.cpu.daa();
                let zero = self.cpu.a == 0;
                set_flags!(self.cpu.f => zero - 0 carry);
            }
            opcode!("DI") => self.interrupt_controller.ime = false,
            opcode!("EI") => self.enable_interrupts_next_step = true,
            opcode!("HALT") => {
                debug!("Executed HALT: CPU entering HALT mode");
                self.state = State::Halted;
            },
            opcode!("STOP") => {
                debug!("Executed STOP: CPU entering ultra-low power mode");

                let any_buttons_select = self.input_controller.is_button_selected()
                    || self.input_controller.is_direction_selected();
                if !any_buttons_select {
                    error!("STOP instruction executed, but no buttons are selected, meaning \
                        that there is no way to exit this STOP mode");
                }

                // TODO: this is most likely still incorrect in some ways
                self.ppu.disable();
                self.state = State::Stopped;
            }
            opcode!("NOP") => {}, // Just do nothing _(:3」∠)_
            opcode!("CPL") => {
                self.cpu.a = !self.cpu.a;
                set_flags!(self.cpu.f => - 1 1 -);
            }

            opcode!("PREFIX CB") => {
                let instr_start = self.cpu.pc + 1u16;
                let op_code = self.load_byte(instr_start);
                instr = PREFIXED_INSTRUCTIONS[op_code];
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
                        let mut val = self.load_hl();
                        rlc!(val);
                        self.store_hl(val);
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
                        let mut val = self.load_hl();
                        rrc!(val);
                        self.store_hl(val);
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
                        let mut val = self.load_hl();
                        rl!(val);
                        self.store_hl(val);
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
                        let mut val = self.load_hl();
                        rr!(val);
                        self.store_hl(val);
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
                        let mut val = self.load_hl();
                        sla!(val);
                        self.store_hl(val);
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
                        let mut val = self.load_hl();
                        srl!(val);
                        self.store_hl(val);
                    },
                    prefixed_opcode!("SRL A") => srl!(self.cpu.a),

                    // ========== SRA ==========
                    prefixed_opcode!("SRA B") => sra!(self.cpu.b),
                    prefixed_opcode!("SRA C") => sra!(self.cpu.c),
                    prefixed_opcode!("SRA D") => sra!(self.cpu.d),
                    prefixed_opcode!("SRA E") => sra!(self.cpu.e),
                    prefixed_opcode!("SRA H") => sra!(self.cpu.h),
                    prefixed_opcode!("SRA L") => sra!(self.cpu.l),
                    prefixed_opcode!("SRA (HL)") => {
                        let mut val = self.load_hl();
                        sra!(val);
                        self.store_hl(val);
                    },
                    prefixed_opcode!("SRA A") => sra!(self.cpu.a),

                    // ========== SWAP ==========
                    prefixed_opcode!("SWAP B") => swap!(self.cpu.b),
                    prefixed_opcode!("SWAP C") => swap!(self.cpu.c),
                    prefixed_opcode!("SWAP D") => swap!(self.cpu.d),
                    prefixed_opcode!("SWAP E") => swap!(self.cpu.e),
                    prefixed_opcode!("SWAP H") => swap!(self.cpu.h),
                    prefixed_opcode!("SWAP L") => swap!(self.cpu.l),
                    prefixed_opcode!("SWAP (HL)") => {
                        let mut val = self.load_hl();
                        swap!(val);
                        self.store_hl(val);
                    },
                    prefixed_opcode!("SWAP A") => swap!(self.cpu.a),

                    // ========== BIT/RES/SET ==========
                    opcode @ 0x40..=0xFF => {
                        // All BIT/RES/SET instructions follow the same structure. Because of this
                        // all three instructions are handled in this match arm to reduce
                        // duplicate code.
                        //
                        // The opcode structure is the following:
                        // 00 000 000
                        // ^^ ^^^ ^^^
                        // || ||| |||
                        // || ||| --------> The first three bits encode the register which is
                        // || |||           used (0: B, 1: C, 2: D, 3: E, 4: H, 5: L, 6: (HL), 7: A)
                        // ||  -----------> The next three bits encode the bit which should be
                        // ||               passed to the instruction (0: LSB, up to 7: MSB)
                        //  --------------> The last two bits encode the instruction which should
                        //                  be executed (1: BIT, 2: RES, 3: SET)

                        // Select register
                        let register_code = opcode & 0b0000_0111;

                        // Select instruction
                        let instr_code = (opcode & 0b1100_0000) >> 6;

                        // Select bit
                        let bit = (opcode & 0b0011_1000) >> 3;

                        // Get bit mask
                        let mask = Byte::new(0b0000_0001 << bit);

                        // Handle (HL) in a special way, because we can't create a mutable borrow
                        // of it
                        if register_code == 6 {
                            let byte = self.load_hl();
                            match instr_code {
                                1 => {
                                    let zero = (byte & mask) == 0;
                                    set_flags!(self.cpu.f => zero 0 1 -);
                                }
                                2 => self.store_hl(byte & !mask),
                                3 => self.store_hl(byte | mask),
                                _ => unreachable!(),
                            }
                        } else {
                            // Create a mutable borrow of the selected register and apply the
                            // instruction on it
                            let reg = match register_code {
                                0 => &mut self.cpu.b,
                                1 => &mut self.cpu.c,
                                2 => &mut self.cpu.d,
                                3 => &mut self.cpu.e,
                                4 => &mut self.cpu.h,
                                5 => &mut self.cpu.l,
                                7 => &mut self.cpu.a,
                                _ => unreachable!(),
                            };
                            match instr_code {
                                1 => {
                                    let zero = (*reg & mask) == 0;
                                    set_flags!(self.cpu.f => zero 0 1 -);
                                }
                                2 => *reg &= !mask,
                                3 => *reg |= mask,
                                _ => unreachable!(),
                            }
                        }
                    }
                }
            }

            // Invalid Opcodes
            0xD3 | 0xDB | 0xDD | 0xE3 | 0xE4 | 0xEB | 0xEC | 0xED | 0xF4 | 0xFC | 0xFD => {
                // We already try to decode the instruction above. If that
                // fails, it panics.
                unreachable!()
            }
        }

        // Unwrap the action_taken `Option` to check, if it was set when we get a branch instruction
        let action_taken = match (instr.clocks_taken, action_taken) {
            (Some(_), Some(b)) => b,
            (Some(_), None) => {
                terminate!(
                    "bug: `action_taken` not set for branch instruction {:?} at {}",
                    instr,
                    instr_start,
                );
            }
            (None, Some(_)) => {
                terminate!(
                    "bug: `action_taken` set for non-branch instruction {:?} at {}",
                    instr,
                    instr_start,
                );
            }
            (None, None) => false,
        };

        let clocks_spent = if action_taken {
            match instr.clocks_taken {
                Some(c) => c,
                None => unreachable!(), // already checked above
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
