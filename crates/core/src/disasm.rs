//! Static disassembly for the implemented 8086 subset.
//!
//! Decoding is purely syntactic — it reads bytes from memory and never
//! touches registers, so a listing stays stable while the CPU runs. The
//! `run`/`step` semantics are mirrored exactly: unknown bytes render as
//! `db` one-byte pseudo-instructions, and lengths match what the CPU
//! consumes (including the `81`/`83` split of disp then immediate).
//!
//! Text follows NASM conventions: lowercase mnemonics, `0x`-prefixed hex
//! for word-size immediates and displacements, signed decimal for
//! byte-size displacements and sign-extended immediates, and `[bp]`-style
//! effective addresses with the default segment implied by the base
//! register. Relative jumps render as absolute offset targets, the way
//! NASM labels would appear.
//!
//! Group-1 mnemonics (`add` / `or` / `adc` / `sbb` / `and` / `sub` / `xor` /
//! `cmp`) are rendered for the operand-carrier families too: `00`–`3B` bytes
//! x0–x3 (register/memory, ordered by the direction bit), the
//! accumulator-immediate slots x4/x5 (`04`–`3D`, fixed AL/AX), and the
//! ModRM group bytes `80` / `81` / `83`. The `83` immediate renders as
//! signed decimal (a hand-written `0xFF` reads as `-1`) to match NASM,
//! while the other immediates stay `0x`-prefixed hex. INC/DEC covers
//! `40`–`4F` and `FE`/`FF /0 /1` (with a `word`/`byte` size hint for
//! memory operands); the short jump family `70`–`7F` and the loop family
//! `E0`–`E2` render their mnemonics as absolute offset targets.

use crate::mem::{Memory, MEM_SIZE};

const W16: [&str; 8] = ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di"];
const B8: [&str; 8] = ["al", "cl", "dl", "bl", "ah", "ch", "dh", "bh"];
const RM_BASE: [&str; 8] = ["bx+si", "bx+di", "bp+si", "bp+di", "si", "di", "bp", "bx"];
const SEG_NAME: [&str; 4] = ["es", "cs", "ss", "ds"];
const ALU_MNEM: [&str; 8] = ["add", "or", "adc", "sbb", "and", "sub", "xor", "cmp"];
const JCC_MNEM: [&str; 16] = [
    "jo", "jno", "jb", "jae", "je", "jne", "jbe", "ja", "js", "jns", "jp", "jnp", "jl", "jge",
    "jle", "jg",
];
const LOOP_MNEM: [&str; 3] = ["loopne", "loope", "loop"];

/// Upper bound on instructions per call, so a bogus `max` from the UI
/// cannot stall the worker.
pub const MAX_INSNS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Insn {
    /// Offset within the disassembled segment.
    pub off: u16,
    /// Linear (physical) address of the first byte.
    pub linear: u32,
    pub bytes: Vec<u8>,
    pub text: String,
}

/// Disassembles up to `max` instructions starting at `seg`:`off`, wrapping
/// the offset at 16 bits and the linear address at 1 MiB.
pub fn disassemble(mem: &Memory, seg: u16, off: u16, max: usize) -> Vec<Insn> {
    let mut insns = Vec::new();
    let mut off = off;
    for _ in 0..max.min(MAX_INSNS) {
        let linear = Memory::linear(seg, off) as u32;
        let cursor = std::cell::Cell::new(0usize);
        let mut fetch8 = || {
            let b = mem.read((linear as usize + cursor.get()) & (MEM_SIZE - 1));
            cursor.set(cursor.get() + 1);
            b
        };
        let ip_after = || off.wrapping_add(cursor.get() as u16) as u32;
        let op = fetch8();
        let Some(text) = decode(op, &mut fetch8, &ip_after) else {
            insns.push(Insn {
                off,
                linear,
                bytes: vec![op],
                text: format!("db 0x{op:02X}"),
            });
            off = off.wrapping_add(1);
            continue;
        };
        let bytes: Vec<u8> = (0..cursor.get())
            .map(|i| mem.read((linear as usize + i) & (MEM_SIZE - 1)))
            .collect();
        insns.push(Insn {
            off,
            linear,
            bytes,
            text,
        });
        off = off.wrapping_add(cursor.get() as u16);
    }
    insns
}

type Fetch8<'a> = &'a mut (dyn FnMut() -> u8 + 'a);

/// Decodes one instruction; `ip_after` reports the 16-bit offset just past
/// the bytes consumed so far, used to resolve relative jump targets.
fn decode(op: u8, fetch8: Fetch8<'_>, ip_after: &dyn Fn() -> u32) -> Option<String> {
    match op {
        0xB8..=0xBF => Some(format!(
            "mov {}, 0x{:04X}",
            W16[(op - 0xB8) as usize],
            fetch16(fetch8)
        )),
        0xB0..=0xB7 => Some(format!(
            "mov {}, 0x{:02X}",
            B8[(op - 0xB0) as usize],
            fetch8()
        )),
        // Base ALU families: op = (opcode >> 3) & 7, direction = bit 1,
        // width = bit 0. Bytes x4..x7 are the accumulator slots.
        op @ 0x00..=0x3B if op & 0b100 == 0 => {
            let mnem = ALU_MNEM[((op >> 3) & 0b111) as usize];
            let (md, reg, rm) = modrm_fields(fetch8());
            let wide = op & 1 == 1;
            let reg_text = (if wide { W16[reg] } else { B8[reg] }).to_string();
            let rm_text = rm_text(md, rm, wide, fetch8)?;
            if op & 2 == 0 {
                Some(format!("{mnem} {rm_text}, {reg_text}"))
            } else {
                Some(format!("{mnem} {reg_text}, {rm_text}"))
            }
        }
        op @ 0x00..=0x3F if op & 0b110 == 0b100 => {
            let mnem = ALU_MNEM[((op >> 3) & 0b111) as usize];
            if op & 1 == 1 {
                Some(format!("{mnem} ax, 0x{:04X}", fetch16(fetch8)))
            } else {
                Some(format!("{mnem} al, 0x{:02X}", fetch8()))
            }
        }
        0x40..=0x4F => {
            let (mnem, i) = if op < 0x48 {
                ("inc", op - 0x40)
            } else {
                ("dec", op - 0x48)
            };
            Some(format!("{mnem} {}", W16[i as usize]))
        }
        0x70..=0x7F => {
            let disp = fetch8() as i8 as u16;
            let target = ip_after().wrapping_add(disp as u32) & 0xFFFF;
            Some(format!(
                "{} 0x{:04X}",
                JCC_MNEM[(op - 0x70) as usize],
                target
            ))
        }
        0xE0..=0xE2 => {
            let disp = fetch8() as i8 as u16;
            let target = ip_after().wrapping_add(disp as u32) & 0xFFFF;
            Some(format!(
                "{} 0x{:04X}",
                LOOP_MNEM[(op - 0xE0) as usize],
                target
            ))
        }
        0x50..=0x57 => Some(format!("push {}", W16[(op - 0x50) as usize])),
        0x58..=0x5F => Some(format!("pop {}", W16[(op - 0x58) as usize])),
        0x06 | 0x0E | 0x16 | 0x1E => Some(format!("push {}", SEG_NAME[(op >> 3) as usize & 3])),
        0x07 | 0x0F | 0x17 | 0x1F => Some(format!("pop {}", SEG_NAME[(op >> 3) as usize & 3])),
        0x9C => Some("pushf".to_string()),
        0x9D => Some("popf".to_string()),
        0xE8 => {
            let disp = fetch16(fetch8);
            let target = ip_after().wrapping_add(disp as u32) & 0xFFFF;
            Some(format!("call 0x{target:04X}"))
        }
        0xC2 => Some(format!("ret 0x{:04X}", fetch16(fetch8))),
        0xC3 => Some("ret".to_string()),
        0xE9 | 0xEB => {
            let disp = if op == 0xE9 {
                fetch16(fetch8)
            } else {
                fetch8() as i8 as u16
            };
            let target = ip_after().wrapping_add(disp as u32) & 0xFFFF;
            Some(format!("jmp 0x{target:04X}"))
        }
        0xF4 => Some("hlt".to_string()),
        0x80 | 0x81 | 0x83 => {
            let modrm = fetch8();
            let mnem = ALU_MNEM[((modrm >> 3) & 0b111) as usize];
            let (md, _reg, rm) = modrm_fields(modrm);
            let wide = op != 0x80;
            let rm_text = rm_text(md, rm, wide, fetch8)?;
            let imm = match op {
                0x81 => format!("0x{:04X}", fetch16(fetch8)),
                0x83 => format!("{}", fetch8() as i8),
                _ => format!("0x{:02X}", fetch8()),
            };
            Some(format!("{mnem} {rm_text}, {imm}"))
        }
        0xFE | 0xFF => {
            let (md, field, rm) = modrm_fields(fetch8());
            let wide = op == 0xFF;
            match field {
                0 | 1 => {
                    let mnem = if field == 0 { "inc" } else { "dec" };
                    if md == 3 {
                        let reg_text = if wide {
                            W16[rm as usize]
                        } else {
                            B8[rm as usize]
                        };
                        Some(format!("{mnem} {reg_text}"))
                    } else {
                        let size = if wide { "word" } else { "byte" };
                        let rm_text = rm_text(md, rm, wide, fetch8)?;
                        Some(format!("{mnem} {size} {rm_text}"))
                    }
                }
                _ => None,
            }
        }
        0x88..=0x8B => {
            let wide = op & 1 == 1;
            let reg_is_src = op & 2 == 0;
            let (md, reg, rm) = modrm_fields(fetch8());
            let reg_text = (if wide { W16[reg] } else { B8[reg] }).to_string();
            let rm_text = rm_text(md, rm, wide, fetch8)?;
            if reg_is_src {
                Some(format!("mov {rm_text}, {reg_text}"))
            } else {
                Some(format!("mov {reg_text}, {rm_text}"))
            }
        }
        _ => None,
    }
}

fn modrm_fields(modrm: u8) -> (u8, usize, u8) {
    (modrm >> 6, ((modrm >> 3) & 0b111) as usize, modrm & 0b111)
}

fn rm_text(md: u8, rm: u8, wide: bool, fetch8: Fetch8<'_>) -> Option<String> {
    match md {
        3 => Some(
            (if wide {
                W16[rm as usize]
            } else {
                B8[rm as usize]
            })
            .to_string(),
        ),
        0 if rm == 6 => Some(format!("[0x{:04X}]", fetch16(fetch8))),
        _ => {
            let base = RM_BASE[rm as usize];
            let disp = match md {
                1 => signed_disp(fetch8() as i8 as i32, false),
                2 => signed_disp(fetch16(fetch8) as i16 as i32, true),
                _ => String::new(),
            };
            Some(if disp.is_empty() {
                format!("[{base}]")
            } else {
                format!("[{base}{disp}]")
            })
        }
    }
}

fn signed_disp(v: i32, hex: bool) -> String {
    if v == 0 {
        String::new()
    } else if v > 0 {
        if hex {
            format!("+0x{v:X}")
        } else {
            format!("+{v}")
        }
    } else if hex {
        format!("-0x{:X}", -v)
    } else {
        format!("-{}", -v)
    }
}

fn fetch16(fetch8: Fetch8<'_>) -> u16 {
    let lo = fetch8() as u16;
    (fetch8() as u16) << 8 | lo
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem_with_at(addr: usize, bytes: &[u8]) -> Memory {
        let mut mem = Memory::new();
        mem.load(addr, bytes);
        mem
    }

    fn texts(bytes: &[u8], count: usize) -> Vec<String> {
        disassemble(&mem_with_at(0, bytes), 0x0000, 0, count)
            .into_iter()
            .map(|i| i.text)
            .collect()
    }

    #[test]
    fn mov_immediate_word_and_byte() {
        assert_eq!(
            texts(&[0xB8, 0x34, 0x12, 0xB3, 0x5A], 2),
            vec!["mov ax, 0x1234", "mov bl, 0x5A"]
        );
    }

    #[test]
    fn accumulator_arithmetic() {
        assert_eq!(
            texts(&[0x05, 0x00, 0x10, 0x2D, 0x01, 0x00, 0x3D, 0xFF, 0xFF], 3),
            vec!["add ax, 0x1000", "sub ax, 0x0001", "cmp ax, 0xFFFF"]
        );
    }

    #[test]
    fn mov_between_register_and_memory() {
        assert_eq!(
            texts(
                &[0x89, 0x07, 0x8B, 0x0F, 0x88, 0x10, 0x8A, 0x1E, 0x34, 0x12],
                4
            ),
            vec![
                "mov [bx], ax",
                "mov cx, [bx]",
                "mov [bx+si], dl",
                "mov bl, [0x1234]",
            ]
        );
    }

    #[test]
    fn alu_immediate_on_memory_operands() {
        assert_eq!(
            texts(
                &[0x81, 0x47, 0x02, 0x00, 0x10, 0x83, 0x2F, 0xFF, 0x83, 0x3F, 0x00],
                3
            ),
            vec!["add [bx+2], 0x1000", "sub [bx], -1", "cmp [bx], 0",]
        );
    }

    #[test]
    fn jumps_render_absolute_offset_targets() {
        // Four adjacent jumps: targets are pure arithmetic, they need not
        // point at decodable instructions.
        //   0: EB 03     after=2 -> 5
        //   2: 74 FC     after=4 -> 0
        //   4: 75 FE     after=6 -> 4
        //   6: E9 F6 FF  after=9 -> 0xFFFF (9 + 0xFFF6 lands exactly on top)
        assert_eq!(
            texts(&[0xEB, 0x03, 0x74, 0xFC, 0x75, 0xFE, 0xE9, 0xF6, 0xFF], 4),
            vec!["jmp 0x0005", "je 0x0000", "jne 0x0004", "jmp 0xFFFF"]
        );
    }

    #[test]
    fn hlt_and_unknown_bytes() {
        // 0x60 is not an 8086 opcode (PUSHA is 186+). 0xFF would now start
        // the INC/DEC ModRM group, so the second filler is another 0x60.
        assert_eq!(
            texts(&[0xF4, 0x60, 0x60], 3),
            vec!["hlt", "db 0x60", "db 0x60"]
        );
    }

    #[test]
    fn addressing_forms_cover_all_modes() {
        // 81 /0 (ADD) with each addressing mode.
        assert_eq!(
            texts(
                &[
                    0x81, 0x00, 0x00, 0x10, // add [bx+si], 0x1000
                    0x81, 0x41, 0x02, 0x00, 0x10, // add [bx+di+2], 0x1000
                    0x81, 0x82, 0x34, 0x12, 0x00, 0x10, // add [bp+si+0x1234], 0x1000
                    0x81, 0x06, 0x00, 0x10, 0x00, 0x10, // add [0x1000], 0x1000
                    0x81, 0xC3, 0x01, 0x00, // add bx, 0x0001
                ],
                5,
            ),
            vec![
                "add [bx+si], 0x1000",
                "add [bx+di+2], 0x1000",
                "add [bp+si+0x1234], 0x1000",
                "add [0x1000], 0x1000",
                "add bx, 0x0001",
            ]
        );
    }

    #[test]
    fn negative_displacements_render_signed() {
        assert_eq!(
            texts(&[0x89, 0x47, 0xFE, 0x8B, 0x42, 0x00], 2),
            vec!["mov [bx-2], ax", "mov ax, [bp+si]"]
        );
    }

    #[test]
    fn listing_walks_offsets_and_linear_addresses() {
        let mem = mem_with_at(0xFFFF0, &[0xB8, 0x01, 0x00, 0xF4, 0x90]);
        let insns = disassemble(&mem, 0xFFFF, 0x0000, 2);
        assert_eq!(insns.len(), 2);
        assert_eq!(insns[0].off, 0x0000);
        assert_eq!(insns[0].linear, 0xFFFF0);
        assert_eq!(insns[0].bytes, vec![0xB8, 0x01, 0x00]);
        assert_eq!(insns[1].off, 0x0003);
        assert_eq!(insns[1].linear, 0xFFFF3);
        assert_eq!(insns[1].bytes, vec![0xF4]);
    }

    #[test]
    fn offsets_wrap_at_16_bits() {
        // Two one-byte instructions starting at 0xFFFE walk into 0x0000.
        // 0xFFFF:FFFE is physical 0xFFEE (the 1 MiB wrap) — see the sibling
        // test below.
        let mem = mem_with_at(0xFFEE, &[0x90, 0x90]);
        let insns = disassemble(&mem, 0xFFFF, 0xFFFE, 2);
        assert_eq!(insns[0].off, 0xFFFE);
        assert_eq!(insns[1].off, 0xFFFF);
    }

    #[test]
    fn linear_addresses_wrap_at_one_mebibyte() {
        // FFFF:FFFF has no A20 gate: the physical address wraps to 0xFFEF.
        let mem = mem_with_at(0xFFEF, &[0x90, 0x90]);
        let insns = disassemble(&mem, 0xFFFF, 0xFFFF, 2);
        assert_eq!(insns[0].linear, 0xFFEF);
        assert_eq!(insns[1].linear, 0xFFFF0);
    }

    #[test]
    fn countdown_demo_disassembles_completely() {
        let bytes = include_bytes!("../../../tests/programs/countdown.bin");
        let mem = mem_with_at(0xFFFF0, bytes);
        let listing: Vec<String> = disassemble(&mem, 0xFFFF, 0x0000, 9)
            .into_iter()
            .map(|i| i.text)
            .collect();
        assert_eq!(
            listing,
            vec![
                "mov bx, 0x0020",
                "mov ax, 0x0003",
                "mov cx, 0xFFFF",
                "mov [bx], ax",
                "sub [bx], 1",
                "cmp [bx], 0",
                "jne 0x000B",
                "mov cx, [bx]",
                "hlt",
            ]
        );
    }

    #[test]
    fn loopreg_demo_disassembles_completely() {
        let bytes = include_bytes!("../../../tests/programs/loopreg.bin");
        let mem = mem_with_at(0xFFFF0, bytes);
        let listing: Vec<String> = disassemble(&mem, 0xFFFF, 0x0000, 5)
            .into_iter()
            .map(|i| i.text)
            .collect();
        assert_eq!(
            listing,
            vec![
                "mov cx, 0x0003",
                "mov ax, 0x0000",
                "add ax, 0x0002",
                "loop 0x0006",
                "hlt",
            ]
        );
    }

    #[test]
    fn db_is_one_byte_and_does_not_desynchronize() {
        // 0x60 is unknown; the listing skips exactly one byte and recovers.
        assert_eq!(
            texts(&[0x60, 0xB8, 0x34, 0x12], 2),
            vec!["db 0x60", "mov ax, 0x1234"]
        );
    }

    #[test]
    fn byte_alu_immediate_forms() {
        assert_eq!(
            texts(&[0x80, 0x07, 0x10, 0x80, 0xC3, 0xFF, 0x80, 0x3F, 0x05], 3),
            vec!["add [bx], 0x10", "add bl, 0xFF", "cmp [bx], 0x05"]
        );
    }

    #[test]
    fn byte_alu_immediate_with_disp_keeps_listing_in_sync() {
        assert_eq!(
            texts(&[0x80, 0x47, 0xFE, 0x01, 0xF4], 2),
            vec!["add [bx-2], 0x01", "hlt"]
        );
    }

    #[test]
    fn byte_alu_immediate_consumes_exactly_its_bytes() {
        let insns = disassemble(
            &mem_with_at(0, &[0x80, 0x47, 0xFE, 0x01]),
            0x0000,
            0x0000,
            3,
        );
        assert_eq!(insns[0].bytes, vec![0x80, 0x47, 0xFE, 0x01]);
        assert_eq!(insns[1].off, 0x0004);
    }

    #[test]
    fn push_pop_registers_render_nasm_names() {
        assert_eq!(
            texts(&[0x50, 0x54, 0x57, 0x58, 0x5C, 0x5F], 6),
            vec!["push ax", "push sp", "push di", "pop ax", "pop sp", "pop di",]
        );
    }

    #[test]
    fn segment_and_flag_pushes_render() {
        assert_eq!(
            texts(
                &[0x06, 0x07, 0x0E, 0x0F, 0x16, 0x17, 0x1E, 0x1F, 0x9C, 0x9D],
                10
            ),
            vec![
                "push es", "pop es", "push cs", "pop cs", "push ss", "pop ss", "push ds", "pop ds",
                "pushf", "popf",
            ]
        );
    }

    #[test]
    fn call_renders_absolute_target_and_ret_variants() {
        //   0: E8 03 00  after=3 -> 6
        //   3: C2 04 00  ret 0x0004
        //   6: C3        ret
        assert_eq!(
            texts(&[0xE8, 0x03, 0x00, 0xC2, 0x04, 0x00, 0xC3], 3),
            vec!["call 0x0006", "ret 0x0004", "ret"]
        );
    }

    #[test]
    fn call_bytes_match_what_step_consumes() {
        let insns = disassemble(
            &mem_with_at(0, &[0xE8, 0x03, 0x00, 0xC2, 0x04, 0x00]),
            0x0000,
            0x0000,
            2,
        );
        assert_eq!(insns[0].bytes, vec![0xE8, 0x03, 0x00]);
        assert_eq!(insns[1].off, 0x0003);
        assert_eq!(insns[1].bytes, vec![0xC2, 0x04, 0x00]);
    }

    // -----------------------------------------------------------------------
    // Operand-carrier families: 00–3B (x0–x3) with direction ordering, the
    // accumulator slots (04–3D), INC/DEC (40–4F, FE/FF /0 /1), the full Jcc
    // set (70–7F) and the loop family (E0–E2). Lengths are pinned next to
    // the renderings.
    // -----------------------------------------------------------------------

    #[test]
    fn alu_rm_reg_between_registers_orders_by_direction() {
        assert_eq!(
            texts(&[0x01, 0xC8, 0x03, 0xC8, 0x00, 0xC8, 0x02, 0xC8], 4),
            vec!["add ax, cx", "add cx, ax", "add al, cl", "add cl, al",]
        );
    }

    #[test]
    fn alu_rm_reg_with_memory_orders_by_direction() {
        assert_eq!(
            texts(&[0x01, 0x07, 0x03, 0x07, 0x29, 0x07, 0x2B, 0x07], 4),
            vec![
                "add [bx], ax",
                "add ax, [bx]",
                "sub [bx], ax",
                "sub ax, [bx]",
            ]
        );
    }

    #[test]
    fn accumulator_family_renders_all_eight_fields() {
        let byte_ops = [0x04u8, 0x0C, 0x14, 0x1C, 0x24, 0x2C, 0x34, 0x3C];
        let word_ops = [0x05u8, 0x0D, 0x15, 0x1D, 0x25, 0x2D, 0x35, 0x3D];
        let mnems = ["add", "or", "adc", "sbb", "and", "sub", "xor", "cmp"];
        let mut bytes = Vec::new();
        for &op in &byte_ops {
            bytes.extend([op, 0x5A]);
        }
        for &op in &word_ops {
            bytes.extend([op, 0x34, 0x12]);
        }
        let mut want = Vec::new();
        for name in mnems {
            want.push(format!("{name} al, 0x5A"));
        }
        for name in mnems {
            want.push(format!("{name} ax, 0x1234"));
        }
        assert_eq!(texts(&bytes, 16), want);
    }

    #[test]
    fn inc_dec_r16_render_nasm_names() {
        assert_eq!(
            texts(&[0x40, 0x47, 0x48, 0x4F], 4),
            vec!["inc ax", "inc di", "dec ax", "dec di"]
        );
    }

    #[test]
    fn inc_dec_rm_render_size_hint_for_memory_operands() {
        assert_eq!(
            texts(&[0xFE, 0x07, 0xFF, 0x07, 0xFE, 0x0F, 0xFF, 0xC8], 4),
            vec!["inc byte [bx]", "inc word [bx]", "dec byte [bx]", "dec ax",]
        );
    }

    #[test]
    fn jcc_family_renders_all_sixteen_mnemonics() {
        const NAMES: [&str; 16] = [
            "jo", "jno", "jb", "jae", "je", "jne", "jbe", "ja", "js", "jns", "jp", "jnp", "jl",
            "jge", "jle", "jg",
        ];
        let mut bytes = Vec::new();
        for i in 0u8..16 {
            bytes.extend([0x70 + i, 0x00]);
        }
        let got = texts(&bytes, 16);
        for (i, name) in NAMES.iter().enumerate() {
            assert_eq!(
                got[i],
                format!("{name} 0x{:04X}", 2 * (i as u16 + 1)),
                "index {i}"
            );
        }
    }

    #[test]
    fn loop_family_renders_absolute_targets() {
        // 0: E2 FC → after = 2, target 0xFFFE | 2: E1 00 → 4 | 4: E0 02 → 8.
        assert_eq!(
            texts(&[0xE2, 0xFC, 0xE1, 0x00, 0xE0, 0x02], 3),
            vec!["loop 0xFFFE", "loope 0x0004", "loopne 0x0008"]
        );
    }

    #[test]
    fn operand_carrier_lengths_match_their_bytes() {
        let cases: &[(&[u8], usize)] = &[
            (&[0x00, 0x07], 2),             // add byte [bx], al
            (&[0x01, 0x87, 0x00, 0x10], 4), // add word [bx+0x1000], ax
            (&[0x2A, 0x47, 0x02], 3),       // sub al, [bx+2]
            (&[0x04, 0x5A], 2),             // add al, imm8
            (&[0x05, 0x34, 0x12], 3),       // add ax, imm16
            (&[0xFE, 0x47, 0x02], 3),       // inc byte [bx+2]
            (&[0xFF, 0x87, 0x00, 0x10], 4), // inc word [bx+0x1000]
            (&[0xE2, 0x00], 2),             // loop
            (&[0x77, 0xFE], 2),             // ja
        ];
        for &(bytes, want) in cases {
            let insns = disassemble(&mem_with_at(0, bytes), 0x0000, 0, 2);
            assert_eq!(insns[0].bytes, bytes, "{}", sprintf_case(bytes));
            assert_eq!(insns[0].bytes.len(), want);
            assert_eq!(insns[1].off, want as u16);
        }
    }

    fn sprintf_case(bytes: &[u8]) -> String {
        bytes
            .iter()
            .map(|b| format!("{b:02X}"))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
