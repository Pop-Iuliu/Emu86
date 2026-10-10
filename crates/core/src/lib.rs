pub mod alu;
pub mod cpu;
pub mod disasm;
pub mod flags;
pub mod mem;
pub mod modrm;
pub mod reg;
pub mod run;

pub use cpu::{Cpu, Snapshot, StepError};
pub use disasm::{disassemble, Insn, MAX_INSNS};
pub use flags::Flags;
pub use mem::Memory;
pub use modrm::Operand;
pub use reg::{Reg16, Reg8, RegFile, Seg};
pub use run::{RunResult, RunStop};

#[cfg(test)]
mod tests {
    #[test]
    fn ci_path_is_alive() {
        assert_eq!(2 + 2, 4);
    }
}
