#![allow(clippy::panic)]

use emu86_core::{Cpu, Reg16, StepError};

const PROGRAM_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/programs");
const MAX_STEPS: usize = 64;

struct Expectation {
    ax: u16,
    flags: u16,
    ip: u16,
}

fn parse_expected(text: &str) -> Expectation {
    let mut ax = None;
    let mut flags = None;
    let mut ip = None;
    for field in text.split_whitespace() {
        let (k, v) = field.split_once('=').expect("expected key=value");
        let v = u16::from_str_radix(v, 16).expect("expected hex value");
        match k {
            "ax" => ax = Some(v),
            "flags" => flags = Some(v),
            "ip" => ip = Some(v),
            _ => panic!("unknown field {k}"),
        }
    }
    Expectation {
        ax: ax.expect("missing ax"),
        flags: flags.expect("missing flags"),
        ip: ip.expect("missing ip"),
    }
}

fn run_program(bin: &[u8]) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.load_flat(0xFFFF, 0x0000, bin);
    for _ in 0..MAX_STEPS {
        if cpu.halted {
            return cpu;
        }
        cpu.step().expect("program step failed");
    }
    panic!("program did not halt within {MAX_STEPS} steps");
}

#[test]
fn programs_match_expected_results() {
    for name in ["arith", "carry", "overflow", "underflow", "wrap", "memory"] {
        let bin = std::fs::read(format!("{PROGRAM_DIR}/{name}.bin")).unwrap();
        let expected = std::fs::read_to_string(format!("{PROGRAM_DIR}/{name}.expected")).unwrap();
        let e = parse_expected(&expected);

        let cpu = run_program(&bin);
        let s = cpu.snapshot();
        assert_eq!(s.ax, e.ax, "{name}: ax mismatch");
        assert_eq!(s.flags, e.flags, "{name}: flags mismatch");
        assert_eq!(s.ip, e.ip, "{name}: ip mismatch");
        assert!(cpu.halted, "{name}: cpu must be halted");
    }
}

#[test]
fn wrap_program_fetches_across_one_mebibyte_boundary() {
    let bin = std::fs::read(format!("{PROGRAM_DIR}/wrap.bin")).unwrap();
    let cpu = run_program(&bin);
    assert_eq!(cpu.mem.read(0xFFFFF), 0x34);
    assert_eq!(cpu.mem.read(0x00000), 0x12);
    assert_eq!(cpu.mem.read(0x00001), 0xF4);
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1234);
}

#[test]
fn replay_produces_identical_results() {
    for name in ["arith", "carry", "overflow", "underflow", "wrap", "memory"] {
        let bin = std::fs::read(format!("{PROGRAM_DIR}/{name}.bin")).unwrap();

        let mut cpu = Cpu::new();
        cpu.load_flat(0xFFFF, 0x0000, &bin);
        while !cpu.halted {
            cpu.step().unwrap();
        }
        let first = cpu.snapshot();

        cpu.reset();
        cpu.load_flat(0xFFFF, 0x0000, &bin);
        while !cpu.halted {
            cpu.step().unwrap();
        }
        let second = cpu.snapshot();

        assert_eq!(first, second, "{name}: replay diverged");
    }
}

#[test]
fn memory_program_roundtrips_through_ram() {
    let bin = std::fs::read(format!("{PROGRAM_DIR}/memory.bin")).unwrap();
    let cpu = run_program(&bin);
    assert_eq!(cpu.regs.reg(Reg16::Cx), 0x1101);
    assert_eq!(cpu.mem.read_word(0x0020), 0x1101);
}

#[test]
fn step_error_for_unknown_is_exhaustive() {
    let mut cpu = Cpu::new();
    cpu.load_flat(0xFFFF, 0x0000, &[0x0F]);
    assert_eq!(cpu.step(), Err(StepError::UnknownOpcode(0x0F)));
}
