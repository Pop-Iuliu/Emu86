use crate::flags::Flags;
use crate::mem::Memory;
use crate::reg::{Reg16, RegFile, Seg};

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

pub struct Cpu {
    pub regs: RegFile,
    pub ip: u16,
    pub flags: Flags,
    pub mem: Memory,
}

impl Cpu {
    pub fn new() -> Self {
        let mut cpu = Self {
            regs: RegFile::new(),
            ip: 0x0000,
            flags: Flags::default(),
            mem: Memory::new(),
        };
        cpu.regs.set_seg(Seg::Cs, 0xFFFF);
        cpu
    }

    pub fn reset(&mut self) {
        *self = Self::new();
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
    use crate::reg::Reg8;

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
