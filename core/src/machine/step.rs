use super::{
    Machine,
    instr::INSTRUCTIONS,
};
use crate::{
    Disruption,
    log::*,
};


impl Machine {
    /// Executes one (the next) operation.
    pub(crate) fn step(&mut self) -> Result<(), Disruption> {
        let op_code = self.load_byte(self.cpu.pc);
        let instr = match INSTRUCTIONS[op_code.get() as usize] {
            Some(v) => v,
            None => {
                error!("Unknown instruction {} in position: {}", op_code, self.cpu.pc);
                return Err(Disruption::Terminated);
            }
        };

        match op_code.get() {
            // 0x3_
            0x31 => {
                let immediate = self.load_word(self.cpu.pc + 1);
                self.cpu.sp = immediate;

                self.cpu.pc += instr.len as u16;
                self.cycle_counter += instr.cycles;
            }

            _ => {
                error!("Unimplemented instruction {:?} in position: {}", instr, self.cpu.pc);
                return Err(Disruption::Terminated);
            }
        }

        // TODO: increment cycle counter
        // self.cycle_counter.inc();

        Ok(())
    }
}
