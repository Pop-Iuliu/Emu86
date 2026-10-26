fn flag_bits(a: u16, b: u16, r: u16, cf: bool, of: bool, sign: u16) -> u16 {
    // width = 0x8000 | 0x7FFF = 0xFFFF (word); 0x0080 | 0x007F = 0x00FF (byte).
    let width = sign | (sign - 1);
    let cf = u16::from(cf);
    let pf = u16::from(r as u8).count_ones() as u16 % 2;
    let pf = (1 - pf) << 2;
    let af = u16::from((a ^ b ^ r) & 0x10 != 0) << 4;
    let zf = u16::from(r & width == 0) << 6;
    let sf = u16::from(r & sign != 0) << 7;
    let of = u16::from(of) << 11;
    cf | pf | af | zf | sf | of
}

pub fn add(a: u16, b: u16) -> (u16, u16) {
    let r = a.wrapping_add(b);
    let cf = a as u32 + b as u32 > 0xFFFF;
    let of = !(a ^ b) & (a ^ r) & 0x8000 != 0;
    (r, flag_bits(a, b, r, cf, of, 0x8000))
}

pub fn sub(a: u16, b: u16) -> (u16, u16) {
    let r = a.wrapping_sub(b);
    let cf = a < b;
    let of = (a ^ b) & (a ^ r) & 0x8000 != 0;
    (r, flag_bits(a, b, r, cf, of, 0x8000))
}

pub fn add8(a: u8, b: u8) -> (u8, u16) {
    let r = a.wrapping_add(b);
    let cf = (a as u16) + (b as u16) > 0xFF;
    let of = !(a ^ b) & (a ^ r) & 0x80 != 0;
    (
        r,
        flag_bits(a as u16, b as u16, r as u16, cf, of, 0x0080),
    )
}

pub fn sub8(a: u8, b: u8) -> (u8, u16) {
    let r = a.wrapping_sub(b);
    let cf = a < b;
    let of = (a ^ b) & (a ^ r) & 0x80 != 0;
    (
        r,
        flag_bits(a as u16, b as u16, r as u16, cf, of, 0x0080),
    )
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

    #[test]
    fn add8_carry_and_zero() {
        let (r, f) = add8(0xFF, 1);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::CF | Flags::ZF | Flags::AF | Flags::PF);
    }

    #[test]
    fn add8_signed_overflow_sets_of_and_sf() {
        let (r, f) = add8(0x7F, 1);
        assert_eq!(r, 0x80);
        // 0x80 has one set bit -> PF clear; unlike the 16-bit sibling whose low
        // byte is 0x00.
        assert_eq!(f, Flags::SF | Flags::OF | Flags::AF);
    }

    #[test]
    fn sub8_borrow() {
        let (r, f) = sub8(0, 1);
        assert_eq!(r, 0xFF);
        assert_eq!(f, Flags::CF | Flags::SF | Flags::AF | Flags::PF);
    }

    #[test]
    fn sub8_signed_overflow() {
        let (r, f) = sub8(0x80, 1);
        assert_eq!(r, 0x7F);
        // 0x7F has seven set bits -> PF clear.
        assert_eq!(f, Flags::AF | Flags::OF);
    }
}
