pub mod alu;
pub mod cpu;
pub mod flags;
pub mod mem;
pub mod reg;

pub use cpu::{Cpu, Snapshot};
pub use flags::Flags;
pub use mem::Memory;
pub use reg::{Reg16, Reg8, RegFile, Seg};

#[cfg(test)]
mod tests {
    #[test]
    fn ci_path_is_alive() {
        assert_eq!(2 + 2, 4);
    }
}
