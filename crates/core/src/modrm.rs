use crate::mem::Memory;
use crate::reg::{Reg16, Reg8, RegFile, Seg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand {
    Reg16(Reg16),
    Reg8(Reg8),
    Word { seg: Seg, off: u16 },
    Byte { seg: Seg, off: u16 },
}

impl Operand {
    pub fn decode_reg(modrm: u8, wide: bool) -> Operand {
        let r = (modrm >> 3) & 0b111;
        if wide {
            Operand::Reg16(Reg16::from_index(r))
        } else {
            Operand::Reg8(Reg8::from_index(r))
        }
    }

    pub fn decode_rm(
        fetch: &mut impl FnMut() -> u8,
        regs: RegFile,
        modrm: u8,
        wide: bool,
    ) -> Operand {
        let rm = modrm & 0b111;
        match modrm >> 6 {
            3 => {
                if wide {
                    Operand::Reg16(Reg16::from_index(rm))
                } else {
                    Operand::Reg8(Reg8::from_index(rm))
                }
            }
            0 if rm == 6 => Self::memory(fetch_addr(fetch), Seg::Ds, wide),
            md => {
                let disp: u16 = match md {
                    1 => fetch() as i8 as u16,
                    2 => fetch_addr(fetch),
                    _ => 0,
                };
                Self::memory(ea_base(rm, regs).wrapping_add(disp), ea_seg(rm), wide)
            }
        }
    }

    fn memory(off: u16, seg: Seg, wide: bool) -> Operand {
        if wide {
            Operand::Word { seg, off }
        } else {
            Operand::Byte { seg, off }
        }
    }

    pub fn read(&self, regs: &RegFile, mem: &Memory) -> u16 {
        match *self {
            Operand::Reg16(r) => regs.reg(r),
            Operand::Reg8(r) => u16::from(regs.reg8(r)),
            Operand::Word { seg, off } => mem.read_word(Memory::linear(regs.seg(seg), off)),
            Operand::Byte { seg, off } => u16::from(mem.read(Memory::linear(regs.seg(seg), off))),
        }
    }

    pub fn write(&self, regs: &mut RegFile, mem: &mut Memory, v: u16) {
        match *self {
            Operand::Reg16(r) => regs.set_reg(r, v),
            Operand::Reg8(r) => regs.set_reg8(r, v as u8),
            Operand::Word { seg, off } => mem.write_word(Memory::linear(regs.seg(seg), off), v),
            Operand::Byte { seg, off } => mem.write(Memory::linear(regs.seg(seg), off), v as u8),
        }
    }
}

fn fetch_addr(fetch: &mut impl FnMut() -> u8) -> u16 {
    let lo = fetch() as u16;
    (fetch() as u16) << 8 | lo
}

fn ea_base(rm: u8, regs: RegFile) -> u16 {
    match rm {
        0 => regs.reg(Reg16::Bx).wrapping_add(regs.reg(Reg16::Si)),
        1 => regs.reg(Reg16::Bx).wrapping_add(regs.reg(Reg16::Di)),
        2 => regs.reg(Reg16::Bp).wrapping_add(regs.reg(Reg16::Si)),
        3 => regs.reg(Reg16::Bp).wrapping_add(regs.reg(Reg16::Di)),
        4 => regs.reg(Reg16::Si),
        5 => regs.reg(Reg16::Di),
        6 => regs.reg(Reg16::Bp),
        _ => regs.reg(Reg16::Bx),
    }
}

fn ea_seg(rm: u8) -> Seg {
    match rm {
        2 | 3 | 6 => Seg::Ss,
        _ => Seg::Ds,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn regs() -> RegFile {
        let mut r = RegFile::new();
        r.set_reg(Reg16::Bx, 0x1000);
        r.set_reg(Reg16::Bp, 0x2000);
        r.set_reg(Reg16::Si, 0x0300);
        r.set_reg(Reg16::Di, 0x0400);
        r
    }

    fn dec(modrm: u8, disp: &[u8], regs: RegFile, wide: bool) -> Operand {
        let mut it = disp.iter();
        let mut fetch = || *it.next().unwrap();
        Operand::decode_rm(&mut fetch, regs, modrm, wide)
    }

    #[test]
    fn register_direct_selects_word_or_byte_view() {
        assert_eq!(
            dec(0b11_000_111, &[], regs(), true),
            Operand::Reg16(Reg16::Di)
        );
        assert_eq!(
            dec(0b11_000_101, &[], regs(), false),
            Operand::Reg8(Reg8::Ch)
        );
    }

    #[test]
    fn mod0_covers_all_eight_address_forms() {
        let r = regs();
        assert_eq!(
            dec(0b00_000_000, &[], r, true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x1300
            }
        );
        assert_eq!(
            dec(0b00_000_001, &[], r, true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x1400
            }
        );
        assert_eq!(
            dec(0b00_000_010, &[], r, true),
            Operand::Word {
                seg: Seg::Ss,
                off: 0x2300
            }
        );
        assert_eq!(
            dec(0b00_000_011, &[], r, true),
            Operand::Word {
                seg: Seg::Ss,
                off: 0x2400
            }
        );
        assert_eq!(
            dec(0b00_000_100, &[], r, true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x0300
            }
        );
        assert_eq!(
            dec(0b00_000_101, &[], r, true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x0400
            }
        );
        assert_eq!(
            dec(0b00_000_111, &[], r, true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x1000
            }
        );
    }

    #[test]
    fn mod0_rm6_is_direct_address_not_bp() {
        assert_eq!(
            dec(0b00_000_110, &[0x00, 0x10], regs(), true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x1000
            }
        );
    }

    #[test]
    fn mod1_sign_extends_disp8() {
        assert_eq!(
            dec(0b01_000_000, &[0x02], regs(), true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x1302
            }
        );
        assert_eq!(
            dec(0b01_000_000, &[0xFF], regs(), true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x12FF
            }
        );
    }

    #[test]
    fn mod2_adds_disp16() {
        assert_eq!(
            dec(0b10_000_010, &[0x34, 0x12], regs(), true),
            Operand::Word {
                seg: Seg::Ss,
                off: 0x3534
            }
        );
    }

    #[test]
    fn byte_mode_yields_byte_operand() {
        assert_eq!(
            dec(0b00_000_100, &[], regs(), false),
            Operand::Byte {
                seg: Seg::Ds,
                off: 0x0300
            }
        );
    }

    #[test]
    fn base_sums_wrap_at_16_bits() {
        let mut r = RegFile::new();
        r.set_reg(Reg16::Bx, 0xFFFF);
        r.set_reg(Reg16::Si, 0xFFFF);
        assert_eq!(
            dec(0b00_000_000, &[], r, true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0xFFFE
            }
        );
    }

    #[test]
    fn fetch_reads_low_byte_first() {
        assert_eq!(
            dec(0b00_000_110, &[0x34, 0x12], regs(), true),
            Operand::Word {
                seg: Seg::Ds,
                off: 0x1234
            }
        );
    }

    #[test]
    fn decode_reg_maps_reg_field() {
        assert_eq!(
            Operand::decode_reg(0b00_101_000, true),
            Operand::Reg16(Reg16::Bp)
        );
        assert_eq!(
            Operand::decode_reg(0b00_100_000, false),
            Operand::Reg8(Reg8::Ah)
        );
    }

    #[test]
    fn read_write_roundtrip_through_memory() {
        let mut r = RegFile::new();
        let mut m = Memory::new();
        let o = Operand::Word {
            seg: Seg::Ds,
            off: 0x0010,
        };
        o.write(&mut r, &mut m, 0xBEEF);
        assert_eq!(o.read(&r, &m), 0xBEEF);
        assert_eq!(m.read_word(0x0010), 0xBEEF);
    }

    #[test]
    fn read_write_roundtrip_through_byte_reg() {
        let mut r = RegFile::new();
        let mut m = Memory::new();
        let o = Operand::Reg8(Reg8::Bh);
        o.write(&mut r, &mut m, 0x5A);
        assert_eq!(o.read(&r, &m), 0x005A);
        assert_eq!(r.reg(Reg16::Bx), 0x5A00);
    }
}
