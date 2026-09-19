fn flag_bits(a: u16, b: u16, r: u16, cf: bool, of: bool) -> u16 {
    let cf = u16::from(cf);
    let pf = u16::from(r as u8).count_ones() as u16 % 2;
    let pf = (1 - pf) << 2;
    let af = u16::from((a ^ b ^ r) & 0x10 != 0) << 4;
    let zf = u16::from(r == 0) << 6;
    let sf = u16::from(r & 0x8000 != 0) << 7;
    let of = u16::from(of) << 11;
    cf | pf | af | zf | sf | of
}

pub fn add(a: u16, b: u16) -> (u16, u16) {
    let r = a.wrapping_add(b);
    let cf = a as u32 + b as u32 > 0xFFFF;
    let of = !(a ^ b) & (a ^ r) & 0x8000 != 0;
    (r, flag_bits(a, b, r, cf, of))
}

pub fn sub(a: u16, b: u16) -> (u16, u16) {
    let r = a.wrapping_sub(b);
    let cf = a < b;
    let of = (a ^ b) & (a ^ r) & 0x8000 != 0;
    (r, flag_bits(a, b, r, cf, of))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::Flags;

    #[test]
    fn add_carry_and_zero() {
        let (r, f) = add(0xFFFF, 1);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::CF | Flags::ZF | Flags::AF | Flags::PF);
    }

    #[test]
    fn add_signed_overflow_sets_of_and_sf() {
        let (r, f) = add(0x7FFF, 1);
        assert_eq!(r, 0x8000);
        assert_eq!(f, Flags::SF | Flags::OF | Flags::AF | Flags::PF);
    }

    #[test]
    fn sub_borrow() {
        let (r, f) = sub(0, 1);
        assert_eq!(r, 0xFFFF);
        assert_eq!(f, Flags::CF | Flags::SF | Flags::AF | Flags::PF);
    }

    #[test]
    fn sub_signed_overflow() {
        let (r, f) = sub(0x8000, 1);
        assert_eq!(r, 0x7FFF);
        assert_eq!(f, Flags::AF | Flags::PF | Flags::OF);
    }
}
