use crate::alu;
use crate::flags::Flags;
use crate::mem::Memory;
use crate::modrm::Operand;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepError {
    UnknownOpcode(u8),
    UnsupportedForm { ip: u16, bytes: Vec<u8> },
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

    fn push_word(&mut self, v: u16) {
        let sp = self.regs.reg(Reg16::Sp).wrapping_sub(2);
        self.regs.set_reg(Reg16::Sp, sp);
        let addr = Memory::linear(self.regs.seg(Seg::Ss), sp);
        self.mem.write_word(addr, v);
    }

    fn pop_word(&mut self) -> u16 {
        let sp = self.regs.reg(Reg16::Sp);
        let addr = Memory::linear(self.regs.seg(Seg::Ss), sp);
        let v = self.mem.read_word(addr);
        self.regs.set_reg(Reg16::Sp, sp.wrapping_add(2));
        v
    }

    pub fn step(&mut self) -> Result<(), StepError> {
        if self.halted {
            return Err(StepError::Halted);
        }
        let start_ip = self.ip;
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
            // Base ALU groups 0x00/08/10/18/20/28/30/38, forms x2/x3:
            // bit 1 = direction (1 = the register field receives), bit 0 =
            // width. Bytes x4..x7 fall through to the accumulator family.
            op @ 0x00..=0x3B if op & 0b100 == 0 => self.alu_rm_reg(op)?,
            // Accumulator-immediate family `04`/`05` … `3C`/`3D`: bit 1 =
            // width, bits 5:3 = the group-1 field, destination always AL/AX.
            op @ 0x00..=0x3F if op & 0b110 == 0b100 => self.alu_acc_imm(op)?,
            0x40..=0x4F => self.inc_dec_r16(op),
            0x80 | 0x81 | 0x83 => self.alu_rm_imm(op)?,
            0x88..=0x8B => self.mov_rm_reg(op),
            0x50..=0x57 => {
                let r = Reg16::from_index(op - 0x50);
                let v = if r == Reg16::Sp {
                    self.regs.reg(Reg16::Sp).wrapping_sub(2)
                } else {
                    self.regs.reg(r)
                };
                self.push_word(v);
            }
            0x58..=0x5F => {
                let r = Reg16::from_index(op - 0x58);
                let v = self.pop_word();
                self.regs.set_reg(r, v);
            }
            0x06 => {
                let v = self.regs.seg(Seg::Es);
                self.push_word(v);
            }
            0x07 => {
                let v = self.pop_word();
                self.regs.set_seg(Seg::Es, v);
            }
            0x0E => {
                let v = self.regs.seg(Seg::Cs);
                self.push_word(v);
            }
            0x0F => {
                let v = self.pop_word();
                self.regs.set_seg(Seg::Cs, v);
            }
            0x16 => {
                let v = self.regs.seg(Seg::Ss);
                self.push_word(v);
            }
            0x17 => {
                let v = self.pop_word();
                self.regs.set_seg(Seg::Ss, v);
            }
            0x1E => {
                let v = self.regs.seg(Seg::Ds);
                self.push_word(v);
            }
            0x1F => {
                let v = self.pop_word();
                self.regs.set_seg(Seg::Ds, v);
            }
            0x9C => {
                let v = self.flags.bits();
                self.push_word(v);
            }
            0x9D => {
                let v = self.pop_word();
                self.flags.set_bits(v);
            }
            0xE8 => {
                let disp = self.fetch16();
                let ret_off = self.ip;
                let target = self.ip.wrapping_add(disp);
                self.push_word(ret_off);
                self.ip = target;
            }
            0xC2 => {
                let imm = self.fetch16();
                let ret_off = self.pop_word();
                self.ip = ret_off;
                self.regs
                    .set_reg(Reg16::Sp, self.regs.reg(Reg16::Sp).wrapping_add(imm));
            }
            0xC3 => {
                let ret_off = self.pop_word();
                self.ip = ret_off;
            }
            0x70..=0x7F => self.jcc(op),
            0xE0..=0xE2 => self.loop_family(op),
            0xE9 => {
                let disp = self.fetch16();
                self.ip = self.ip.wrapping_add(disp);
            }
            0xEB => {
                let disp = self.fetch8() as i8 as u16;
                self.ip = self.ip.wrapping_add(disp);
            }
            0xFE | 0xFF => self.inc_dec_rm(op, start_ip)?,
            0xF4 => self.halted = true,
            op => return Err(StepError::UnknownOpcode(op)),
        }
        Ok(())
    }

    /// `80`/`81`/`83 /0`–`/7`: ADD, OR, ADC, SBB, AND, SUB, XOR, CMP.
    ///
    /// `80` is the byte form (raw `imm8`), `81` is the word form (`imm16`),
    /// and `83` is the word form with a sign-extended `imm8`. CF is read
    /// before the resulting flags are written back, so `adc`/`sbb` consume
    /// the previous instruction's carry. `/7` (CMP) computes SUB's flags
    /// without writing the destination.
    fn alu_rm_imm(&mut self, op: u8) -> Result<(), StepError> {
        let modrm = self.fetch8();
        let alu_op = (modrm >> 3) & 0b111;
        // 0x80 is the byte form; 0x81/0x83 are word.
        let wide = op != 0x80;
        let dst = self.decode_rm(modrm, wide);
        let imm = match op {
            0x81 => self.fetch16(),
            // 0x83's immediate is sign-extended; 0x80's is a raw byte.
            0x83 => self.fetch8() as i8 as u16,
            _ => u16::from(self.fetch8()),
        };
        // Read the carry-in before the result's flags are written back, so
        // ADC/SBB see the CF produced by the previous instruction.
        let carry = u16::from(self.flags.get(Flags::CF));
        let a = dst.read(&self.regs, &self.mem);
        let (r, f) = Self::alu_binary(alu_op, wide, a, imm, carry);
        if alu_op != 7 {
            dst.write(&mut self.regs, &mut self.mem, r);
        }
        self.set_flag_bits(f);
        Ok(())
    }

    /// The one group-1 dispatch table: `alu_op` is always `(opcode >> 3) & 7`
    /// (or the ModRM `/N` field), `a` is the destination value and `b` the
    /// source. `carry` is read from CF before any flag write-back by the
    /// caller. The `/N` index routes to exactly the same helpers as
    /// `alu_rm_imm` — no arithmetic here computes flags a second way.
    fn alu_binary(alu_op: u8, wide: bool, a: u16, b: u16, carry: u16) -> (u16, u16) {
        if wide {
            match alu_op {
                0 => alu::add(a, b),
                1 => alu::or(a, b),
                2 => alu::adc(a, b, carry),
                3 => alu::sbb(a, b, carry),
                4 => alu::and(a, b),
                5 => alu::sub(a, b),
                6 => alu::xor(a, b),
                _ => alu::sub(a, b), // /7 = CMP: flags only
            }
        } else {
            // `Operand::read` zero-extends, so narrow back to u8 before the
            // byte-width flag maths runs.
            let (r, f) = match alu_op {
                0 => alu::add8(a as u8, b as u8),
                1 => alu::or8(a as u8, b as u8),
                2 => alu::adc8(a as u8, b as u8, carry as u8),
                3 => alu::sbb8(a as u8, b as u8, carry as u8),
                4 => alu::and8(a as u8, b as u8),
                5 => alu::sub8(a as u8, b as u8),
                6 => alu::xor8(a as u8, b as u8),
                _ => alu::sub8(a as u8, b as u8), // /7 = CMP: flags only
            };
            (u16::from(r), f)
        }
    }

    /// `00`–`3B` minus the accumulator slots: ADD/OR/ADC/SBB/AND/SUB/XOR/CMP
    /// between a register and a register or memory operand. The opcode itself
    /// carries the group-1 field (`(op >> 3) & 7`), the direction
    /// (bit 1 set = the register field is the destination) and the width
    /// (bit 0). CMP (`/7`) keeps its write-to-nothing rule.
    fn alu_rm_reg(&mut self, op: u8) -> Result<(), StepError> {
        let modrm = self.fetch8();
        let alu_op = (op >> 3) & 0b111;
        let wide = op & 1 == 1;
        let reg_receives = op & 2 == 2;
        let rm = self.decode_rm(modrm, wide);
        let reg = Operand::decode_reg(modrm, wide);
        let (dst, src) = if reg_receives { (reg, rm) } else { (rm, reg) };
        // Read the carry-in before the result's flags are written back, so
        // ADC/SBB see the CF produced by the previous instruction.
        let carry = u16::from(self.flags.get(Flags::CF));
        let a = dst.read(&self.regs, &self.mem);
        let b = src.read(&self.regs, &self.mem);
        let (r, f) = Self::alu_binary(alu_op, wide, a, b, carry);
        if alu_op != 7 {
            dst.write(&mut self.regs, &mut self.mem, r);
        }
        self.set_flag_bits(f);
        Ok(())
    }

    /// `04`–`3D` accumulator family: the eight group-1 fields with AL or AX
    /// as the fixed destination and one immediate. Bits 5:3 select the
    /// operation, bit 0 selects AL/AX; the byte immediate is a raw byte
    /// (`04`–`3C`) and the word immediate is full `imm16` (`05`–`3D`).
    /// CMP (```/7``) computes flags without touching the accumulator.
    fn alu_acc_imm(&mut self, op: u8) -> Result<(), StepError> {
        let alu_op = (op >> 3) & 0b111;
        let wide = op & 1 == 1;
        // Read the carry-in before the result's flags are written back, so
        // ADC/SBB see the CF produced by the previous instruction.
        let carry = u16::from(self.flags.get(Flags::CF));
        let (acc, imm) = if wide {
            (self.regs.reg(Reg16::Ax), self.fetch16())
        } else {
            (
                u16::from(self.regs.reg8(Reg8::Al)),
                u16::from(self.fetch8()),
            )
        };
        let (r, f) = Self::alu_binary(alu_op, wide, acc, imm, carry);
        if wide {
            if alu_op != 7 {
                self.regs.set_reg(Reg16::Ax, r);
            }
        } else if alu_op != 7 {
            self.regs.set_reg8(Reg8::Al, r as u8);
        }
        self.set_flag_bits(f);
        Ok(())
    }

    /// `40`–`47` INC r16, `48`–`4F` DEC r16: everything but CF is the
    /// `add`/`sub`-by-one flag machine.
    fn inc_dec_r16(&mut self, op: u8) {
        let r = Reg16::from_index(op);
        let value = self.regs.reg(r);
        let (next, f) = if op < 0x48 {
            alu::inc(value)
        } else {
            alu::dec(value)
        };
        self.regs.set_reg(r, next);
        self.set_flag_bits_preserving_carry(f);
    }

    /// `FE /0 /1` and `FF /0 /1`: INC/DEC r/m8 and INC/DEC r/m16 through
    /// the operand decoder. The remaining `/N` fields of `FE`/`FF` are
    /// reported as unsupported forms.
    fn inc_dec_rm(&mut self, op: u8, start: u16) -> Result<(), StepError> {
        let modrm = self.fetch8();
        let field = (modrm >> 3) & 0b111;
        let wide = op == 0xFF;
        if field > 1 {
            return Err(StepError::UnsupportedForm {
                ip: start,
                bytes: self.consumed_bytes(start),
            });
        }
        let dst = self.decode_rm(modrm, wide);
        let value = dst.read(&self.regs, &self.mem);
        let (next, f) = match (field, wide) {
            (0, true) => alu::inc(value),
            (0, false) => {
                let (r, f) = alu::inc8(value as u8);
                (u16::from(r), f)
            }
            (1, true) => alu::dec(value),
            _ => {
                let (r, f) = alu::dec8(value as u8);
                (u16::from(r), f)
            }
        };
        dst.write(&mut self.regs, &mut self.mem, next);
        self.set_flag_bits_preserving_carry(f);
        Ok(())
    }

    /// `70`–`7F`: the full short-jump conditional set; the opcode's low
    /// nibble indexes the condition in hardware order. Every jcc consumes
    /// exactly the disp8 whether or not it is taken.
    fn jcc(&mut self, op: u8) {
        let disp = self.fetch8() as i8 as u16;
        if self.condition(op & 0xF) {
            self.ip = self.ip.wrapping_add(disp);
        }
    }

    fn condition(&self, index: u8) -> bool {
        let of = self.flags.get(Flags::OF);
        let cf = self.flags.get(Flags::CF);
        let zf = self.flags.get(Flags::ZF);
        let sf = self.flags.get(Flags::SF);
        let pf = self.flags.get(Flags::PF);
        match index {
            0 => of,
            1 => !of,
            2 => cf,
            3 => !cf,
            4 => zf,
            5 => !zf,
            6 => cf || zf,
            7 => !cf && !zf,
            8 => sf,
            9 => !sf,
            10 => pf,
            11 => !pf,
            12 => sf != of,
            13 => sf == of,
            14 => zf || sf != of,
            _ => !zf && sf == of,
        }
    }

    /// `E0`/`E1`/`E2`: LOOPNE/LOOPE/LOOP. CX is decremented with no flag
    /// write of any kind; the re-entry conditions layer `CX != 0` with the
    /// ZF test for the E1/E0 variants. A CX that starts at 0 wraps to FFFF
    /// and takes the branch.
    fn loop_family(&mut self, op: u8) {
        let disp = self.fetch8() as i8 as u16;
        let cx = self.regs.reg(Reg16::Cx).wrapping_sub(1);
        self.regs.set_reg(Reg16::Cx, cx);
        let take = match op {
            0xE0 => cx != 0 && !self.flags.get(Flags::ZF),
            0xE1 => cx != 0 && self.flags.get(Flags::ZF),
            _ => cx != 0,
        };
        if take {
            self.ip = self.ip.wrapping_add(disp);
        }
    }

    fn consumed_bytes(&self, start: u16) -> Vec<u8> {
        let mut off = start;
        let mut out = Vec::new();
        while off != self.ip && out.len() < 16 {
            out.push(self.mem.read(Memory::linear(self.regs.seg(Seg::Cs), off)));
            off = off.wrapping_add(1);
        }
        out
    }

    fn mov_rm_reg(&mut self, op: u8) {
        let modrm = self.fetch8();
        let wide = op & 1 == 1;
        let reg_is_src = op & 2 == 0;
        let rm = self.decode_rm(modrm, wide);
        let reg = Operand::decode_reg(modrm, wide);
        let (src, dst) = if reg_is_src { (reg, rm) } else { (rm, reg) };
        let v = src.read(&self.regs, &self.mem);
        dst.write(&mut self.regs, &mut self.mem, v);
    }

    fn decode_rm(&mut self, modrm: u8, wide: bool) -> Operand {
        let regs = self.regs;
        let mut fetch = || self.fetch8();
        Operand::decode_rm(&mut fetch, regs, modrm, wide)
    }

    fn set_flag_bits(&mut self, f: u16) {
        self.flags
            .set_bits((self.flags.bits() & !Flags::SETTABLE) | f);
    }

    /// Flag write-back for INC/DEC: the helper's word arrives with CF
    /// cleared, so OR-ing the incoming CF bit back preserves it while the
    /// remaining arithmetic flags come from the helper.
    fn set_flag_bits_preserving_carry(&mut self, f: u16) {
        let carry = u16::from(self.flags.get(Flags::CF));
        self.set_flag_bits(f | carry);
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

    fn loaded_to_halt(code: &[u8]) -> Cpu {
        let mut cpu = cpu_with(code);
        while !cpu.halted {
            cpu.step().unwrap();
        }
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
        // 0x60 is not an 8086 opcode; 0x0F decodes as POP CS.
        let mut cpu = cpu_with(&[0x60]);
        assert_eq!(cpu.step(), Err(StepError::UnknownOpcode(0x60)));
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
    fn cmp_ax_imm_sets_flags_and_keeps_ax() {
        let mut cpu = cpu_with(&[0x3D, 0x01, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 1);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 1);
        assert!(cpu.flags.get(Flags::ZF));
        assert_eq!(cpu.ip, RESET_OFF + 3);
    }

    #[test]
    fn cmp_ax_imm_reports_borrow() {
        let mut cpu = cpu_with(&[0x3D, 0x01, 0x00]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0);
        assert!(cpu.flags.get(Flags::CF));
        assert!(cpu.flags.get(Flags::SF));
        assert!(!cpu.flags.get(Flags::ZF));
    }

    #[test]
    fn cmp_rm16_imm16_keeps_operand() {
        let mut cpu = cpu_with(&[0x81, 0xF9, 0x01, 0x00]);
        cpu.regs.set_reg(Reg16::Cx, 1);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Cx), 1);
        assert!(cpu.flags.get(Flags::ZF));
        assert_eq!(cpu.ip, RESET_OFF + 4);
    }

    #[test]
    fn cmp_rm16_imm16_memory_form_reuses_decoder() {
        let mut cpu = cpu_with(&[0x81, 0x3F, 0x34, 0x12]);
        cpu.regs.set_reg(Reg16::Bx, 0x0010);
        cpu.mem.write_word(0x0010, 0x1234);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_word(0x0010), 0x1234);
        assert!(cpu.flags.get(Flags::ZF));
    }

    #[test]
    fn cmp_rm16_imm8_sign_extends_immediate() {
        let mut cpu = cpu_with(&[0x83, 0xFF, 0xFF]);
        cpu.regs.set_reg(Reg16::Di, 0x0100);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Di), 0x0100);
        assert!(cpu.flags.get(Flags::CF));
    }

    #[test]
    fn cmp_rm16_imm8_positive_compares_directly() {
        let mut cpu = cpu_with(&[0x83, 0xFF, 0x01]);
        cpu.regs.set_reg(Reg16::Di, 1);
        cpu.step().unwrap();
        assert!(cpu.flags.get(Flags::ZF));
        assert!(!cpu.flags.get(Flags::CF));
    }

    #[test]
    fn jmp_short_moves_relative_both_ways() {
        let mut cpu = cpu_with(&[0xEB, 0x02, 0x90, 0x90, 0xEB, 0xFC]);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 4);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 2);
    }

    #[test]
    fn jmp_short_disp8_boundaries() {
        let mut cpu = cpu_with(&[0xEB, 0x7F]);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 2 + 0x7F);
        let mut cpu = cpu_with(&[0xEB, 0x80]);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0xFF82);
    }

    #[test]
    fn jmp_near_uses_disp16() {
        let mut cpu = cpu_with(&[0xE9, 0x34, 0x12]);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0x1237);
    }

    #[test]
    fn jump_target_wraps_ip_16bit() {
        let mut cpu = Cpu::new();
        cpu.regs.set_seg(Seg::Cs, 0);
        cpu.mem.load(0xFFF0, &[0xE9, 0xFF, 0x7F]);
        cpu.ip = 0xFFF0;
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0x7FF2);
    }

    #[test]
    fn je_taken_when_zf_set() {
        let mut cpu = cpu_with(&[0x3D, 0x00, 0x00, 0x74, 0x02, 0x90]);
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 7);
    }

    #[test]
    fn je_untaken_when_zf_clear() {
        let mut cpu = cpu_with(&[0xB8, 0x01, 0x00, 0x3D, 0x00, 0x00, 0x74, 0x02]);
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert!(!cpu.flags.get(Flags::ZF));
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 8);
    }

    #[test]
    fn jne_taken_when_zf_clear() {
        let mut cpu = cpu_with(&[0xB8, 0x01, 0x00, 0x3D, 0x00, 0x00, 0x75, 0x02]);
        cpu.step().unwrap();
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 10);
    }

    #[test]
    fn jne_untaken_when_zf_set() {
        let mut cpu = cpu_with(&[0x3D, 0x00, 0x00, 0x75, 0x02]);
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 5);
    }

    #[test]
    fn jumps_leave_flags_unchanged() {
        let mut cpu = cpu_with(&[0x3D, 0x01, 0x00, 0xEB, 0x00, 0x74, 0x00, 0x75, 0x00]);
        cpu.step().unwrap();
        let before = cpu.flags.bits();
        cpu.step().unwrap();
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert_eq!(cpu.ip, RESET_OFF + 9);
        assert_eq!(cpu.flags.bits(), before);
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

    #[test]
    fn memory_program_store_modify_load_halt() {
        let mut cpu = cpu_with(&[
            0xBB, 0x20, 0x00, 0xB8, 0x00, 0x01, 0x89, 0x07, 0x81, 0x07, 0x00, 0x10, 0x83, 0x2F,
            0xFF, 0x8B, 0x0F, 0xF4,
        ]);
        while !cpu.halted {
            cpu.step().unwrap();
        }
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0x1101);
        assert_eq!(cpu.mem.read_word(0x0020), 0x1101);
        assert_eq!(cpu.ip, RESET_OFF + 18);
    }

    #[test]
    fn byte_mov_moves_between_low_and_high_regs() {
        let mut cpu = cpu_with(&[0xB5, 0x11, 0x88, 0xE9, 0x8A, 0xE1]);
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg8(Reg8::Cl), 0x11);
        assert_eq!(cpu.regs.reg8(Reg8::Ch), 0x11);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg8(Reg8::Ah), 0x11);
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0x1111);
    }

    #[test]
    fn word_mov_loads_direct_address_from_ds() {
        let mut cpu = cpu_with(&[0x8B, 0x1E, 0x00, 0x10]);
        cpu.regs.set_seg(Seg::Ds, 0x1000);
        cpu.mem.write_word(0x11000, 0xBEEF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Bx), 0xBEEF);
    }

    #[test]
    fn bp_addressing_uses_ss() {
        let mut cpu = cpu_with(&[0x8A, 0x7E, 0x01]);
        cpu.regs.set_seg(Seg::Ss, 0x2000);
        cpu.regs.set_seg(Seg::Ds, 0x1000);
        cpu.regs.set_reg(Reg16::Bp, 0x0005);
        cpu.mem.write(0x20006, 0xAB);
        cpu.mem.write(0x10006, 0xCD);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg8(Reg8::Bh), 0xAB);
    }

    #[test]
    fn word_mov_loads_with_disp16() {
        let mut cpu = cpu_with(&[0x8B, 0x8E, 0x34, 0x12]);
        cpu.regs.set_reg(Reg16::Bp, 0x0100);
        cpu.regs.set_seg(Seg::Ss, 0x1000);
        cpu.mem.write_word(0x11334, 0x5A5A);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0x5A5A);
    }

    #[test]
    fn effective_addresses_wrap_at_16_bits() {
        let mut cpu = cpu_with(&[0x89, 0x47, 0x01]);
        cpu.regs.set_reg(Reg16::Bx, 0xFFFF);
        cpu.regs.set_reg(Reg16::Ax, 0x1234);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_word(0x00000), 0x1234);
    }

    #[test]
    fn memory_mov_does_not_touch_flags() {
        let mut cpu = cpu_with(&[0x05, 0xFF, 0xFF, 0x89, 0x07, 0x8B, 0x0F]);
        cpu.regs.set_reg(Reg16::Bx, 0x0040);
        cpu.step().unwrap();
        let before = cpu.flags.bits();
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert_eq!(cpu.flags.bits(), before);
    }

    #[test]
    fn add_byte_imm_to_memory_uses_byte_flags() {
        // 80 /0 = ADD r/m8, imm8
        let mut cpu = cpu_with(&[0x80, 0x07, 0x01]); // add byte [bx], 1
        cpu.regs.set_reg(Reg16::Bx, 0x0010);
        cpu.mem.write(0x0010, 0xFF);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read(0x0010), 0x00);
        assert!(cpu.flags.get(Flags::CF));
        assert!(cpu.flags.get(Flags::ZF));
        assert!(!cpu.flags.get(Flags::SF));
        assert_eq!(cpu.ip, RESET_OFF + 3);
    }

    #[test]
    fn sub_byte_imm_sets_sf_from_bit_seven() {
        // 0x00 - 1 = 0xFF is negative at byte width. The word-width flag maths
        // this used to run would have reported SF clear here.
        let mut cpu = cpu_with(&[0x80, 0x2F, 0x01]); // sub byte [bx], 1
        cpu.regs.set_reg(Reg16::Bx, 0x0010);
        cpu.mem.write(0x0010, 0x00);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read(0x0010), 0xFF);
        assert!(cpu.flags.get(Flags::CF));
        assert!(cpu.flags.get(Flags::SF));
        assert_eq!(cpu.flags.bits(), 0xF097);
    }

    #[test]
    fn add_byte_imm_to_high_byte_register_preserves_low_byte() {
        // mod=11, r/m=111 -> BH.
        let mut cpu = cpu_with(&[0x80, 0xC7, 0x01]); // add bh, 1
        cpu.regs.set_reg(Reg16::Bx, 0x1200);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg8(Reg8::Bh), 0x13);
        assert_eq!(cpu.regs.reg8(Reg8::Bl), 0x00);
    }

    #[test]
    fn cmp_byte_imm_keeps_the_operand() {
        // 80 /7 = CMP r/m8, imm8
        let mut cpu = cpu_with(&[0x80, 0x3F, 0x05]); // cmp byte [bx], 5
        cpu.regs.set_reg(Reg16::Bx, 0x0010);
        cpu.mem.write(0x0010, 0x05);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read(0x0010), 0x05);
        assert!(cpu.flags.get(Flags::ZF));
    }

    #[test]
    fn byte_alu_imm_immediate_is_a_raw_byte() {
        // 0x80's immediate is a plain byte; 0x83's is the one that sign-extends.
        // 0x02 + 0xFF wraps to 0x01 with a carry out of bit 7.
        let mut cpu = cpu_with(&[0x80, 0xC3, 0xFF]); // add bl, 0xFF
        cpu.regs.set_reg(Reg16::Bx, 0x0002);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg8(Reg8::Bl), 0x01);
        assert!(cpu.flags.get(Flags::CF));
        assert!(!cpu.flags.get(Flags::SF));
    }

    #[test]
    fn byte_alu_leaves_static_flag_bits_intact() {
        let mut cpu = cpu_with(&[0x80, 0x07, 0x01]);
        cpu.regs.set_reg(Reg16::Bx, 0x0010);
        cpu.mem.write(0x0010, 0xFF);
        cpu.step().unwrap();
        assert_eq!(cpu.flags.bits() & !Flags::SETTABLE, 0xF002);
    }

    // -----------------------------------------------------------------------
    // Group 1 through `Cpu::step`: prove the decode in `alu_rm_imm` routes each
    // of the eight `match alu_op` arms to its own helper. `alu.rs` covers the
    // helpers in isolation, but a swap of two arms here would not fail those
    // unit tests. Fields `/0`, `/5`, `/7` are already covered by the earlier
    // slice; this section closes `/1`, `/2`, `/3`, `/4`, `/6` at both widths
    // and pins the `83 /1` sign-extension path.
    // -----------------------------------------------------------------------

    #[test]
    fn group1_word_or_through_step() {
        // 81 /1 -> OR AX, imm16. 0x00FF | 0x0001 = 0x00FF; ADD would give 0x0100.
        let mut cpu = cpu_with(&[0x81, 0xC8, 0x01, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0x00FF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x00FF);
        // PF set (0xFF has even parity); CF/AF/OF cleared by the logic result.
        assert_eq!(cpu.flags.bits(), 0xF006);
    }

    #[test]
    fn group1_word_adc_consumes_carry_through_step() {
        // 81 /2 -> ADC AX, imm16 with CF = 1: 0 + 0 + 1 = 1. ADD would leave 0.
        let mut cpu = cpu_with(&[0x81, 0xD0, 0x00, 0x00]);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x0001);
        assert_eq!(cpu.flags.bits(), 0xF002);
    }

    #[test]
    fn group1_word_sbb_consumes_borrow_through_step() {
        // 81 /3 -> SBB AX, imm16 with CF = 1: 1 - 0 - 1 = 0. SUB would leave 1.
        let mut cpu = cpu_with(&[0x81, 0xD8, 0x00, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0x0001);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x0000);
        // ZF | PF (result 0 is even parity).
        assert_eq!(cpu.flags.bits(), 0xF046);
    }

    #[test]
    fn group1_word_and_through_step() {
        // 81 /4 -> AND AX, imm16. 0xFFFF & 0x00FF = 0x00FF; OR would keep 0xFFFF.
        let mut cpu = cpu_with(&[0x81, 0xE0, 0xFF, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0xFFFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x00FF);
        assert_eq!(cpu.flags.bits(), 0xF006);
    }

    #[test]
    fn group1_word_xor_through_step() {
        // 81 /6 -> XOR AX, imm16. 0xA5A5 ^ 0xFFFF = 0x5A5A; AND would keep 0xA5A5.
        let mut cpu = cpu_with(&[0x81, 0xF0, 0xFF, 0xFF]);
        cpu.regs.set_reg(Reg16::Ax, 0xA5A5);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x5A5A);
        assert_eq!(cpu.flags.bits(), 0xF006);
    }

    #[test]
    fn group1_byte_or_through_step() {
        // 80 /1 -> OR AL, imm8. 0xF0 | 0xFF = 0xFF; AH (0x12) must survive.
        let mut cpu = cpu_with(&[0x80, 0xC8, 0xFF]);
        cpu.regs.set_reg(Reg16::Ax, 0x12F0);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x12FF);
        // PF (even parity of 0xFF) | SF (bit 7 of 0xFF is set at byte width).
        assert_eq!(cpu.flags.bits(), 0xF086);
    }

    #[test]
    fn group1_byte_adc_consumes_carry_through_step() {
        // 80 /2 -> ADC AL, imm8 with CF = 1: 0 + 0 + 1 = 1.
        let mut cpu = cpu_with(&[0x80, 0xD0, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0x1200);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1201);
        assert_eq!(cpu.flags.bits(), 0xF002);
    }

    #[test]
    fn group1_byte_sbb_consumes_borrow_through_step() {
        // 80 /3 -> SBB AL, imm8 with CF = 1: 1 - 0 - 1 = 0.
        let mut cpu = cpu_with(&[0x80, 0xD8, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0x1201);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1200);
        assert_eq!(cpu.flags.bits(), 0xF046);
    }

    #[test]
    fn group1_byte_and_through_step() {
        // 80 /4 -> AND AL, imm8. 0xFF & 0x0F = 0x0F.
        let mut cpu = cpu_with(&[0x80, 0xE0, 0x0F]);
        cpu.regs.set_reg(Reg16::Ax, 0x12FF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x120F);
        assert_eq!(cpu.flags.bits(), 0xF006);
    }

    #[test]
    fn group1_byte_xor_through_step() {
        // 80 /6 -> XOR AL, imm8. 0xAA ^ 0xFF = 0x55.
        let mut cpu = cpu_with(&[0x80, 0xF0, 0xFF]);
        cpu.regs.set_reg(Reg16::Ax, 0x12AA);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1255);
        assert_eq!(cpu.flags.bits(), 0xF006);
    }

    #[test]
    fn group1_word_or_imm8_sign_extends() {
        // 83 /1 -> OR AX, imm8. imm8 = 0xFF sign-extends to 0xFFFF, so the
        // result is 0xFFFF. A raw byte immediate (0x00FF) would give 0x00FF.
        // Also refutes the CHANGELOG line that used to omit `83 /1`.
        let mut cpu = cpu_with(&[0x83, 0xC8, 0xFF]);
        cpu.regs.set_reg(Reg16::Ax, 0x00F0);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0xFFFF);
        // PF (0xFF parity) | SF (bit 15 of 0xFFFF).
        assert_eq!(cpu.flags.bits(), 0xF086);
    }

    // -----------------------------------------------------------------------
    // The operand-carrier ALU families. `alu_binary` is the one group-1
    // table: the sweeps below route every opcode byte through `step` with
    // one (dst, src) pair — 0xF8/0x09 at both widths — and compare against
    // the same helpers the immediate path uses. CF is pre-set on every row
    // so ADC/SBB cannot pass for ADD/SUB anywhere. The pair is chosen so
    // all eight fields land on distinct results at both widths.
    // -----------------------------------------------------------------------

    const SWEEP_A16: u16 = 0x00F8;
    const SWEEP_B16: u16 = 0x0009;

    #[test]
    fn rm_reg_word_routes_all_eight_fields_both_directions() {
        for alu_op in 0u8..8 {
            let expected = Cpu::alu_binary(alu_op, true, SWEEP_A16, SWEEP_B16, 1);
            // 01/09/...|3: ADD Ev,Gv — r/m (AX) receives op(AX, CX).
            let mut cpu = cpu_with(&[(0x01 | (alu_op << 3)), 0b11_001_000, 0xF4]);
            cpu.regs.set_reg(Reg16::Ax, SWEEP_A16);
            cpu.regs.set_reg(Reg16::Cx, SWEEP_B16);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            assert_eq!(cpu.ip, RESET_OFF + 2, "d0 field {alu_op}");
            if alu_op != 7 {
                assert_eq!(cpu.regs.reg(Reg16::Ax), expected.0, "d0 field {alu_op}");
            } else {
                assert_eq!(cpu.regs.reg(Reg16::Ax), SWEEP_A16, "cmp d0 {alu_op}");
            }
            assert_eq!(cpu.regs.reg(Reg16::Cx), SWEEP_B16, "d0 field {alu_op}");
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "d0 field {alu_op}");

            // 03/0B/...|1: ADD Gv,Ev — the reg field (CX) receives op(CX, AX).
            let mut cpu = cpu_with(&[(0x03 | (alu_op << 3)), 0b11_001_000, 0xF4]);
            cpu.regs.set_reg(Reg16::Cx, SWEEP_A16);
            cpu.regs.set_reg(Reg16::Ax, SWEEP_B16);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            assert_eq!(cpu.ip, RESET_OFF + 2, "d1 field {alu_op}");
            if alu_op != 7 {
                assert_eq!(cpu.regs.reg(Reg16::Cx), expected.0, "d1 field {alu_op}");
            } else {
                assert_eq!(cpu.regs.reg(Reg16::Cx), SWEEP_A16, "cmp d1 {alu_op}");
            }
            assert_eq!(cpu.regs.reg(Reg16::Ax), SWEEP_B16, "d1 field {alu_op}");
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "d1 field {alu_op}");
        }
    }

    #[test]
    fn rm_reg_byte_routes_all_eight_fields_both_directions() {
        for alu_op in 0u8..8 {
            let expected = Cpu::alu_binary(alu_op, false, 0xF8, 0x09, 1);
            // 00/08/...|0: ADD Gb,Gb with d = 0 — AL receives op(AL, CL).
            let mut cpu = cpu_with(&[alu_op << 3, 0b11_001_000, 0xF4]);
            cpu.regs.set_reg(Reg16::Ax, 0x1200 | 0xF8);
            cpu.regs.set_reg(Reg16::Cx, 0x3400 | 0x09);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            if alu_op != 7 {
                assert_eq!(
                    cpu.regs.reg(Reg16::Ax),
                    0x1200 | expected.0,
                    "d0 field {alu_op}"
                );
            } else {
                assert_eq!(cpu.regs.reg(Reg16::Ax), 0x12F8, "cmp d0 {alu_op}");
            }
            assert_eq!(cpu.regs.reg(Reg16::Cx), 0x3409, "d0 field {alu_op}");
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "d0 field {alu_op}");

            // 02/0A/...|2: ADD Gb,Eb with d = 1 — AL (reg) receives op(AL, CL).
            let mut cpu = cpu_with(&[(0x02 | (alu_op << 3)), 0b11_000_001, 0xF4]);
            cpu.regs.set_reg(Reg16::Ax, 0x1200 | 0xF8);
            cpu.regs.set_reg(Reg16::Cx, 0x3400 | 0x09);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            if alu_op != 7 {
                assert_eq!(
                    cpu.regs.reg(Reg16::Ax),
                    0x1200 | expected.0,
                    "d1 field {alu_op}"
                );
            } else {
                assert_eq!(cpu.regs.reg(Reg16::Ax), 0x12F8, "cmp d1 {alu_op}");
            }
            assert_eq!(cpu.regs.reg(Reg16::Cx), 0x3409, "d1 field {alu_op}");
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "d1 field {alu_op}");
        }
    }

    #[test]
    fn rm_reg_word_memory_destination_round_trips_all_fields() {
        for alu_op in 0u8..8 {
            let expected = Cpu::alu_binary(alu_op, true, SWEEP_A16, SWEEP_B16, 1);
            // 01|8k with reg=CX over rm=[BX]: memory receives op(mem, CX).
            let mut cpu = cpu_with(&[(0x01 | (alu_op << 3)), 0b00_001_111, 0xF4]);
            cpu.regs.set_reg(Reg16::Bx, 0x0010);
            cpu.mem.write_word(0x0010, SWEEP_A16);
            cpu.regs.set_reg(Reg16::Cx, SWEEP_B16);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            assert_eq!(cpu.ip, RESET_OFF + 2, "field {alu_op}");
            let after = cpu.mem.read_word(0x0010);
            if alu_op != 7 {
                assert_eq!(after, expected.0, "field {alu_op}");
            } else {
                assert_eq!(after, SWEEP_A16, "cmp keeps the memory operand");
            }
            assert_eq!(cpu.regs.reg(Reg16::Cx), SWEEP_B16, "field {alu_op}");
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "field {alu_op}");
        }
    }

    #[test]
    fn rm_reg_word_memory_source_loads_through_decoder() {
        for alu_op in 0u8..8 {
            let expected = Cpu::alu_binary(alu_op, true, SWEEP_A16, SWEEP_B16, 1);
            // 03|8k with reg=AX over rm=[BX]: AX receives op(AX, mem).
            let mut cpu = cpu_with(&[(0x03 | (alu_op << 3)), 0b00_000_111, 0xF4]);
            cpu.regs.set_reg(Reg16::Bx, 0x0010);
            cpu.mem.write_word(0x0010, SWEEP_B16);
            cpu.regs.set_reg(Reg16::Ax, SWEEP_A16);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            if alu_op != 7 {
                assert_eq!(cpu.regs.reg(Reg16::Ax), expected.0, "field {alu_op}");
            } else {
                assert_eq!(cpu.regs.reg(Reg16::Ax), SWEEP_A16, "field {alu_op}");
            }
            assert_eq!(cpu.mem.read_word(0x0010), SWEEP_B16, "field {alu_op}");
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "field {alu_op}");
        }
    }

    #[test]
    fn rm_reg_word_add_matches_group1_word_words() {
        // 01 C8 = add ax, cx. 0xFFFF + 1 = 0 — the same flag word the
        // immediate path produced for these operands (CF|ZF|AF|PF).
        let mut cpu = cpu_with(&[0x01, 0xC8]);
        cpu.regs.set_reg(Reg16::Ax, 0xFFFF);
        cpu.regs.set_reg(Reg16::Cx, 1);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0);
        assert_eq!(cpu.flags.bits(), 0xF057);
    }

    #[test]
    fn rm_reg_word_sbb_consumes_carry() {
        // 1B C8 = sbb cx, ax with CF = 1: 1 - 0 - 1 = 0. ADD's flags word
        // would keep CX at 1 and clear PF.
        let mut cpu = cpu_with(&[0x1B, 0xC8]);
        cpu.regs.set_reg(Reg16::Cx, 1);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0);
        assert_eq!(cpu.flags.bits(), 0xF046);
    }

    #[test]
    fn rm_reg_byte_or_keeps_high_byte_and_sets_byte_sf() {
        // 08 C8 = or al, cl: 0xF0 | 0xFF = 0xFF; AH survives the byte RMW.
        let mut cpu = cpu_with(&[0x08, 0xC8]);
        cpu.regs.set_reg(Reg16::Ax, 0x12F0);
        cpu.regs.set_reg(Reg16::Cx, 0x34FF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x12FF);
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0x34FF);
        // PF (0xFF parity) | SF (bit 7 of the byte result).
        assert_eq!(cpu.flags.bits(), 0xF086);
    }

    #[test]
    fn rm_reg_byte_xor_clears_byte_carry_and_overflow() {
        // 30 C8 = xor al, cl: 0xF0 ^ 0xFF = 0x0F; OR would have left 0xFF.
        let mut cpu = cpu_with(&[0x30, 0xC8]);
        cpu.regs.set_reg(Reg16::Ax, 0x12F0);
        cpu.regs.set_reg(Reg16::Cx, 0x34FF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x120F);
        // 0x0F has even parity → PF, no other status flags.
        assert_eq!(cpu.flags.bits(), 0xF006);
    }

    #[test]
    fn rm_reg_consumes_opcode_modrm_and_displacement_only() {
        let cases: &[(&[u8], u16)] = &[
            (&[0x00, 0x07, 0xF4], 3),             // add byte [bx], al
            (&[0x01, 0x47, 0x02, 0xF4], 4),       // add word [bx+2], ax
            (&[0x02, 0x87, 0x34, 0x12, 0xF4], 5), // add al, [bx+0x1234]
            (&[0x03, 0xC8, 0xF4], 3),             // add cx, ax
            (&[0x39, 0x1F, 0xF4], 3),             // cmp word [bx], cx
            (&[0x38, 0xC1, 0xF4], 3),             // cmp cl, al
        ];
        for &(code, end) in cases {
            let cpu = loaded_to_halt(code);
            assert_eq!(cpu.ip, end, "code {code:02x?}");
        }
    }

    // -----------------------------------------------------------------------
    // Accumulator-immediate family (04–3D): the fixed AL/AX destination.
    // One (AL/AX, imm) pair at each width; the byte immediate is a raw byte,
    // the word immediate a full imm16.
    // -----------------------------------------------------------------------

    #[test]
    fn accumulator_byte_family_routes_all_eight_fields() {
        for alu_op in 0u8..8 {
            let mut cpu = cpu_with(&[0x04 | (alu_op << 3), 0x09]);
            cpu.regs.set_reg(Reg16::Ax, 0x1200 | 0xF8);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            assert_eq!(cpu.ip, RESET_OFF + 2, "field {alu_op}");
            let expected = Cpu::alu_binary(alu_op, false, 0xF8, 0x09, 1);
            if alu_op != 7 {
                assert_eq!(
                    cpu.regs.reg(Reg16::Ax),
                    0x1200 | expected.0,
                    "field {alu_op}"
                );
            } else {
                assert_eq!(cpu.regs.reg(Reg16::Ax), 0x12F8, "cmp field {alu_op}");
            }
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "field {alu_op}");
        }
    }

    #[test]
    fn accumulator_word_family_routes_all_eight_fields() {
        for alu_op in 0u8..8 {
            let mut cpu = cpu_with(&[0x05 | (alu_op << 3), 0x09, 0x00]);
            cpu.regs.set_reg(Reg16::Ax, SWEEP_A16);
            cpu.flags.set(Flags::CF, true);
            cpu.step().unwrap();
            assert_eq!(cpu.ip, RESET_OFF + 3, "field {alu_op}");
            let expected = Cpu::alu_binary(alu_op, true, SWEEP_A16, SWEEP_B16, 1);
            if alu_op != 7 {
                assert_eq!(cpu.regs.reg(Reg16::Ax), expected.0, "field {alu_op}");
            } else {
                assert_eq!(cpu.regs.reg(Reg16::Ax), SWEEP_A16, "cmp field {alu_op}");
            }
            assert_eq!(cpu.flags.bits(), 0xF002 | expected.1, "field {alu_op}");
        }
    }

    #[test]
    fn accumulator_byte_adc_consumes_carry_exact_word() {
        // 14 00 = adc al, 0 with CF = 1: 0x12xx + 0 + 1; AH is untouched.
        let mut cpu = cpu_with(&[0x14, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0x1200);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1201);
        assert_eq!(cpu.flags.bits(), 0xF002);
    }

    #[test]
    fn accumulator_byte_sbb_consumes_borrow_exact_word() {
        // 1C 00 = sbb al, 0 with CF = 1: 1 - 0 - 1 = 0 → ZF|PF on AH=0x12.
        let mut cpu = cpu_with(&[0x1C, 0x00]);
        cpu.regs.set_reg(Reg16::Ax, 0x1201);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1200);
        assert_eq!(cpu.flags.bits(), 0xF046);
    }

    #[test]
    fn accumulator_word_xor_clears_carry_and_overflow() {
        // 35 FF FF = xor ax, imm16: 0x5A5A ^ 0xFFFF = 0xA5A5 → SF|PF.
        let mut cpu = cpu_with(&[0x35, 0xFF, 0xFF]);
        cpu.regs.set_reg(Reg16::Ax, 0x5A5A);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0xA5A5);
        assert_eq!(cpu.flags.bits(), 0xF086);
    }

    // -----------------------------------------------------------------------
    // INC/DEC — the pro-carrier ops: CF is preserved bit-for-bit while every
    // other arithmetic flag follows add/sub by one.
    // -----------------------------------------------------------------------

    #[test]
    fn inc_dec_r16_wrap_all_registers_with_one_flag_tail() {
        // FFFE -> inc -> FFFF -> inc -> 0 -> dec -> FFFF -> dec -> FFFE.
        // The tail word is always SF (0xF082) for every register, SP
        // included — INC/DEC never touch the stack.
        for i in 0u8..8 {
            let mut cpu = cpu_with(&[0x40 + i, 0x40 + i, 0x48 + i, 0x48 + i]);
            cpu.regs.set_reg(Reg16::from_index(i), 0xFFFE);
            for _ in 0..4 {
                cpu.step().unwrap();
            }
            assert_eq!(cpu.regs.reg(Reg16::from_index(i)), 0xFFFE);
            assert_eq!(cpu.flags.bits(), 0xF082, "reg {i}");
        }
    }

    #[test]
    fn inc_r16_preserves_carry_exactly() {
        // inc ax with CF set: 0 + 1 = 1 raises no flags; CF must survive
        // untouched (ADD by one would have left it, SUB would too).
        let mut cpu = cpu_with(&[0x40]);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 1);
        assert!(cpu.flags.get(Flags::CF));
        assert!(!cpu.flags.get(Flags::ZF));
        assert_eq!(cpu.flags.bits(), 0xF003);
    }

    #[test]
    fn inc_r16_wraps_negative_with_over_and_sign_set() {
        // 0x7FFF + 1 = 0x8000 → OF|SF|AF|PF, CF preserved (clear here).
        let mut cpu = cpu_with(&[0x40]);
        cpu.regs.set_reg(Reg16::Ax, 0x7FFF);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x8000);
        assert_eq!(cpu.flags.bits(), 0xF896);
    }

    #[test]
    fn dec_r16_borrows_with_carry_exactly() {
        // 0x8000 - 1 = 0x7FFF → OF|AF|PF; SF must clear, CF untouched.
        let mut cpu = cpu_with(&[0x4F]);
        cpu.regs.set_reg(Reg16::Di, 0x8000);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Di), 0x7FFF);
        assert_eq!(cpu.flags.bits(), 0xF816);
    }

    #[test]
    fn inc_r16_carry_survives_word_wrap() {
        // 0xFFFF inc → 0: add(x, 1) would set CF; INC keeps the old CF = 1.
        let mut cpu = cpu_with(&[0x47]);
        cpu.regs.set_reg(Reg16::Di, 0xFFFF);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Di), 0);
        assert!(cpu.flags.get(Flags::CF), "CF untouched");
        assert!(cpu.flags.get(Flags::ZF));
        assert_eq!(cpu.flags.bits(), 0xF057);
    }

    #[test]
    fn inc_matches_add_one_except_carry_word() {
        let xs = [
            0x0000, 0x000F, 0x0010, 0x007F, 0x0080, 0x00FF, 0x7FFF, 0x8000, 0xFF0F, 0xFFFF,
        ];
        for &x in &xs {
            for cf in [false, true] {
                let mut adder = cpu_with(&[0x05, 0x01, 0x00]);
                adder.regs.set_reg(Reg16::Ax, x);
                adder.flags.set(Flags::CF, cf);
                adder.step().unwrap();

                let mut inced = cpu_with(&[0x40]);
                inced.regs.set_reg(Reg16::Ax, x);
                inced.flags.set(Flags::CF, cf);
                inced.step().unwrap();

                assert_eq!(inced.regs.reg(Reg16::Ax), adder.regs.reg(Reg16::Ax));
                let without_carry = Flags::CF;
                assert_eq!(
                    inced.flags.bits() & !without_carry,
                    adder.flags.bits() & !without_carry,
                    "x {x:#06x} cf {cf}"
                );
            }
        }
    }

    #[test]
    fn dec_matches_sub_one_except_carry_word() {
        let xs = [
            0x0000, 0x000F, 0x0010, 0x007F, 0x0080, 0x0100, 0x7FFF, 0x8000, 0xFF0F, 0xFFFF,
        ];
        for &x in &xs {
            for cf in [false, true] {
                let mut suber = cpu_with(&[0x2D, 0x01, 0x00]);
                suber.regs.set_reg(Reg16::Ax, x);
                suber.flags.set(Flags::CF, cf);
                suber.step().unwrap();

                let mut deced = cpu_with(&[0x48]);
                deced.regs.set_reg(Reg16::Ax, x);
                deced.flags.set(Flags::CF, cf);
                deced.step().unwrap();

                assert_eq!(deced.regs.reg(Reg16::Ax), suber.regs.reg(Reg16::Ax));
                let without_carry = Flags::CF;
                assert_eq!(
                    deced.flags.bits() & !without_carry,
                    suber.flags.bits() & !without_carry,
                    "x {x:#06x} cf {cf}"
                );
            }
        }
    }

    #[test]
    fn inc_byte_matches_add_one_except_carry() {
        let xs = [0x00, 0x0F, 0x10, 0x7F, 0x80, 0xFF];
        for &x in &xs {
            for cf in [false, true] {
                let mut adder = cpu_with(&[0x80, 0xC0, 0x01]);
                adder.regs.set_reg(Reg16::Ax, x);
                adder.flags.set(Flags::CF, cf);
                adder.step().unwrap();

                let mut inced = cpu_with(&[0xFE, 0xC0]);
                inced.regs.set_reg(Reg16::Ax, x);
                inced.flags.set(Flags::CF, cf);
                inced.step().unwrap();

                assert_eq!(inced.regs.reg(Reg16::Ax), adder.regs.reg(Reg16::Ax));
                let without_carry = Flags::CF;
                assert_eq!(
                    inced.flags.bits() & !without_carry,
                    adder.flags.bits() & !without_carry,
                    "x {x:#04x} cf {cf}"
                );
            }
        }
    }

    #[test]
    fn dec_byte_matches_sub_one_except_carry() {
        let xs = [0x00, 0x0F, 0x10, 0x7F, 0x80, 0xFF];
        for &x in &xs {
            for cf in [false, true] {
                let mut suber = cpu_with(&[0x80, 0xE8, 0x01]);
                suber.regs.set_reg(Reg16::Ax, x);
                suber.flags.set(Flags::CF, cf);
                suber.step().unwrap();

                let mut deced = cpu_with(&[0xFE, 0xC8]);
                deced.regs.set_reg(Reg16::Ax, x);
                deced.flags.set(Flags::CF, cf);
                deced.step().unwrap();

                assert_eq!(deced.regs.reg(Reg16::Ax), suber.regs.reg(Reg16::Ax));
                let without_carry = Flags::CF;
                assert_eq!(
                    deced.flags.bits() & !without_carry,
                    suber.flags.bits() & !without_carry,
                    "x {x:#04x} cf {cf}"
                );
            }
        }
    }

    #[test]
    fn inc_dec_rm_memory_byte_and_word_preserve_carry() {
        // FE /0 = inc byte [bx] with CF = 1: 0xFF wraps to 0 with ZF|AF|PF
        // and the old CF. The byte form must not widen its SF/ZF.
        let mut cpu = cpu_with(&[0xFE, 0x07]);
        cpu.regs.set_reg(Reg16::Bx, 0x0010);
        cpu.mem.write(0x0010, 0xFF);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read(0x0010), 0x00);
        assert_eq!(cpu.flags.bits(), 0xF057);

        // FF /1 = dec word [bx]: 0x0001 - 1 = 0 keeps CF and raises ZF|PF.
        let mut cpu = cpu_with(&[0xFF, 0x0F]);
        cpu.regs.set_reg(Reg16::Bx, 0x0020);
        cpu.mem.write_word(0x0020, 0x0001);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.mem.read_word(0x0020), 0x0000);
        assert!(cpu.flags.get(Flags::CF));
        assert!(cpu.flags.get(Flags::ZF));
        assert!(!cpu.flags.get(Flags::SF));
        assert_eq!(cpu.flags.bits(), 0xF047);
    }

    #[test]
    fn inc_dec_rm_register_forms_route_the_right_operand() {
        // FE C0 = inc al, FF C8 = dec ax — register-direct, byte vs word.
        let mut cpu = cpu_with(&[0xFE, 0xC0, 0xFF, 0xC8]);
        cpu.regs.set_reg(Reg16::Ax, 0x00FF);
        cpu.flags.set(Flags::CF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x0000, "byte inc wraps AL only");
        assert_eq!(cpu.flags.bits(), 0xF057, "CF kept; ZF|AF|PF from the wrap");
        cpu.step().unwrap();
        // 0x0000 - 1 (word) = 0xFFFF → SF|AF|PF, CF preserved.
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0xFFFF);
        assert_eq!(cpu.flags.bits(), 0xF097);
    }

    #[test]
    fn fe_ff_unsupported_reg_fields_report_form_bytes() {
        // FE /2 is not implemented; the error carries the instruction bytes
        // as consumed so far.
        let mut cpu = cpu_with(&[0xFE, 0xD0]);
        let err = cpu.step().unwrap_err();
        assert_eq!(
            err,
            StepError::UnsupportedForm {
                ip: RESET_OFF,
                bytes: vec![0xFE, 0xD0],
            }
        );
        assert_eq!(cpu.ip, RESET_OFF + 2);

        // FF /4 = JMP r/m16 — decoded syntactically but unsupported.
        let mut cpu = cpu_with(&[0xFF, 0xE0]);
        let err = cpu.step().unwrap_err();
        assert_eq!(
            err,
            StepError::UnsupportedForm {
                ip: RESET_OFF,
                bytes: vec![0xFF, 0xE0],
            }
        );
    }

    // -----------------------------------------------------------------------
    // Full Jcc family (70–7F): the taken/not-taken table for all sixteen
    // conditions, each with a pair of flag words that distinguishes it.
    // -----------------------------------------------------------------------

    fn set_arith_flags(cpu: &mut Cpu, bits: u16) {
        for &mask in &[
            Flags::CF,
            Flags::PF,
            Flags::AF,
            Flags::ZF,
            Flags::SF,
            Flags::OF,
        ] {
            cpu.flags.set(mask, bits & mask != 0);
        }
    }

    #[test]
    fn jcc_table_takes_and_declines_all_sixteen_conditions() {
        // (op, flags that take it, flags that decline it).
        let cases: &[(u8, u16, u16)] = &[
            (0x70, Flags::OF, 0),                     // jo
            (0x71, 0, Flags::OF),                     // jno
            (0x72, Flags::CF, 0),                     // jb
            (0x73, 0, Flags::CF),                     // jae
            (0x74, Flags::ZF, 0),                     // je
            (0x75, 0, Flags::ZF),                     // jne
            (0x76, Flags::ZF, 0),                     // jbe (ZF alone takes it)
            (0x77, 0, Flags::CF),                     // ja (needs neither)
            (0x78, Flags::SF, 0),                     // js
            (0x79, 0, Flags::SF),                     // jns
            (0x7A, Flags::PF, 0),                     // jp
            (0x7B, 0, Flags::PF),                     // jnp
            (0x7C, Flags::SF, 0),                     // jl (SF≠OF; SF==OF declines)
            (0x7D, Flags::SF | Flags::OF, Flags::SF), // jge (SF==OF takes; SF≠OF declines)
            (0x7E, Flags::ZF, Flags::SF | Flags::OF), // jle (ZF | SF≠OF)
            (0x7F, 0, Flags::ZF),                     // jg (¬ZF & SF==OF)
        ];
        for &(op, taken, declined) in cases {
            // Layout: [jcc, +1] skips one hlt and lands on the final hlt.
            let code = [op, 0x01, 0xF4, 0xF4];
            let mut cpu = cpu_with(&code);
            set_arith_flags(&mut cpu, taken);
            cpu.step().unwrap();
            assert_eq!(cpu.ip, RESET_OFF + 3, "{op:#04x} should take the jump");

            let mut cpu = cpu_with(&code);
            set_arith_flags(&mut cpu, declined);
            cpu.step().unwrap();
            assert_eq!(cpu.ip, RESET_OFF + 2, "{op:#04x} should fall through");
        }
    }

    #[test]
    fn jcc_leave_flags_unchanged_family() {
        let suit = [0x70u8, 0x71, 0x72, 0x73, 0x76, 0x77, 0x7C, 0x7D, 0x7E, 0x7F];
        for &op in &suit {
            let mut cpu = cpu_with(&[op, 0x00, 0xF4]);
            set_arith_flags(&mut cpu, Flags::SF | Flags::PF);
            let before = cpu.flags.bits();
            cpu.step().unwrap();
            assert_eq!(cpu.flags.bits(), before, "{op:#04x} must not write flags");
            assert_eq!(cpu.ip, RESET_OFF + 2, "{op:#04x} consumes 2 bytes");
        }
    }

    // -----------------------------------------------------------------------
    // Loop family (E0–E2): CX decremented, no flag writes of any kind;
    // LOOPE re-enters only while ZF holds, LOOPNE until ZF sets.
    // -----------------------------------------------------------------------

    #[test]
    fn loop_counts_down_and_never_touches_flags() {
        // 0: mov ax, 0 | 3: cmp ax, 0 (forces ZF=1) | 6: add ax, 2 | 9: loop 6.
        // Plain LOOP ignores ZF, so the stale ZF from cmp must not matter.
        let mut cpu = cpu_with(&[
            0xB8, 0x00, 0x00, // mov ax, 0
            0x83, 0xF8, 0x00, // cmp ax, 0 -> ZF set
            0x05, 0x02, 0x00, // add ax, 2
            0xE2, 0xFB, // loop: after = 0x000B; target 6 = 0x000B - 5
            0xF4,
        ]);
        cpu.regs.set_reg(Reg16::Cx, 3);
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert!(cpu.flags.get(Flags::ZF), "cmp ax, 0 raised ZF");
        for _ in 0..3 {
            cpu.step().unwrap(); // add ax, 2 — this writes flags
            let before = cpu.flags.bits();
            cpu.step().unwrap(); // loop: CX decremented, no flags
            assert_eq!(cpu.flags.bits(), before, "loop must not write flags");
        }
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0);
        assert_eq!(cpu.regs.reg(Reg16::Ax), 6);
        cpu.step().unwrap();
        assert!(cpu.halted);
    }

    #[test]
    fn loop_with_cx_zero_wraps_to_ffff_and_takes_the_branch() {
        // E2 FF: after = 2; 2 + 0xFFFF wraps to 0x0001.
        let mut cpu = cpu_with(&[0xE2, 0xFF, 0xF4]);
        cpu.regs.set_reg(Reg16::Cx, 0);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0xFFFF, "CX wraps on the decrement");
        assert_eq!(
            cpu.ip,
            RESET_OFF + 1,
            "a 0-starting CX still takes the branch"
        );
    }

    #[test]
    fn loope_exits_when_zf_clears() {
        // 0: mov ax, 0 | 3: cmp ax, 0 (ZF=1) | 6: inc ax (ZF clears)
        // 7: loope 6 | 9: hlt.
        let mut cpu = cpu_with(&[
            0xB8, 0x00, 0x00, // mov ax, 0
            0x83, 0xF8, 0x00, // cmp ax, 0
            0x40, // inc ax — ZF clears
            0xE1, 0xFD, // loope 0x0006 (after = 9; 9 + 0xFFFD = 6)
            0xF4,
        ]);
        cpu.regs.set_reg(Reg16::Cx, 2);
        cpu.step().unwrap();
        cpu.step().unwrap();
        assert!(cpu.flags.get(Flags::ZF));
        cpu.step().unwrap(); // inc ax: ZF clears
        assert!(!cpu.flags.get(Flags::ZF));
        cpu.step().unwrap(); // loope: CX 2 → 1 but ZF = 0 → falls through
        assert_eq!(cpu.regs.reg(Reg16::Cx), 1, "CX sinks even on the decline");
        assert_eq!(cpu.ip, RESET_OFF + 9);
        cpu.step().unwrap();
        assert!(cpu.halted);
    }

    #[test]
    fn loope_takes_the_back_edge_while_zf_holds() {
        let mut cpu = cpu_with(&[0xE1, 0xFF]);
        cpu.regs.set_reg(Reg16::Cx, 2);
        cpu.flags.set(Flags::ZF, true);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Cx), 1);
        assert_eq!(
            cpu.ip,
            RESET_OFF + 1,
            "disp8 -1 re-enters the single instruction"
        );
    }

    #[test]
    fn loopne_exits_when_zf_sets() {
        // 0: cmp ax, 0 with AX = 0 → ZF = 1 | 3: loopne 3 | 5: hlt.
        let mut cpu = cpu_with(&[0x83, 0xF8, 0x00, 0xE0, 0xFD, 0xF4]);
        cpu.regs.set_reg(Reg16::Cx, 2);
        cpu.regs.set_reg(Reg16::Ax, 0);
        cpu.step().unwrap(); // cmp: ZF = 1
        cpu.step().unwrap(); // loopne: CX 2 → 1, ZF = 1 → declines
        assert_eq!(cpu.regs.reg(Reg16::Cx), 1);
        assert_eq!(cpu.ip, RESET_OFF + 5);
        cpu.step().unwrap();
        assert!(cpu.halted);
    }

    #[test]
    fn loopne_takes_the_back_edge_while_zf_stays_clear() {
        // 0: cmp ax, 1 with AX = 2 (ZF = 0) | 3: loopne 3 | 5: hlt.
        let mut cpu = cpu_with(&[0x83, 0xF8, 0x01, 0xE0, 0xFE, 0xF4]);
        cpu.regs.set_reg(Reg16::Ax, 2);
        cpu.regs.set_reg(Reg16::Cx, 2);
        cpu.step().unwrap(); // cmp: ZF = 0
        let before = cpu.flags.bits();
        cpu.step().unwrap(); // loopne: CX 2 → 1, ZF clear → take
        assert_eq!(cpu.ip, RESET_OFF + 3, "re-entered while ZF was clear");
        cpu.step().unwrap(); // loopne: CX 1 → 0 → falls through on the drain
        assert_eq!(cpu.regs.reg(Reg16::Cx), 0);
        assert_eq!(cpu.flags.bits(), before, "loopne never writes flags");
        cpu.step().unwrap();
        assert!(cpu.halted);
    }

    // -----------------------------------------------------------------------
    // Stack primitives: PUSH/POP r16 (50-57, 58-5F), segment pushes/pops
    // (06/07, 0E/0F, 16/17, 1E/1F), PUSHF/POPF (9C/9D). All stack traffic
    // goes through SS:SP with 16-bit offset wrap.
    // -----------------------------------------------------------------------

    fn stack_cpu(sp: u16) -> Cpu {
        let mut cpu = cpu_with(&[]);
        cpu.regs.set_seg(Seg::Ss, 0x2000);
        cpu.regs.set_reg(Reg16::Sp, sp);
        cpu
    }

    #[test]
    fn push_pop_round_trip_all_registers() {
        // SP has dedicated quirk tests below (PUSH stores the new value,
        // POP takes the loaded value outright).
        for i in [0u8, 1, 2, 3, 5, 6, 7] {
            let r = Reg16::from_index(i);
            let mut cpu = stack_cpu(0x0100);
            cpu.regs.set_reg(r, 0xBEEF);
            cpu.load_flat(0xFFFF, 0x0000, &[0x50 + i, 0x58 + i]);
            cpu.step().unwrap();
            assert_eq!(cpu.regs.reg(Reg16::Sp), 0x00FE, "{i}: push");
            cpu.step().unwrap();
            assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0100, "{i}: pop");
            assert_eq!(cpu.regs.reg(r), 0xBEEF, "{i}: value restored");
        }
    }

    #[test]
    fn push_decrements_sp_and_writes_via_ss() {
        let mut cpu = stack_cpu(0x0100);
        cpu.regs.set_reg(Reg16::Bx, 0x1234);
        cpu.load_flat(0xFFFF, 0x0000, &[0x53]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x00FE);
        assert_eq!(cpu.mem.read_word(Memory::linear(0x2000, 0x00FE)), 0x1234);
        assert_eq!(cpu.ip, RESET_OFF + 1);
    }

    #[test]
    fn pop_reads_via_ss_and_increments_sp() {
        let mut cpu = stack_cpu(0x0100);
        cpu.mem.write_word(Memory::linear(0x2000, 0x0100), 0x5A5A);
        cpu.load_flat(0xFFFF, 0x0000, &[0x58]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x5A5A);
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0102);
        assert_eq!(cpu.ip, RESET_OFF + 1);
    }

    #[test]
    fn push_wraps_sp_below_zero() {
        let mut cpu = stack_cpu(0x0000);
        cpu.regs.set_reg(Reg16::Ax, 0x0102);
        cpu.load_flat(0xFFFF, 0x0000, &[0x50]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0xFFFE);
        assert_eq!(cpu.mem.read_word(Memory::linear(0x2000, 0xFFFE)), 0x0102);
    }

    #[test]
    fn pop_wraps_sp_above_top() {
        let mut cpu = stack_cpu(0xFFFE);
        cpu.mem.write_word(Memory::linear(0x2000, 0xFFFE), 0x3344);
        cpu.load_flat(0xFFFF, 0x0000, &[0x58]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Ax), 0x3344);
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0000);
    }

    #[test]
    fn push_sp_pushes_the_new_sp_8086_quirk() {
        // 8086 PUSH SP stores the decremented SP, not the original.
        // On 286+ the original value is stored; this pins 8086 behaviour.
        let mut cpu = stack_cpu(0x1234);
        cpu.load_flat(0xFFFF, 0x0000, &[0x54]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x1232);
        assert_eq!(
            cpu.mem.read_word(Memory::linear(0x2000, 0x1232)),
            0x1232,
            "the word stored is the NEW sp"
        );
    }

    #[test]
    fn pop_sp_takes_the_popped_value() {
        // POP SP discards the +2: the loaded value becomes SP outright.
        let mut cpu = stack_cpu(0x0100);
        cpu.mem.write_word(Memory::linear(0x2000, 0x0100), 0xBEEF);
        cpu.load_flat(0xFFFF, 0x0000, &[0x5C]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0xBEEF);
    }

    #[test]
    fn segment_registers_push_and_pop_all_four() {
        let mut cpu = stack_cpu(0x0100);
        // SS holds the live stack segment: pushing and popping it must
        // restore the value, or the later pops would read elsewhere.
        // CS likewise round-trips its own fetch-segment value.
        cpu.regs.set_seg(Seg::Es, 0x1111);
        cpu.regs.set_seg(Seg::Ds, 0x4444);
        // push es, push cs, push ss, push ds,
        // then pop ds, pop ss, pop cs, pop es — reverse order.
        cpu.load_flat(
            0xFFFF,
            0x0000,
            &[0x06, 0x0E, 0x16, 0x1E, 0x1F, 0x17, 0x0F, 0x07],
        );
        for _ in 0..8 {
            cpu.step().unwrap();
        }
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0100);
        assert_eq!(cpu.regs.seg(Seg::Es), 0x1111);
        assert_eq!(cpu.regs.seg(Seg::Cs), 0xFFFF);
        assert_eq!(cpu.regs.seg(Seg::Ss), 0x2000);
        assert_eq!(cpu.regs.seg(Seg::Ds), 0x4444);
    }

    #[test]
    fn pop_cs_switches_the_code_segment() {
        let mut cpu = stack_cpu(0x0010);
        cpu.mem.write_word(Memory::linear(0x2000, 0x0010), 0x2000);
        cpu.regs.set_seg(Seg::Cs, 0x0000);
        cpu.mem.load(0x00000, &[0x0F]);
        cpu.mem.load(0x20001, &[0xF4]);
        cpu.ip = 0x0000;
        cpu.step().unwrap();
        assert_eq!(cpu.regs.seg(Seg::Cs), 0x2000);
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0012);
        assert_eq!(cpu.ip, 0x0001);
        cpu.step().unwrap();
        assert!(cpu.halted, "next fetch ran from the new CS");
    }

    #[test]
    fn pushf_popf_round_trip_writable_flags() {
        let mut cpu = stack_cpu(0x0100);
        cpu.flags.set(Flags::CF, true);
        cpu.flags.set(Flags::ZF, true);
        cpu.load_flat(0xFFFF, 0x0000, &[0x9C, 0x9D]);
        cpu.step().unwrap();
        assert_eq!(
            cpu.mem.read_word(Memory::linear(0x2000, 0x00FE)),
            0xF043,
            "CF | ZF on the static base"
        );
        cpu.flags.set(Flags::CF, false);
        cpu.flags.set(Flags::ZF, false);
        cpu.step().unwrap();
        assert!(cpu.flags.get(Flags::CF));
        assert!(cpu.flags.get(Flags::ZF));
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0100);
    }

    #[test]
    fn push_and_pop_leave_flags_unchanged() {
        let mut cpu = stack_cpu(0x0100);
        cpu.regs.set_reg(Reg16::Bx, 0x0001);
        cpu.load_flat(0xFFFF, 0x0000, &[0x53, 0x5B, 0x06, 0x07, 0x9C]);
        cpu.step().unwrap();
        cpu.step().unwrap();
        cpu.step().unwrap();
        cpu.step().unwrap();
        let before = cpu.flags.bits();
        cpu.step().unwrap();
        assert_eq!(
            cpu.flags.bits(),
            before,
            "pushf reads flags without writing"
        );
    }

    // -----------------------------------------------------------------------
    // Near call and return: CALL rel16 (E8), RET imm16 (C2), RET (C3).
    // Return addresses live on the SS:SP stack; IP arithmetic wraps at 16
    // bits. No flags are touched.
    // -----------------------------------------------------------------------

    #[test]
    fn call_pushes_return_address_and_jumps() {
        let mut cpu = cpu_with(&[0xE8, 0x05, 0x00, 0x90, 0x90, 0x90, 0x90, 0x90, 0xF4]);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0x0008);
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0xFFFE);
        assert_eq!(cpu.mem.read_word(0xFFFE), 0x0003);
        cpu.step().unwrap();
        assert!(cpu.halted, "call reached the hlt");
        assert_eq!(cpu.ip, 0x0009);
    }

    #[test]
    fn call_target_wraps_ip_at_16_bits() {
        let mut cpu = Cpu::new();
        cpu.regs.set_seg(Seg::Cs, 0x0000);
        cpu.mem.load(0xFFF0, &[0xE8, 0xFF, 0x7F]);
        cpu.ip = 0xFFF0;
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0x7FF2);
        assert_eq!(cpu.mem.read_word(0xFFFE), 0xFFF3);
    }

    #[test]
    fn ret_pops_return_address() {
        let mut cpu = cpu_with(&[0xC3]);
        cpu.regs.set_reg(Reg16::Sp, 0x0100);
        cpu.mem.write_word(0x0100, 0x0210);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0x0210);
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0102);
    }

    #[test]
    fn ret_imm_drops_stack_arguments() {
        let mut cpu = cpu_with(&[0xC2, 0x04, 0x00]);
        cpu.regs.set_reg(Reg16::Sp, 0x0100);
        cpu.mem.write_word(0x0100, 0x0210);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0x0210);
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0106);
    }

    #[test]
    fn nested_call_and_ret_unwind_sp() {
        let mut cpu = cpu_with(&[
            0xE8, 0x04, 0x00, // call 0x0007
            0xF4, // hlt (return lands here)
            0x90, 0x90, 0x90, // padding
            0xE8, 0x02, 0x00, // call 0x000C
            0xC3, // ret -> 0x0003
            0x90, // padding
            0xC3, // ret -> 0x000A
        ]);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0xFFFE);
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0xFFFC);
        cpu.step().unwrap();
        assert_eq!(cpu.ip, 0x000A, "inner ret returns into outer");
        cpu.step().unwrap();
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0000, "stack fully unwound");
        cpu.step().unwrap();
        assert!(cpu.halted);
        assert_eq!(cpu.ip, 0x0004);
    }

    #[test]
    fn call_and_ret_leave_flags_unchanged() {
        let mut cpu = cpu_with(&[0xE8, 0x03, 0x00, 0xF4, 0x90, 0x90, 0xC3]);
        cpu.step().unwrap();
        let before = cpu.flags.bits();
        cpu.step().unwrap();
        assert_eq!(cpu.flags.bits(), before);
        assert_eq!(cpu.ip, RESET_OFF + 3);
        assert_eq!(cpu.regs.reg(Reg16::Sp), 0x0000);
    }
}
