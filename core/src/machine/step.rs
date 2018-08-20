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
        let instr = INSTRUCTIONS[op_code.get() as usize]
            .expect(&format!("Unknown instruction {} in position: {}", op_code, self.cpu.pc));

        match op_code {
            _ => {
                error!("Unimplemented instruction {:?} in position: {}", instr, self.cpu.pc);
                return Err(Disruption::Terminated);
            }
        }

        // TODO: increment cycle counter
        // self.cycle_counter.inc();
    }
}
