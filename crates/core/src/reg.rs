#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg16 {
    Ax = 0,
    Cx = 1,
    Dx = 2,
    Bx = 3,
    Sp = 4,
    Bp = 5,
    Si = 6,
    Di = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Reg8 {
    Al = 0,
    Cl = 1,
    Dl = 2,
    Bl = 3,
    Ah = 4,
    Ch = 5,
    Dh = 6,
    Bh = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Seg {
    Es = 0,
    Cs = 1,
    Ss = 2,
    Ds = 3,
}

#[derive(Clone, Copy)]
pub struct RegFile {
    words: [u16; 8],
    segs: [u16; 4],
}

impl RegFile {
    pub fn new() -> Self {
        Self {
            words: [0; 8],
            segs: [0; 4],
        }
    }

    pub fn reg(&self, r: Reg16) -> u16 {
        self.words[r as usize]
    }

    pub fn set_reg(&mut self, r: Reg16, v: u16) {
        self.words[r as usize] = v;
    }

    pub fn reg8(&self, r: Reg8) -> u8 {
        match r as usize {
            i @ 0..=3 => self.words[i] as u8,
            i => (self.words[i - 4] >> 8) as u8,
        }
    }

    pub fn set_reg8(&mut self, r: Reg8, v: u8) {
        match r as usize {
            i @ 0..=3 => self.words[i] = (self.words[i] & 0xFF00) | v as u16,
            i => {
                self.words[i - 4] = (self.words[i - 4] & 0x00FF) | ((v as u16) << 8);
            }
        }
    }

    pub fn seg(&self, s: Seg) -> u16 {
        self.segs[s as usize]
    }

    pub fn set_seg(&mut self, s: Seg, v: u16) {
        self.segs[s as usize] = v;
    }
}

impl Reg16 {
    pub fn from_index(i: u8) -> Self {
        match i & 0b111 {
            0 => Self::Ax,
            1 => Self::Cx,
            2 => Self::Dx,
            3 => Self::Bx,
            4 => Self::Sp,
            5 => Self::Bp,
            6 => Self::Si,
            _ => Self::Di,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ax" => Some(Self::Ax),
            "cx" => Some(Self::Cx),
            "dx" => Some(Self::Dx),
            "bx" => Some(Self::Bx),
            "sp" => Some(Self::Sp),
            "bp" => Some(Self::Bp),
            "si" => Some(Self::Si),
            "di" => Some(Self::Di),
            _ => None,
        }
    }
}

impl Reg8 {
    pub fn from_index(i: u8) -> Self {
        match i & 0b111 {
            0 => Self::Al,
            1 => Self::Cl,
            2 => Self::Dl,
            3 => Self::Bl,
            4 => Self::Ah,
            5 => Self::Ch,
            6 => Self::Dh,
            _ => Self::Bh,
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "al" => Some(Self::Al),
            "cl" => Some(Self::Cl),
            "dl" => Some(Self::Dl),
            "bl" => Some(Self::Bl),
            "ah" => Some(Self::Ah),
            "ch" => Some(Self::Ch),
            "dh" => Some(Self::Dh),
            "bh" => Some(Self::Bh),
            _ => None,
        }
    }
}

impl Seg {
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "es" => Some(Self::Es),
            "cs" => Some(Self::Cs),
            "ss" => Some(Self::Ss),
            "ds" => Some(Self::Ds),
            _ => None,
        }
    }
}

impl Default for RegFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn byte_views_alias_word() {
        let mut rf = RegFile::new();
        rf.set_reg8(Reg8::Al, 0x12);
        rf.set_reg8(Reg8::Ah, 0x34);
        assert_eq!(rf.reg(Reg16::Ax), 0x3412);
        rf.set_reg(Reg16::Ax, 0xABCD);
        assert_eq!(rf.reg8(Reg8::Al), 0xCD);
        assert_eq!(rf.reg8(Reg8::Ah), 0xAB);
    }

    #[test]
    fn names_resolve_every_register() {
        assert_eq!(Reg16::from_name("ax"), Some(Reg16::Ax));
        assert_eq!(Reg16::from_name("di"), Some(Reg16::Di));
        assert_eq!(Reg16::from_name("AX"), None);
        assert_eq!(Reg16::from_name("ah"), None);
        assert_eq!(Reg8::from_name("al"), Some(Reg8::Al));
        assert_eq!(Reg8::from_name("bh"), Some(Reg8::Bh));
        assert_eq!(Reg8::from_name("ax"), None);
        assert_eq!(Seg::from_name("cs"), Some(Seg::Cs));
        assert_eq!(Seg::from_name("ds"), Some(Seg::Ds));
        assert_eq!(Seg::from_name("ip"), None);
    }
}
