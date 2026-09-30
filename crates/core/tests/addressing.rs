#![allow(clippy::panic)]

//! Behaviour-level tests for 8086 operand decoding and segmented addressing.
//!
//! Expectations are derived from the 8086 programmer's model (Intel 8086 User's
//! Manual, effective-address and displacement descriptions), not from the
//! implementation under test:
//!   * effective offsets wrap within 16 bits,
//!   * linear addresses wrap within one mebibyte,
//!   * BP-based forms read their segment from SS, everything else from DS,
//!   * the mod=00 / r/m=110 form is a direct address in DS, never [BP].

use emu86_core::cpu::{Cpu, StepError};
use emu86_core::flags::Flags;
use emu86_core::reg::{Reg16, Reg8, Seg};

const CS: u16 = 0xFFFF;
const IP: u16 = 0x0000;

// Base register values shared by the effective-address sweep so each r/m field
// maps to a distinct, easy-to-read offset.
const BX: u16 = 0x1000;
const BP: u16 = 0x2000;
const SI: u16 = 0x0300;
const DI: u16 = 0x0400;

fn loaded(code: &[u8]) -> Cpu {
    let mut cpu = Cpu::new();
    cpu.load_flat(CS, IP, code);
    cpu
}

fn loaded_with_bases(code: &[u8]) -> Cpu {
    let mut cpu = loaded(code);
    cpu.regs.set_reg(Reg16::Bx, BX);
    cpu.regs.set_reg(Reg16::Bp, BP);
    cpu.regs.set_reg(Reg16::Si, SI);
    cpu.regs.set_reg(Reg16::Di, DI);
    cpu
}

#[test]
fn effective_addresses_cover_every_mod_and_r_m_combination() {
    // `89 /0` is MOV r/m16, AX. DS and SS both start at 0, so for these cases
    // the effective offset equals the physical address.
    let cases: &[(u8, &[u8], usize)] = &[
        // mod = 00: register indirect (r/m = 110 is the direct-address exception)
        (0b00_000_000, &[], 0x1300),           // [BX+SI]
        (0b00_000_001, &[], 0x1400),           // [BX+DI]
        (0b00_000_010, &[], 0x2300),           // [BP+SI]
        (0b00_000_011, &[], 0x2400),           // [BP+DI]
        (0b00_000_100, &[], 0x0300),           // [SI]
        (0b00_000_101, &[], 0x0400),           // [DI]
        (0b00_000_110, &[0x00, 0x10], 0x1000), // [0x1000] (direct)
        (0b00_000_111, &[], 0x1000),           // [BX]
        // mod = 01: disp8, added with wrapping
        (0b01_000_000, &[0x05], 0x1305), // [BX+SI+5]
        (0b01_000_001, &[0x05], 0x1405), // [BX+DI+5]
        (0b01_000_010, &[0x05], 0x2305), // [BP+SI+5]
        (0b01_000_011, &[0x05], 0x2405), // [BP+DI+5]
        (0b01_000_100, &[0x05], 0x0305), // [SI+5]
        (0b01_000_101, &[0x05], 0x0405), // [DI+5]
        (0b01_000_110, &[0x05], 0x2005), // [BP+5]
        (0b01_000_111, &[0x05], 0x1005), // [BX+5]
        // mod = 10: disp16
        (0b10_000_000, &[0x05, 0x00], 0x1305), // [BX+SI+5]
        (0b10_000_110, &[0x05, 0x00], 0x2005), // [BP+5]
    ];

    for &(modrm, disp, addr) in cases {
        let mut code = vec![0x89, modrm];
        code.extend_from_slice(disp);
        code.push(0xF4);

        let mut cpu = loaded_with_bases(&code);
        cpu.regs.set_reg(Reg16::Ax, 0xCAFE);
        cpu.step().expect("MOV should execute");

        assert_eq!(cpu.mem.read_word(addr), 0xCAFE, "modrm {modrm:#04x}");
        assert_eq!(cpu.ip, 2 + disp.len() as u16, "modrm {modrm:#04x}");
    }
}

#[test]
fn bp_based_form_uses_ss_segment() {
    // MOV [BP+1], AX -- 8086 defaults base-pointer effective addresses to SS.
    let mut cpu = loaded(&[0x89, 0x46, 0x01, 0xF4]);
    cpu.regs.set_seg(Seg::Ss, 0x2000);
    cpu.regs.set_seg(Seg::Ds, 0x1000);
    cpu.regs.set_reg(Reg16::Bp, 0x0005);
    cpu.regs.set_reg(Reg16::Ax, 0x1234);
    cpu.mem.write_word(0x10006, 0xBBBB); // the DS target that must be left alone

    cpu.step().expect("MOV should execute");

    assert_eq!(cpu.mem.read_word(0x20006), 0x1234);
    assert_eq!(cpu.mem.read_word(0x10006), 0xBBBB);
}

#[test]
fn direct_form_uses_ds_and_ignores_bp() {
    // mod=00, r/m=110 is a direct address in DS -- not [BP].
    let mut cpu = loaded(&[0x89, 0x06, 0x06, 0x00, 0xF4]); // MOV [0x0006], AX
    cpu.regs.set_seg(Seg::Ds, 0x1000);
    cpu.regs.set_seg(Seg::Ss, 0x2000);
    cpu.regs.set_reg(Reg16::Bp, 0x0006);
    cpu.regs.set_reg(Reg16::Ax, 0x1234);
    cpu.mem.write_word(0x20006, 0xAAAA); // would be the [BP] target

    cpu.step().expect("MOV should execute");

    assert_eq!(cpu.mem.read_word(0x10006), 0x1234);
    assert_eq!(cpu.mem.read_word(0x20006), 0xAAAA);
}

#[test]
fn disp8_sign_extends() {
    for &(disp, off) in &[(0x02u8, 0x1302usize), (0xFFu8, 0x12FFusize)] {
        // MOV [BX+SI+disp8], AX
        let mut cpu = loaded_with_bases(&[0x89, 0x40, disp, 0xF4]);
        cpu.regs.set_reg(Reg16::Ax, 0xCAFE);
        cpu.step().expect("MOV should execute");
        assert_eq!(cpu.mem.read_word(off), 0xCAFE, "disp8 {disp:#04x}");
    }
}

#[test]
fn disp16_adds_to_full_base_combination() {
    // MOV CX, [BX+SI+0x1234]
    let mut cpu = loaded_with_bases(&[0x8B, 0x88, 0x34, 0x12, 0xF4]);
    cpu.mem.write_word(0x2534, 0xBEEF);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Cx), 0xBEEF);
}

#[test]
fn imm8_sign_extension_boundaries_for_add() {
    // ADD AX, imm8 (83 /0). AX starts at 0, so the result is sign_ext(imm).
    for &(imm, expect) in &[
        (0x00u8, 0x0000u16),
        (0x01, 0x0001),
        (0x7F, 0x007F),
        (0x80, 0xFF80),
        (0xFF, 0xFFFF),
    ] {
        let mut cpu = loaded(&[0x83, 0xC0, imm, 0xF4]);
        cpu.step().expect("ADD should execute");
        assert_eq!(cpu.regs.reg(Reg16::Ax), expect, "imm8 {imm:#04x}");
    }
}

#[test]
fn imm8_sign_extension_boundaries_for_sub() {
    // SUB AX, imm8 (83 /5). AX starts at 0, so the result is -sign_ext(imm).
    for &(imm, expect) in &[
        (0x00u8, 0x0000u16),
        (0x01, 0xFFFF),
        (0x7F, 0xFF81),
        (0x80, 0x0080),
        (0xFF, 0x0001),
    ] {
        let mut cpu = loaded(&[0x83, 0xE8, imm, 0xF4]);
        cpu.step().expect("SUB should execute");
        assert_eq!(cpu.regs.reg(Reg16::Ax), expect, "imm8 {imm:#04x}");
    }
}

#[test]
fn high_byte_immediate_write_preserves_low_byte() {
    let mut cpu = loaded(&[0xB4, 0x12]); // MOV AH, 0x12
    cpu.regs.set_reg(Reg16::Ax, 0xABCD);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0x12CD);
    assert_eq!(cpu.regs.reg8(Reg8::Al), 0xCD);
}

#[test]
fn byte_mov_into_high_byte_keeps_low_byte() {
    let mut cpu = loaded(&[0x88, 0xDC]); // MOV AH, BL
    cpu.regs.set_reg(Reg16::Ax, 0xABCD);
    cpu.regs.set_reg(Reg16::Bx, 0x5678); // BL = 0x78
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0x78CD);
}

#[test]
fn low_byte_write_preserves_high_byte() {
    let mut cpu = loaded(&[0xB0, 0x34]); // MOV AL, 0x34
    cpu.regs.set_reg(Reg16::Ax, 0xABCD);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0xAB34);
}

#[test]
fn word_load_from_memory_into_register() {
    let mut cpu = loaded(&[0x8B, 0x07, 0xF4]); // MOV AX, [BX]
    cpu.regs.set_reg(Reg16::Bx, 0x0100);
    cpu.mem.write_word(0x0100, 0xBEEF);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0xBEEF);
}

#[test]
fn byte_load_from_memory_into_low_byte() {
    let mut cpu = loaded(&[0x8A, 0x07, 0xF4]); // MOV AL, [BX]
    cpu.regs.set_reg(Reg16::Ax, 0xABCD);
    cpu.regs.set_reg(Reg16::Bx, 0x0100);
    cpu.mem.write(0x0100, 0x99);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0xAB99);
}

#[test]
fn byte_store_to_memory() {
    let mut cpu = loaded(&[0x88, 0x07, 0xF4]); // MOV [BX], AL
    cpu.regs.set_reg(Reg16::Bx, 0x0100);
    cpu.regs.set_reg(Reg16::Ax, 0xABCD);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.mem.read(0x0100), 0xCD);
}

#[test]
fn mov_register_does_not_change_flags() {
    // SUB AX, 1 sets several flags; a following MOV must leave them untouched.
    let mut cpu = loaded(&[0x2D, 0x01, 0x00, 0x89, 0xD8, 0xF4]); // SUB AX,1 ; MOV AX,BX
    cpu.regs.set_reg(Reg16::Bx, 0x1234);
    cpu.step().expect("SUB should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0xFFFF);
    let before = cpu.snapshot().flags;

    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1234);
    assert_eq!(cpu.snapshot().flags, before);
}

#[test]
fn mov_memory_does_not_change_flags() {
    // SUB AX,1 ; MOV [0x1000], BX
    let mut cpu = loaded(&[0x2D, 0x01, 0x00, 0x89, 0x1E, 0x00, 0x10, 0xF4]);
    cpu.regs.set_reg(Reg16::Bx, 0x1234);
    cpu.step().expect("SUB should execute");
    let before = cpu.snapshot().flags;

    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.mem.read_word(0x1000), 0x1234);
    assert_eq!(cpu.snapshot().flags, before);
}

#[test]
fn add_imm16_to_register_sets_flags() {
    let mut cpu = loaded(&[0x05, 0x01, 0x00, 0xF4]); // ADD AX, 1
    cpu.regs.set_reg(Reg16::Ax, 0x7FFF);
    cpu.step().expect("ADD should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0x8000);
    assert!(cpu.flags.get(Flags::OF));
    assert!(cpu.flags.get(Flags::SF));
    assert!(cpu.flags.get(Flags::AF));
    assert!(cpu.flags.get(Flags::PF));
    assert!(!cpu.flags.get(Flags::CF));
    assert!(!cpu.flags.get(Flags::ZF));
}

#[test]
fn sub_imm16_from_register_sets_flags() {
    let mut cpu = loaded(&[0x2D, 0x01, 0x00, 0xF4]); // SUB AX, 1
    cpu.regs.set_reg(Reg16::Ax, 0x8000);
    cpu.step().expect("SUB should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0x7FFF);
    assert!(cpu.flags.get(Flags::OF));
    assert!(cpu.flags.get(Flags::AF));
    assert!(cpu.flags.get(Flags::PF));
    assert!(!cpu.flags.get(Flags::CF));
    assert!(!cpu.flags.get(Flags::SF));
    assert!(!cpu.flags.get(Flags::ZF));
}

#[test]
fn add_imm16_to_memory_sets_flags() {
    let mut cpu = loaded(&[0x81, 0x06, 0x00, 0x10, 0x01, 0x00, 0xF4]); // ADD [0x1000], 1
    cpu.mem.write_word(0x1000, 0x7FFF);
    cpu.step().expect("ADD should execute");
    assert_eq!(cpu.mem.read_word(0x1000), 0x8000);
    assert!(cpu.flags.get(Flags::OF));
    assert!(cpu.flags.get(Flags::SF));
    assert!(cpu.flags.get(Flags::AF));
    assert!(cpu.flags.get(Flags::PF));
    assert!(!cpu.flags.get(Flags::CF));
    assert!(!cpu.flags.get(Flags::ZF));
}

#[test]
fn sub_imm16_from_memory_sets_flags() {
    let mut cpu = loaded(&[0x81, 0x2E, 0x00, 0x10, 0x01, 0x00, 0xF4]); // SUB [0x1000], 1
    cpu.mem.write_word(0x1000, 0x0000);
    cpu.step().expect("SUB should execute");
    assert_eq!(cpu.mem.read_word(0x1000), 0xFFFF);
    assert!(cpu.flags.get(Flags::CF));
    assert!(cpu.flags.get(Flags::SF));
    assert!(cpu.flags.get(Flags::AF));
    assert!(cpu.flags.get(Flags::PF));
    assert!(!cpu.flags.get(Flags::OF));
    assert!(!cpu.flags.get(Flags::ZF));
}

#[test]
fn instruction_lengths_advance_ip() {
    let cases: &[(&[u8], u16)] = &[
        (&[0x89, 0x07, 0xF4], 3),                         // MOV [BX], AX + HLT
        (&[0x89, 0x06, 0x00, 0x10, 0xF4], 5),             // MOV [0x1000], AX
        (&[0x89, 0x47, 0x02, 0xF4], 4),                   // MOV [BX+2], AX
        (&[0x8B, 0x88, 0x34, 0x12, 0xF4], 5),             // MOV CX, [BX+SI+0x1234]
        (&[0x81, 0x06, 0x00, 0x10, 0x01, 0x00, 0xF4], 7), // ADD [0x1000], imm16
        (&[0x83, 0x2E, 0x00, 0x10, 0xFF, 0xF4], 6),       // SUB [0x1000], imm8
        (&[0xB8, 0x34, 0x12, 0xF4], 4),                   // MOV AX, imm16
        (&[0xB4, 0x12, 0xF4], 3),                         // MOV AH, imm8
    ];

    for &(code, end) in cases {
        let mut cpu = loaded(code);
        while !cpu.halted {
            cpu.step().expect("step should succeed");
        }
        assert_eq!(cpu.ip, end, "code {code:02x?}");
    }
}

#[test]
fn effective_offset_wraps_within_16_bits() {
    // [BX+SI] with BX = SI = 0xFFFF -> 0xFFFE, then +2 wraps to 0x0000.
    let mut cpu = loaded(&[0x89, 0x40, 0x02, 0xF4]); // MOV [BX+SI+2], AX
    cpu.regs.set_reg(Reg16::Bx, 0xFFFF);
    cpu.regs.set_reg(Reg16::Si, 0xFFFF);
    cpu.regs.set_reg(Reg16::Ax, 0xCAFE);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.mem.read_word(0x0000), 0xCAFE);
}

#[test]
fn physical_address_wraps_at_one_mebibyte() {
    // DS:0010 = 0xFFFF0 + 0x0010 = 0x100000 -> wraps to 0x00000.
    let mut cpu = loaded(&[0x89, 0x06, 0x10, 0x00, 0xF4]); // MOV [0x0010], AX
    cpu.regs.set_seg(Seg::Ds, 0xFFFF);
    cpu.regs.set_reg(Reg16::Ax, 0xBEEF);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.mem.read(0x00000), 0xEF);
    assert_eq!(cpu.mem.read(0x00001), 0xBE);
}

#[test]
fn instruction_fetch_wraps_at_one_mebibyte() {
    // Opcode at 0xFFFFE: the immediate's high byte spills past the top of
    // memory onto 0x00000, so the fetch must wrap.
    let mut cpu = Cpu::new();
    cpu.load_flat(0xFFFF, 0x000E, &[0xB8, 0x34, 0x12]); // MOV AX, 0x1234
    cpu.ip = 0x000E;
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0x1234);
    assert_eq!(cpu.ip, 0x0011);
}

#[test]
fn word_store_at_top_of_memory_wraps() {
    // MOV [0x000F], AX with DS = 0xFFFF: low byte at 0xFFFFF, high byte at 0x00000.
    let mut cpu = loaded(&[0x89, 0x06, 0x0F, 0x00, 0xF4]);
    cpu.regs.set_seg(Seg::Ds, 0xFFFF);
    cpu.regs.set_reg(Reg16::Ax, 0xBEEF);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.mem.read(0xFFFFF), 0xEF);
    assert_eq!(cpu.mem.read(0x00000), 0xBE);
}

#[test]
fn word_load_at_top_of_memory_wraps() {
    let mut cpu = loaded(&[0x8B, 0x06, 0x0F, 0x00, 0xF4]); // MOV AX, [0x000F]
    cpu.regs.set_seg(Seg::Ds, 0xFFFF);
    cpu.mem.write(0xFFFFF, 0xEF);
    cpu.mem.write(0x00000, 0xBE);
    cpu.step().expect("MOV should execute");
    assert_eq!(cpu.regs.reg(Reg16::Ax), 0xBEEF);
}

#[test]
fn unknown_opcode_is_reported() {
    let mut cpu = loaded(&[0x06]); // PUSH ES: not implemented yet
    assert_eq!(cpu.step(), Err(StepError::UnknownOpcode(0x06)));
    assert_eq!(cpu.ip, 0x0001);
}

#[test]
fn unsupported_group_extension_reports_ip_and_bytes() {
    let mut cpu = loaded(&[0x81, 0xC8, 0x34, 0x12]); // 81 /1 (OR r/m16, imm16)
    assert_eq!(
        cpu.step(),
        Err(StepError::UnsupportedForm {
            ip: 0x0000,
            bytes: vec![0x81, 0xC8],
        })
    );
    assert_eq!(cpu.ip, 0x0002);
}

#[test]
fn group1_extensions_other_than_add_and_sub_are_unsupported() {
    for reg in [1u8, 2, 3, 4, 6, 7] {
        let modrm = 0b11_000_000 | (reg << 3);
        let mut cpu = loaded(&[0x81, modrm, 0x00, 0x00]);
        assert_eq!(
            cpu.step(),
            Err(StepError::UnsupportedForm {
                ip: 0x0000,
                bytes: vec![0x81, modrm],
            }),
            "group field {reg}"
        );
    }
}
