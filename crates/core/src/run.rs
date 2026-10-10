//! Batch execution shared by the debugger frontends.
//!
//! `Cpu::run` executes instructions until the CPU halts, a step budget runs
//! out, a breakpoint is reached, or an error occurs. Breakpoints are linear
//! addresses supplied per call, so the CPU owns no breakpoint state and runs
//! stay replayable from any host (CLI keeps its own tracing loop, which
//! prints per-step state that a silent batch loop cannot).
//!
//! Breakpoint semantics mirror a debugger's *continue*: a run entered at a
//! breakpoint address does not stop before executing it — the stop happens
//! when the instruction pointer *arrives* at the breakpoint after at least
//! one executed step.

use std::collections::BTreeSet;

use crate::cpu::{Cpu, StepError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunStop {
    /// The CPU executed HLT (or was already halted when the run began).
    Halted,
    /// `max_steps` executed without reaching HLT.
    StepLimit,
    /// The instruction pointer arrived at a breakpoint address.
    Breakpoint,
    /// The CPU rejected an instruction at the recorded CS:IP.
    Error { cs: u16, ip: u16, err: StepError },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResult {
    /// Instructions executed before the run stopped.
    pub steps: u64,
    pub stop: RunStop,
}

impl Cpu {
    pub fn run(&mut self, max_steps: u64, breakpoints: &BTreeSet<u32>) -> RunResult {
        let mut steps: u64 = 0;
        loop {
            if self.halted {
                return RunResult {
                    steps,
                    stop: RunStop::Halted,
                };
            }
            if steps >= max_steps {
                return RunResult {
                    steps,
                    stop: RunStop::StepLimit,
                };
            }
            let before = self.snapshot();
            match self.step() {
                Ok(()) => {}
                Err(StepError::Halted) => {
                    return RunResult {
                        steps,
                        stop: RunStop::Halted,
                    };
                }
                Err(err) => {
                    return RunResult {
                        steps,
                        stop: RunStop::Error {
                            cs: before.cs,
                            ip: before.ip,
                            err,
                        },
                    };
                }
            }
            steps += 1;
            if breakpoints.contains(&(self.linear_ip() as u32)) {
                return RunResult {
                    steps,
                    stop: RunStop::Breakpoint,
                };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reg::{Reg16, Seg};

    const RESET_LINEAR: u32 = 0xF_FFF0;

    fn cpu_with(code: &[u8]) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.load_flat(0xFFFF, 0x0000, code);
        cpu
    }

    fn entry(offset: u16) -> u32 {
        RESET_LINEAR.wrapping_add(offset as u32) & 0xF_FFFF
    }

    fn two_movs_then_hlt() -> Vec<u8> {
        vec![
            0xB8, 0x01, 0x00, // mov ax, 0x0001
            0xBB, 0x02, 0x00, // mov bx, 0x0002
            0xF4, // hlt
        ]
    }

    #[test]
    fn runs_to_halt_and_counts_every_instruction() {
        let mut cpu = cpu_with(&two_movs_then_hlt());
        let result = cpu.run(100, &BTreeSet::new());
        assert_eq!(result.steps, 3);
        assert_eq!(result.stop, RunStop::Halted);
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x0001);
        assert_eq!(cpu.regs.reg(Reg16::Bx), 0x0002);
    }

    #[test]
    fn zero_budget_stops_before_the_first_step() {
        let mut cpu = cpu_with(&two_movs_then_hlt());
        let result = cpu.run(0, &BTreeSet::new());
        assert_eq!(result.steps, 0);
        assert_eq!(result.stop, RunStop::StepLimit);
        assert_eq!(cpu.ip, 0x0000);
    }

    #[test]
    fn budget_stops_mid_program_with_partial_state() {
        let mut cpu = cpu_with(&two_movs_then_hlt());
        let result = cpu.run(2, &BTreeSet::new());
        assert_eq!(result.steps, 2);
        assert_eq!(result.stop, RunStop::StepLimit);
        assert_eq!(cpu.regs.reg(Reg16::Bx), 0x0002);
        assert!(!cpu.halted);
    }

    #[test]
    fn breakpoint_stops_when_execution_arrives() {
        let mut cpu = cpu_with(&two_movs_then_hlt());
        let bps = BTreeSet::from([entry(3)]);
        let result = cpu.run(100, &bps);
        assert_eq!(result.steps, 1);
        assert_eq!(result.stop, RunStop::Breakpoint);
        assert_eq!(cpu.ip, 0x0003);
    }

    #[test]
    fn breakpoint_at_entry_does_not_stop_before_the_first_step() {
        let mut cpu = cpu_with(&two_movs_then_hlt());
        let bps = BTreeSet::from([entry(0)]);
        let result = cpu.run(100, &bps);
        assert_eq!(result.stop, RunStop::Halted);
        assert_eq!(result.steps, 3);
    }

    #[test]
    fn breakpoint_at_entry_stops_on_the_next_arrival() {
        // A program that jumps back to its own entry: the entry breakpoint
        // fires on the second arrival, after the back-edge executes.
        //   0: B8 01 00        mov ax, 0x0001
        //   3: EB FB           jmp short -5  -> back to 0
        let mut cpu = cpu_with(&[0xB8, 0x01, 0x00, 0xEB, 0xFB]);
        let bps = BTreeSet::from([entry(0)]);
        let result = cpu.run(100, &bps);
        assert_eq!(result.steps, 2);
        assert_eq!(result.stop, RunStop::Breakpoint);
        assert_eq!(cpu.ip, 0x0000);
    }

    #[test]
    fn error_reports_the_rejected_instruction() {
        let mut cpu = cpu_with(&[0x0F]);
        let result = cpu.run(100, &BTreeSet::new());
        assert_eq!(result.steps, 0);
        assert_eq!(
            result.stop,
            RunStop::Error {
                cs: 0xFFFF,
                ip: 0x0000,
                err: StepError::UnknownOpcode(0x0F),
            }
        );
    }

    #[test]
    fn running_a_halted_cpu_is_a_no_op() {
        let mut cpu = cpu_with(&[0xF4]);
        cpu.step().unwrap();
        let result = cpu.run(100, &BTreeSet::new());
        assert_eq!(result.steps, 0);
        assert_eq!(result.stop, RunStop::Halted);
    }

    #[test]
    fn breakpoint_addresses_are_linear_not_segmented() {
        // The image lives at FFFF:0000 but the CPU starts with a different
        // segment pairing (FFF0:00F3 — the same physical entry point). The
        // breakpoint is a linear address, so the arrival after the first jump
        // matches even though CS:IP never equals FFFF:xxxx along the way.
        let mut cpu = cpu_with(&[0xEB, 0x01, 0x00, 0xB8, 0x01, 0x00, 0xF4]);
        let bps = BTreeSet::from([entry(3)]);
        cpu.regs.set_seg(Seg::Cs, 0xFFF0);
        cpu.ip = 0x00F0;
        assert_eq!(cpu.linear_ip() as u32, entry(0));
        let result = cpu.run(100, &bps);
        assert_eq!(result.steps, 1);
        assert_eq!(result.stop, RunStop::Breakpoint);
        assert_eq!(cpu.linear_ip() as u32, entry(3));
    }
}
