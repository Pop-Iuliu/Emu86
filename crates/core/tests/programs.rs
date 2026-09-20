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
fn memory_program_hits_checkpoints() {
    let bin = std::fs::read(format!("{PROGRAM_DIR}/memory.bin")).unwrap();
    let mut cpu = Cpu::new();
    cpu.load_flat(0xFFFF, 0x0000, &bin);

    cpu.step().unwrap();
    assert_eq!(cpu.regs.reg(Reg16::Bx), 0x0020);
    cpu.step().unwrap();
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0xBEEF);
    cpu.step().unwrap();
    assert_eq!(cpu.mem.read_word(0x0020), 0xBEEF);
    assert_eq!(cpu.flags.bits(), 0xF002);
    cpu.step().unwrap();
    assert_eq!(cpu.mem.read_word(0x0020), 0xD000);
    assert_eq!(cpu.flags.bits(), 0xF096);
    cpu.step().unwrap();
    assert_eq!(cpu.mem.read_word(0x0020), 0xD001);
    assert_eq!(cpu.flags.bits(), 0xF093);
    cpu.step().unwrap();
    assert_eq!(cpu.regs.reg(Reg16::Cx), 0xD001);
    cpu.step().unwrap();
    assert!(cpu.halted);
    assert_eq!(cpu.ip, 0x0012);
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0xBEEF);
}

#[test]
fn memory_program_uses_planned_encodings() {
    let bin = std::fs::read(format!("{PROGRAM_DIR}/memory.bin")).unwrap();
    assert_eq!(&bin[0..3], &[0xBB, 0x20, 0x00]);
    assert_eq!(&bin[3..6], &[0xB8, 0xEF, 0xBE]);
    assert_eq!(&bin[6..8], &[0x89, 0x07]);
    assert_eq!(&bin[8..12], &[0x81, 0x07, 0x11, 0x11]);
    assert_eq!(&bin[12..15], &[0x83, 0x2F, 0xFF]);
    assert_eq!(&bin[15..17], &[0x8B, 0x0F]);
    assert_eq!(bin[17], 0xF4);
    assert_eq!(bin.len(), 18);
}

#[test]
fn step_error_for_unknown_is_exhaustive() {
    let mut cpu = Cpu::new();
    cpu.load_flat(0xFFFF, 0x0000, &[0x0F]);
    assert_eq!(cpu.step(), Err(StepError::UnknownOpcode(0x0F)));
}
