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

use crate::mem::{Memory, MEM_SIZE};

const W16: [&str; 8] = ["ax", "cx", "dx", "bx", "sp", "bp", "si", "di"];
const B8: [&str; 8] = ["al", "cl", "dl", "bl", "ah", "ch", "dh", "bh"];
const RM_BASE: [&str; 8] = ["bx+si", "bx+di", "bp+si", "bp+di", "si", "di", "bp", "bx"];

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
        0x05 => Some(format!("add ax, 0x{:04X}", fetch16(fetch8))),
        0x2D => Some(format!("sub ax, 0x{:04X}", fetch16(fetch8))),
        0x3D => Some(format!("cmp ax, 0x{:04X}", fetch16(fetch8))),
        0x74 | 0x75 => {
            let disp = fetch8() as i8 as u16;
            let mnem = if op == 0x74 { "je" } else { "jne" };
            let target = ip_after().wrapping_add(disp as u32) & 0xFFFF;
            Some(format!("{mnem} 0x{target:04X}"))
        }
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
        0x81 | 0x83 => {
            let modrm = fetch8();
            let mnem = match (modrm >> 3) & 0b111 {
                0 => "add",
                5 => "sub",
                7 => "cmp",
                _ => return None,
            };
            let (md, _reg, rm) = modrm_fields(modrm);
            let rm_text = rm_text(md, rm, true, fetch8)?;
            let imm = if op == 0x81 {
                format!("0x{:04X}", fetch16(fetch8))
            } else {
                format!("{}", fetch8() as i8)
            };
            Some(format!("{mnem} {rm_text}, {imm}"))
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
    fn alu_with_unsupported_operator_is_db() {
        // 0x81 /1 = OR — the CPU raises UnsupportedForm, the listing says db.
        assert_eq!(texts(&[0x81, 0xC8, 0x34, 0x12], 1), vec!["db 0x81"]);
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
        assert_eq!(
            texts(&[0xF4, 0x0F, 0xFF], 3),
            vec!["hlt", "db 0x0F", "db 0xFF"]
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
        let mem = mem_with_at(0xFFFF0, &[0x90, 0x90]);
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
    fn db_is_one_byte_and_does_not_desynchronize() {
        // 0F is unknown; the listing skips exactly one byte and recovers.
        assert_eq!(
            texts(&[0x0F, 0xB8, 0x34, 0x12], 2),
            vec!["db 0x0F", "mov ax, 0x1234"]
        );
    }
}
