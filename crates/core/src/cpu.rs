use crate::alu;
use crate::flags::Flags;
use crate::mem::Memory;
use crate::reg::{Reg16, Reg8, RegFile, Seg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Snapshot {
    pub ax: u16,
    pub cx: u16,
    pub dx: u16,
    pub bx: u16,
    pub sp: u16,
    pub bp: u16,
    pub si: u16,
    pub di: u16,
    pub es: u16,
    pub cs: u16,
    pub ss: u16,
    pub ds: u16,
    pub ip: u16,
    pub flags: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepError {
    UnknownOpcode(u8),
    Halted,
}

pub struct Cpu {
    pub regs: RegFile,
    pub ip: u16,
    pub flags: Flags,
    pub mem: Memory,
    pub halted: bool,
}

impl Cpu {
    pub fn new() -> Self {
        let mut cpu = Self {
            regs: RegFile::new(),
            ip: 0x0000,
            flags: Flags::default(),
            mem: Memory::new(),
            halted: false,
        };
        cpu.regs.set_seg(Seg::Cs, 0xFFFF);
        cpu
    }

    pub fn reset(&mut self) {
        *self = Self::new();
    }

    fn fetch8(&mut self) -> u8 {
        let b = self.mem.read(self.linear_ip());
        self.ip = self.ip.wrapping_add(1);
        b
    }

    fn fetch16(&mut self) -> u16 {
        let lo = self.fetch8() as u16;
        (self.fetch8() as u16) << 8 | lo
    }

    pub fn step(&mut self) -> Result<(), StepError> {
        if self.halted {
            return Err(StepError::Halted);
        }
        let op = self.fetch8();
        match op {
            0xB8..=0xBF => {
                let r = Reg16::from_index(op - 0xB8);
                let imm = self.fetch16();
                self.regs.set_reg(r, imm);
            }
            0xB0..=0xB7 => {
                let r = Reg8::from_index(op - 0xB0);
                let imm = self.fetch8();
                self.regs.set_reg8(r, imm);
            }
            0x05 => {
                let imm = self.fetch16();
                let (r, f) = alu::add(self.regs.reg(Reg16::Ax), imm);
                self.regs.set_reg(Reg16::Ax, r);
                self.flags
                    .set_bits((self.flags.bits() & !Flags::SETTABLE) | f);
            }
            0x2D => {
                let imm = self.fetch16();
                let (r, f) = alu::sub(self.regs.reg(Reg16::Ax), imm);
                self.regs.set_reg(Reg16::Ax, r);
                self.flags
                    .set_bits((self.flags.bits() & !Flags::SETTABLE) | f);
            }
            0xF4 => self.halted = true,
            op => return Err(StepError::UnknownOpcode(op)),
        }
        Ok(())
    }

    pub fn linear_ip(&self) -> usize {
        Memory::linear(self.regs.seg(Seg::Cs), self.ip)
    }

    pub fn load_flat(&mut self, seg: u16, off: u16, bytes: &[u8]) {
        self.mem.load(Memory::linear(seg, off), bytes);
    }

    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            ax: self.regs.reg(Reg16::Ax),
            cx: self.regs.reg(Reg16::Cx),
            dx: self.regs.reg(Reg16::Dx),
            bx: self.regs.reg(Reg16::Bx),
            sp: self.regs.reg(Reg16::Sp),
            bp: self.regs.reg(Reg16::Bp),
            si: self.regs.reg(Reg16::Si),
            di: self.regs.reg(Reg16::Di),
            es: self.regs.seg(Seg::Es),
            cs: self.regs.seg(Seg::Cs),
            ss: self.regs.seg(Seg::Ss),
            ds: self.regs.seg(Seg::Ds),
            ip: self.ip,
            flags: self.flags.bits(),
        }
    }
}

impl Default for Cpu {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESET_SEG: u16 = 0xFFFF;
    const RESET_OFF: u16 = 0x0000;

    fn cpu_with(code: &[u8]) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.load_flat(RESET_SEG, RESET_OFF, code);
        cpu
    }

    #[test]
    fn mov_word_immediate() {
        let mut cpu = cpu_with(&[0xB8, 0x34, 0x12]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1234);
        assert_eq!(cpu.ip, RESET_OFF + 3);
    }

    #[test]
    fn mov_word_immediate_all_regs() {
        for i in 0u8..8 {
            let mut cpu = cpu_with(&[0xB8 + i, 0xCD, 0xAB]);
            cpu.step().unwrap();
            assert_eq!(cpu.regs.reg(Reg16::from_index(i)), 0xABCD);
        }
    }

    #[test]
    fn mov_byte_immediate_hits_high_byte_view() {
        let mut cpu = cpu_with(&[0xB7, 0x56]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg8(Reg8::Bh), 0x56);
        assert_eq!(cpu.regs.reg(Reg16::Bx), 0x5600);
        assert_eq!(cpu.ip, RESET_OFF + 2);
    }

    #[test]
    fn consecutive_movs() {
        let mut cpu = cpu_with(&[0xB8, 0x01, 0x00, 0xB9, 0x02, 0x00]);
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x0001);
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0x0002);
        assert_eq!(cpu.ip, RESET_OFF + 6);
    }

    #[test]
    fn mov_does_not_touch_flags() {
        let before = cpu_with(&[0xB8, 0x00, 0x00]).flags.bits();
        let mut cpu = cpu_with(&[0xB8, 0x00, 0x00]);
        cpu.step().unwrap();
        assert_eq!(cpu.flags.bits(), before);
    }

    #[test]
    fn unknown_opcode_is_reported_not_panicked() {
        let mut cpu = cpu_with(&[0x0F]);
        assert_eq!(cpu.step(), Err(StepError::UnknownOpcode(0x0F)));
        assert_eq!(cpu.ip, RESET_OFF + 1);
    }

    #[test]
    fn add_imm_to_ax_sets_flags() {
        let mut cpu = cpu_with(&[0x05, 0x01, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0xFFFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0);
        assert!(cpu.flags.get(Flags::CF));
        assert!(cpu.flags.get(Flags::ZF));
        assert_eq!(cpu.ip, RESET_OFF + 3);
    }

    #[test]
    fn sub_imm_from_ax_reports_borrow() {
        let mut cpu = cpu_with(&[0x2D, 0x01, 0x00]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0xFFFF);
        assert!(cpu.flags.get(Flags::CF));
        assert!(cpu.flags.get(Flags::SF));
        assert!(!cpu.flags.get(Flags::OF));
    }

    #[test]
    fn alu_ops_leave_static_flag_bits_intact() {
        let mut cpu = cpu_with(&[0x05, 0xFF, 0xFF]);
        let static_bits = cpu.flags.bits() & !Flags::SETTABLE;
        cpu.step().unwrap();
        assert_eq!(cpu.flags.bits() & !Flags::SETTABLE, static_bits);
    }

    #[test]
    fn hlt_halts_and_blocks_further_steps() {
        let mut cpu = cpu_with(&[0xF4]);
        cpu.step().unwrap();
        assert!(cpu.halted);
        assert_eq!(cpu.ip, RESET_OFF + 1);
        assert_eq!(cpu.step(), Err(StepError::Halted));
    }

    #[test]
    fn reset_clears_halted() {
        let mut cpu = cpu_with(&[0xF4]);
        cpu.step().unwrap();
        cpu.reset();
        assert!(!cpu.halted);
    }

    #[test]
    fn reset_state_is_repeatable() {
        let mut a = Cpu::new();
        let b = Cpu::new();
        a.regs.set_reg(Reg16::Ax, 0x1234);
        a.regs.set_reg8(Reg8::Bl, 0xFF);
        a.reset();
        assert_eq!(a.snapshot(), b.snapshot());
    }

    #[test]
    fn reset_state_matches_8086() {
        let cpu = Cpu::new();
        let s = cpu.snapshot();
        assert_eq!(s.cs, 0xFFFF);
        assert_eq!(s.ip, 0x0000);
        assert_eq!(s.flags, 0xF002);
        assert_eq!(s.ax, 0);
        assert_eq!(s.ds, 0);
    }

    #[test]
    fn loads_flat_binary_at_cs_ip() {
        const FFFF0: usize = 0xFFFF << 4;
        let mut cpu = Cpu::new();
        cpu.load_flat(0xFFFF, 0x0000, &[0xB8, 0x34, 0x12]);
        assert_eq!(cpu.mem.read(FFFF0), 0xB8);
        assert_eq!(cpu.mem.read(FFFF0 + 1), 0x34);
        assert_eq!(cpu.mem.read(FFFF0 + 2), 0x12);
        assert_eq!(cpu.linear_ip(), FFFF0);
    }
}
