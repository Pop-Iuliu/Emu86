//! Headless runner for flat 8086 binaries.
//!
//! The CLI owns only argument handling and output formatting; every piece of
//! execution lives in `emu86-core`. It loads a flat binary image at a segment
//! and offset, runs until the program halts (or an error or step limit is
//! reached), and prints the resulting CPU state. With `--trace` it also prints
//! one line per executed instruction.
//!
//! Exit codes:
//!   0 -> the guest reached HLT
//!   1 -> the guest raised a CPU error (unknown opcode or unsupported form)
//!   2 -> reserved for argument-parsing errors (emitted by clap)
//!   3 -> the step limit was reached before HLT
//!   4 -> the program image could not be read

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

use emu86_core::{Cpu, Flags, Memory, Seg, Snapshot, StepError};

const EXIT_HALTED: u8 = 0;
const EXIT_EXEC_ERROR: u8 = 1;
const EXIT_STEP_LIMIT: u8 = 3;
const EXIT_IO_ERROR: u8 = 4;

/// Run a flat 8086 binary headlessly and report the resulting CPU state.
#[derive(Debug, Parser)]
#[command(name = "emu86", version)]
struct Cli {
    /// Flat binary image to load and run.
    program: PathBuf,

    /// Segment where the image is loaded and where execution begins.
    #[arg(long, value_name = "HEX", value_parser = parse_u16, default_value = "FFFF")]
    seg: u16,

    /// Offset where the image is loaded and where execution begins.
    #[arg(long, value_name = "HEX", value_parser = parse_u16, default_value = "0000")]
    off: u16,

    /// Instruction budget; execution stops with an error if HLT is not reached.
    #[arg(long, value_name = "N", default_value_t = 1_000_000)]
    max_steps: u64,

    /// Print the executed address and resulting state after each instruction.
    #[arg(long)]
    trace: bool,
}

fn parse_u16(text: &str) -> Result<u16, String> {
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .unwrap_or(text);
    u16::from_str_radix(digits, 16)
        .map_err(|err| format!("expected a 16-bit hexadecimal value, got {text:?}: {err}"))
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let image = match std::fs::read(&cli.program) {
        Ok(image) => image,
        Err(err) => {
            eprintln!("error: cannot read {}: {err}", cli.program.display());
            return ExitCode::from(EXIT_IO_ERROR);
        }
    };

    let mut cpu = Cpu::new();
    cpu.load_flat(cli.seg, cli.off, &image);
    // A flat image has no entry point of its own, so execution starts where it
    // was loaded.
    cpu.regs.set_seg(Seg::Cs, cli.seg);
    cpu.ip = cli.off;

    if cli.trace {
        println!(
            "loaded {} byte(s) at {:04X}:{:04X}",
            image.len(),
            cli.seg,
            cli.off
        );
    }

    let result = run(&mut cpu, cli.max_steps, cli.trace);
    let snapshot = cpu.snapshot();

    match result.outcome {
        Outcome::Halted => {
            println!(
                "halted after {} step(s); {:04X}:{:04X} is past the HLT instruction",
                result.steps, snapshot.cs, snapshot.ip
            );
            print_state(&snapshot);
            ExitCode::from(EXIT_HALTED)
        }
        Outcome::StepLimit => {
            eprintln!(
                "error: step limit of {} reached before HLT at {:04X}:{:04X}",
                cli.max_steps, snapshot.cs, snapshot.ip
            );
            print_state(&snapshot);
            ExitCode::from(EXIT_STEP_LIMIT)
        }
        Outcome::Error(run_error) => {
            report_error(&run_error);
            print_state(&snapshot);
            ExitCode::from(EXIT_EXEC_ERROR)
        }
    }
}

#[derive(Debug)]
struct RunResult {
    outcome: Outcome,
    steps: u64,
}

#[derive(Debug)]
enum Outcome {
    Halted,
    StepLimit,
    Error(RunError),
}

#[derive(Debug)]
struct RunError {
    cs: u16,
    ip: u16,
    err: StepError,
}

fn run(cpu: &mut Cpu, max_steps: u64, trace: bool) -> RunResult {
    let mut steps: u64 = 0;
    loop {
        if cpu.halted {
            return RunResult {
                outcome: Outcome::Halted,
                steps,
            };
        }
        if steps >= max_steps {
            return RunResult {
                outcome: Outcome::StepLimit,
                steps,
            };
        }

        let before = cpu.snapshot();
        match cpu.step() {
            Ok(()) => {}
            Err(StepError::Halted) => {
                return RunResult {
                    outcome: Outcome::Halted,
                    steps,
                };
            }
            Err(err) => {
                return RunResult {
                    outcome: Outcome::Error(RunError {
                        cs: before.cs,
                        ip: before.ip,
                        err,
                    }),
                    steps,
                };
            }
        }

        steps += 1;
        if trace {
            print_trace_step(steps, &before, &cpu.snapshot());
        }
    }
}

fn report_error(run_error: &RunError) {
    let physical = Memory::linear(run_error.cs, run_error.ip);
    match &run_error.err {
        StepError::UnknownOpcode(opcode) => {
            eprintln!(
                "error: unknown opcode {opcode:#04x} at {:04X}:{:04X} (physical {physical:05X})",
                run_error.cs, run_error.ip
            );
        }
        StepError::UnsupportedForm { bytes, .. } => {
            eprintln!(
                "error: unsupported form at {:04X}:{:04X} (physical {physical:05X}): {bytes:02X?}",
                run_error.cs, run_error.ip
            );
        }
        StepError::Halted => {
            eprintln!("error: cpu reported halted during execution");
        }
    }
}

fn print_trace_step(step: u64, before: &Snapshot, after: &Snapshot) {
    let physical = Memory::linear(before.cs, before.ip);
    println!(
        "{step:>6}  {:04X}:{:04X}  phys={physical:05X}  {}",
        before.cs,
        before.ip,
        format_trace_state(after)
    );
}

fn format_trace_state(s: &Snapshot) -> String {
    format!(
        "AX={:04X} CX={:04X} DX={:04X} BX={:04X} SP={:04X} BP={:04X} SI={:04X} DI={:04X} \
         ES={:04X} CS={:04X} SS={:04X} DS={:04X} IP={:04X} FLAGS={:04X} [{}]",
        s.ax,
        s.cx,
        s.dx,
        s.bx,
        s.sp,
        s.bp,
        s.si,
        s.di,
        s.es,
        s.cs,
        s.ss,
        s.ds,
        s.ip,
        s.flags,
        format_flags(s.flags),
    )
}

fn print_state(s: &Snapshot) {
    println!(
        "  AX={:04X}  CX={:04X}  DX={:04X}  BX={:04X}",
        s.ax, s.cx, s.dx, s.bx
    );
    println!(
        "  SP={:04X}  BP={:04X}  SI={:04X}  DI={:04X}",
        s.sp, s.bp, s.si, s.di
    );
    println!(
        "  ES={:04X}  CS={:04X}  SS={:04X}  DS={:04X}",
        s.es, s.cs, s.ss, s.ds
    );
    println!(
        "  IP={:04X}  FLAGS={:04X}  [{}]",
        s.ip,
        s.flags,
        format_flags(s.flags)
    );
}

fn format_flags(bits: u16) -> String {
    // Standard 8086 flag order: OF DF IF TF SF ZF AF PF CF.
    const NAMES: [(u16, char); 9] = [
        (Flags::OF, 'O'),
        (Flags::DF, 'D'),
        (Flags::IF, 'I'),
        (Flags::TF, 'T'),
        (Flags::SF, 'S'),
        (Flags::ZF, 'Z'),
        (Flags::AF, 'A'),
        (Flags::PF, 'P'),
        (Flags::CF, 'C'),
    ];
    NAMES
        .iter()
        .map(|&(mask, name)| if bits & mask != 0 { name } else { '.' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use emu86_core::Reg16;

    /// Byte sequence shared with the core's `memory_program_store_modify_load_halt`
    /// fixture: store a word, modify it in memory, load it back, halt.
    const MEMORY_FIXTURE: [u8; 18] = [
        0xBB, 0x20, 0x00, // mov bx, 0x0020
        0xB8, 0x00, 0x01, // mov ax, 0x0100
        0x89, 0x07, // mov [bx], ax
        0x81, 0x07, 0x00, 0x10, // add word [bx], 0x1000
        0x83, 0x2F, 0xFF, // sub word [bx], -1
        0x8B, 0x0F, // mov cx, [bx]
        0xF4, // hlt
    ];

    fn cpu_with(code: &[u8], seg: u16, off: u16) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.load_flat(seg, off, code);
        cpu.regs.set_seg(Seg::Cs, seg);
        cpu.ip = off;
        cpu
    }

    #[test]
    fn memory_fixture_runs_to_halt() {
        let mut cpu = cpu_with(&MEMORY_FIXTURE, 0xFFFF, 0x0000);
        let result = run(&mut cpu, 1_000_000, false);
        assert!(matches!(result.outcome, Outcome::Halted));
        assert_eq!(result.steps, 7);
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0x1101);
        assert_eq!(cpu.mem.read_word(0x0020), 0x1101);
    }

    #[test]
    fn step_limit_is_reported_before_running() {
        let mut cpu = cpu_with(&MEMORY_FIXTURE, 0xFFFF, 0x0000);
        let result = run(&mut cpu, 0, false);
        assert!(matches!(result.outcome, Outcome::StepLimit));
        assert_eq!(result.steps, 0);
        assert!(!cpu.halted);
    }

    #[test]
    fn step_limit_stops_mid_program() {
        let mut cpu = cpu_with(&[0xB8, 0x01, 0x00, 0xF4], 0xFFFF, 0x0000);
        let result = run(&mut cpu, 1, false);
        assert!(matches!(result.outcome, Outcome::StepLimit));
        assert_eq!(result.steps, 1);
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x0001);
        assert!(!cpu.halted);
    }

    #[test]
    fn unknown_opcode_reports_instruction_address() {
        let mut cpu = cpu_with(&[0x0F], 0xFFFF, 0x0100);
        let result = run(&mut cpu, 100, false);
        assert!(matches!(
            result.outcome,
            Outcome::Error(RunError {
                cs: 0xFFFF,
                ip: 0x0100,
                err: StepError::UnknownOpcode(0x0F),
            })
        ));
    }

    #[test]
    fn unsupported_form_reports_error() {
        let mut cpu = cpu_with(&[0x81, 0xC8, 0x34, 0x12], 0xFFFF, 0x0000);
        let result = run(&mut cpu, 100, false);
        assert!(matches!(
            result.outcome,
            Outcome::Error(RunError {
                cs: 0xFFFF,
                ip: 0x0000,
                err: StepError::UnsupportedForm { .. },
            })
        ));
    }

    #[test]
    fn flag_rendering_marks_set_bits_in_standard_order() {
        assert_eq!(format_flags(Flags::CF | Flags::ZF), ".....Z..C");
        assert_eq!(format_flags(0), ".........");
    }
}
