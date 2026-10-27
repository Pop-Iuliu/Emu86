use crate::flags::Flags;

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

/// Flag mask for the bitwise operators: CF, AF and OF are always cleared,
/// so only PF/ZF/SF are derived, from the result. `sign` selects the width
/// (`0x8000` word, `0x0080` byte).
fn logic_flags(r: u16, sign: u16) -> u16 {
    let width = sign | (sign - 1);
    let pf = u16::from(r as u8).count_ones() as u16 % 2;
    let pf = (1 - pf) << 2;
    let zf = u16::from(r & width == 0) << 6;
    let sf = u16::from(r & sign != 0) << 7;
    pf | zf | sf
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

/// ADD with carry-in. `carry` is 0 or 1. CF includes it, and OF/AF/PF/ZF/SF
/// follow `add`'s rules adjusted for the extra increment.
pub fn adc(a: u16, b: u16, carry: u16) -> (u16, u16) {
    let r = a.wrapping_add(b).wrapping_add(carry);
    let cf = a as u32 + b as u32 + carry as u32 > 0xFFFF;
    let of = !(a ^ b) & (a ^ r) & 0x8000 != 0;
    (r, flag_bits(a, b, r, cf, of, 0x8000))
}

/// SUB with borrow-in. `carry` is 0 or 1. CF reports borrow from the
/// effective subtrahend `b + carry`, so `sbb(1, 0, 1)` produces CF = 0 but
/// `sbb(0, 0, 1)` produces CF = 1.
pub fn sbb(a: u16, b: u16, carry: u16) -> (u16, u16) {
    let r = a.wrapping_sub(b).wrapping_sub(carry);
    let cf = (a as u32) < (b as u32) + (carry as u32);
    let of = (a ^ b) & (a ^ r) & 0x8000 != 0;
    (r, flag_bits(a, b, r, cf, of, 0x8000))
}

/// Bitwise AND. Clears CF, AF and OF; SF/ZF/PF from the result.
pub fn and(a: u16, b: u16) -> (u16, u16) {
    let r = a & b;
    (r, logic_flags(r, 0x8000))
}

/// Bitwise OR. Clears CF, AF and OF; SF/ZF/PF from the result.
pub fn or(a: u16, b: u16) -> (u16, u16) {
    let r = a | b;
    (r, logic_flags(r, 0x8000))
}

/// Bitwise XOR. Clears CF, AF and OF; SF/ZF/PF from the result.
pub fn xor(a: u16, b: u16) -> (u16, u16) {
    let r = a ^ b;
    (r, logic_flags(r, 0x8000))
}

pub fn add8(a: u8, b: u8) -> (u8, u16) {
    let r = a.wrapping_add(b);
    let cf = (a as u16) + (b as u16) > 0xFF;
    let of = !(a ^ b) & (a ^ r) & 0x80 != 0;
    (r, flag_bits(a as u16, b as u16, r as u16, cf, of, 0x0080))
}

pub fn sub8(a: u8, b: u8) -> (u8, u16) {
    let r = a.wrapping_sub(b);
    let cf = a < b;
    let of = (a ^ b) & (a ^ r) & 0x80 != 0;
    (r, flag_bits(a as u16, b as u16, r as u16, cf, of, 0x0080))
}

/// 8-bit sibling of `adc`; flags are width-correct (CF/OF from bit 7).
pub fn adc8(a: u8, b: u8, carry: u8) -> (u8, u16) {
    let r = a.wrapping_add(b).wrapping_add(carry);
    let cf = (a as u16) + (b as u16) + (carry as u16) > 0xFF;
    let of = !(a ^ b) & (a ^ r) & 0x80 != 0;
    (r, flag_bits(a as u16, b as u16, r as u16, cf, of, 0x0080))
}

/// 8-bit sibling of `sbb`; flags are width-correct (CF/OF from bit 7).
pub fn sbb8(a: u8, b: u8, carry: u8) -> (u8, u16) {
    let r = a.wrapping_sub(b).wrapping_sub(carry);
    let cf = (a as u16) < (b as u16) + (carry as u16);
    let of = (a ^ b) & (a ^ r) & 0x80 != 0;
    (r, flag_bits(a as u16, b as u16, r as u16, cf, of, 0x0080))
}

/// 8-bit sibling of `and`; SF from bit 7, ZF over 8 bits.
pub fn and8(a: u8, b: u8) -> (u8, u16) {
    let r = a & b;
    (r, logic_flags(r as u16, 0x0080))
}

/// 8-bit sibling of `or`; SF from bit 7, ZF over 8 bits.
pub fn or8(a: u8, b: u8) -> (u8, u16) {
    let r = a | b;
    (r, logic_flags(r as u16, 0x0080))
}

/// 8-bit sibling of `xor`; SF from bit 7, ZF over 8 bits.
pub fn xor8(a: u8, b: u8) -> (u8, u16) {
    let r = a ^ b;
    (r, logic_flags(r as u16, 0x0080))
}

/// `INC r/m16`: every arithmetic flag follows `add(a, 1)` — OF, AF, SF, ZF
/// and PF included — but CF is cleared in the returned word. INC never
/// writes CF, so the caller ORs the incoming carry back on top.
pub fn inc(a: u16) -> (u16, u16) {
    let (r, f) = add(a, 1);
    (r, f & !Flags::CF)
}

/// `DEC r/m16`: every arithmetic flag follows `sub(a, 1)`; CF preserved
/// the same way as `inc`.
pub fn dec(a: u16) -> (u16, u16) {
    let (r, f) = sub(a, 1);
    (r, f & !Flags::CF)
}

/// 8-bit sibling of `inc`; OF/AF/SF/ZF/PF from bit 7 width rules.
pub fn inc8(a: u8) -> (u8, u16) {
    let (r, f) = add8(a, 1);
    (r, f & !Flags::CF)
}

/// 8-bit sibling of `dec`.
pub fn dec8(a: u8) -> (u8, u16) {
    let (r, f) = sub8(a, 1);
    (r, f & !Flags::CF)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn adc_adds_carry_in() {
        let (r, f) = adc(0x0000, 0x0000, 1);
        assert_eq!(r, 0x0001);
        assert_eq!(f, 0);
    }

    #[test]
    fn adc_carries_out() {
        let (r, f) = adc(0xFFFF, 0x0000, 1);
        assert_eq!(r, 0x0000);
        assert_eq!(f, Flags::CF | Flags::ZF | Flags::AF | Flags::PF);
    }

    #[test]
    fn adc_signed_overflow_accounts_for_carry() {
        let (r, f) = adc(0x7FFF, 0x0000, 1);
        assert_eq!(r, 0x8000);
        assert_eq!(f, Flags::SF | Flags::OF | Flags::AF | Flags::PF);
    }

    #[test]
    fn sbb_subtracts_borrow() {
        let (r, f) = sbb(0x0000, 0x0000, 1);
        assert_eq!(r, 0xFFFF);
        assert_eq!(f, Flags::CF | Flags::SF | Flags::AF | Flags::PF);
    }

    #[test]
    fn sbb_borrow_is_covered_by_minuend() {
        let (r, f) = sbb(0x0001, 0x0000, 1);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::ZF | Flags::PF);
    }

    #[test]
    fn sbb_signed_overflow() {
        let (r, f) = sbb(0x8000, 0x0000, 1);
        assert_eq!(r, 0x7FFF);
        assert_eq!(f, Flags::AF | Flags::PF | Flags::OF);
    }

    #[test]
    fn adc8_adds_carry_in() {
        let (r, f) = adc8(0xFF, 0x00, 1);
        assert_eq!(r, 0x00);
        assert_eq!(f, Flags::CF | Flags::ZF | Flags::AF | Flags::PF);
    }

    #[test]
    fn adc8_signed_overflow_accounts_for_carry() {
        let (r, f) = adc8(0x7F, 0x00, 1);
        assert_eq!(r, 0x80);
        assert_eq!(f, Flags::SF | Flags::OF | Flags::AF);
    }

    #[test]
    fn sbb8_subtracts_borrow() {
        let (r, f) = sbb8(0x00, 0x00, 1);
        assert_eq!(r, 0xFF);
        assert_eq!(f, Flags::CF | Flags::SF | Flags::AF | Flags::PF);
    }

    #[test]
    fn sbb8_signed_overflow() {
        let (r, f) = sbb8(0x80, 0x00, 1);
        assert_eq!(r, 0x7F);
        assert_eq!(f, Flags::AF | Flags::OF);
    }

    #[test]
    fn or_combines_bits_and_clears_carry_and_overflow() {
        let (r, f) = or(0x0F00, 0x0001);
        assert_eq!(r, 0x0F01);
        // CF/AF/OF cleared; 0x0F01 is positive with odd low-byte parity, so
        // no status flags either.
        assert_eq!(f, 0);
    }

    #[test]
    fn and_sets_zf_and_pf_on_zero_result() {
        let (r, f) = and(0x00FF, 0xFF00);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::ZF | Flags::PF);
    }

    #[test]
    fn and_sets_sf_from_bit_fifteen() {
        let (r, f) = and(0xFFFF, 0x8001);
        assert_eq!(r, 0x8001);
        assert_eq!(f, Flags::SF);
    }

    #[test]
    fn xor_zeroes_a_value_with_zf_and_pf() {
        let (r, f) = xor(0x1234, 0x1234);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::ZF | Flags::PF);
    }

    #[test]
    fn and8_sets_sf_from_bit_seven() {
        let (r, f) = and8(0xFF, 0x80);
        assert_eq!(r, 0x80);
        assert_eq!(f, Flags::SF);
    }

    #[test]
    fn or8_sets_zf_and_pf_on_zero_result() {
        let (r, f) = or8(0x00, 0x00);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::ZF | Flags::PF);
    }

    #[test]
    fn xor8_sets_no_flags_for_a_positive_odd_result() {
        let (r, f) = xor8(0x0F, 0x0E);
        assert_eq!(r, 0x01);
        assert_eq!(f, 0);
    }

    #[test]
    fn inc_settable_flags_are_exactly_add_inc_one() {
        // 0xFF wraps the low byte → AF out of bit 3; 0x7F crosses +127 → OF.
        // CF is stripped by the helper itself (INC never writes it).
        let (r, f) = inc(0x00FF);
        assert_eq!(r, 0x0100);
        assert_eq!(f, Flags::AF | Flags::PF);
        let (r, f) = inc(0x7FFF);
        assert_eq!(r, 0x8000);
        assert_eq!(f, Flags::SF | Flags::OF | Flags::AF | Flags::PF);
        let (r, f) = inc(0xFFFF);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::ZF | Flags::AF | Flags::PF);
    }

    #[test]
    fn dec_settable_flags_are_exactly_sub_inc_one() {
        // 0x8000 crosses −32768 → OF; 0x0010 borrows from the low nibble → AF.
        let (r, f) = dec(0x0000);
        assert_eq!(r, 0xFFFF);
        assert_eq!(f, Flags::SF | Flags::AF | Flags::PF);
        let (r, f) = dec(0x8000);
        assert_eq!(r, 0x7FFF);
        assert_eq!(f, Flags::AF | Flags::PF | Flags::OF);
    }

    #[test]
    fn inc8_settable_flags_are_exactly_add_inc_one() {
        let (r, f) = inc8(0xFF);
        assert_eq!(r, 0);
        assert_eq!(f, Flags::ZF | Flags::AF | Flags::PF);
        let (r, f) = inc8(0x7F);
        assert_eq!(r, 0x80);
        assert_eq!(f, Flags::SF | Flags::OF | Flags::AF);
    }

    #[test]
    fn dec8_settable_flags_are_exactly_sub_inc_one() {
        let (r, f) = dec8(0x00);
        assert_eq!(r, 0xFF);
        assert_eq!(f, Flags::SF | Flags::AF | Flags::PF);
        let (r, f) = dec8(0x80);
        assert_eq!(r, 0x7F);
        assert_eq!(f, Flags::AF | Flags::OF);
    }
}
