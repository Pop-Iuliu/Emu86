const STATIC: u16 = 0xF002;
const WRITABLE: u16 = 0x0FD5;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Flags(u16);

impl Flags {
    pub const CF: u16 = 1 << 0;
    pub const PF: u16 = 1 << 2;
    pub const AF: u16 = 1 << 4;
    pub const ZF: u16 = 1 << 6;
    pub const SF: u16 = 1 << 7;
    pub const TF: u16 = 1 << 8;
    pub const IF: u16 = 1 << 9;
    pub const DF: u16 = 1 << 10;
    pub const OF: u16 = 1 << 11;
    pub const SETTABLE: u16 = Self::CF | Self::PF | Self::AF | Self::ZF | Self::SF | Self::OF;

    pub fn bits(self) -> u16 {
        self.0 | STATIC
    }

    pub fn set_bits(&mut self, v: u16) {
        self.0 = v & WRITABLE;
    }

    pub fn get(self, flag: u16) -> bool {
        self.bits() & flag != 0
    }

    pub fn set(&mut self, flag: u16, on: bool) {
        let b = self.bits();
        self.0 = if on { b | flag } else { b & !flag } & WRITABLE;
    }

    pub fn update(&mut self, mask: u16, bits: u16) {
        self.set_bits((self.bits() & !mask) | bits);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_bits_are_visible_but_not_writable() {
        let mut f = Flags::default();
        assert_eq!(f.bits(), 0xF002);
        f.set_bits(0xFFFF);
        assert_eq!(f.bits(), 0xF002 | WRITABLE);
        f.set(Flags::ZF, true);
        assert!(f.get(Flags::ZF));
        f.set(Flags::ZF, false);
        assert!(!f.get(Flags::ZF));
        assert!(f.get(Flags::CF));
        f.set(Flags::CF, false);
        assert!(!f.get(Flags::CF));
    }
}
