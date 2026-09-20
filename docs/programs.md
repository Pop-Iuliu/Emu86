# Program tests

Each `.asm` is restricted to `cpu 8086` (NASM rejects 386+ mnemonics) and to
the opcodes emu86 currently decodes: `MOV reg, imm` (`B0+r`/`B8+r`),
`ADD/SUB r/m16, imm` (`81 /0`, `81 /5`, `83 /0`, `83 /5`), `MOV` between
r/m and reg for byte and word (`88`–`8B`), `ADD AX, imm16` (`05 iw`),
`SUB AX, imm16` (`2D iw`), `HLT` (`F4`).
NASM is invoked with `-O0` so `add ax, imm` assembles to the canonical
`05 iw` form rather than the r/m form `83 /0`.

Binaries (`.bin`) are committed flat images loaded at reset state
`CS:IP = FFFF:0000` (physical `0xFFFF0`). CI assembles every `.asm` with
`just asm` and fails if a committed `.bin` differs (`git diff`), proving the
sources and binaries never drift.

## Expected results

Flag words include the 8086 static bits (`0xF002`: bit 1 and the top nibble
always read as 1).

| Program       | Behavior under test        | Final `AX` | Final flags | Why (reference)                                                                                     |
| ------------- | -------------------------- | ---------- | ----------- | --------------------------------------------------------------------------------------------------- |
| `arith`       | ordinary addition          | `0003`     | `F006`      | `1+2=3`; PF set on even parity of low byte (3 = two 1-bits) — Intel SDM Vol.1, flag conventions      |
| `carry`       | unsigned carry + zero      | `0000`     | `F057`      | `0xFFFF+1` wraps to 0: CF (carry out), ZF (result 0), AF (carry out of bit 3), PF (0x00, even)       |
| `overflow`    | signed overflow            | `8000`     | `F896`      | `0x7FFF+1` crosses +32767 → OF set, SF set; CF clear (Intel SDM Vol.1 §5.1.3 signed-integer overflow) |
| `underflow`   | subtraction underflow      | `FFFF`     | `F097`      | `0-1` borrows: CF set (borrow), SF set, OF clear (−1 is representable), AF set                       |
| `wrap`        | 1 MiB fetch/load wrapping  | `1234`     | `F002`      | program's last 3 bytes straddle `0xFFFFF`→`0x0`; 8086 has no A20 gate, addresses wrap (see ADR-0001)  |
| `memory`      | browser demo + RAM roundtrip | `BEEF`     | `F093`      | `0xBEEF` stored at `DS:0020`, `+0x1111` → `0xD000`, `byte -1` sign-extends to `0xFFFF` → borrow; result loaded into `CX` = `0xD001` |

`wrap` byte layout: 16 bytes fill `0xFFFF0..0xFFFFF` exactly; the
immediate of `MOV AX, 0x1234` and the `HLT` wrap to `0x0000..0x0001`.

## The `memory` demonstration

The single demo program shared by the core fixtures, the WASM binding
(`demo_program()` embeds the committed `memory.bin`) and the browser UI.
Data lives at the dedicated location `DS:0020` (`DS = 0` at reset).

| After              | ip       | State                                                                                      |
| ------------------ | -------- | ------------------------------------------------------------------------------------------ |
| `mov bx, 0x0020`   | `0003`   | `BX = 0020` — data pointer                                                                 |
| `mov ax, 0xBEEF`   | `0006`   | recognizable value                                                                          |
| `mov [bx], ax`     | `0008`   | word `BEEF` at physical `00020` (bytes `EF BE`)                                             |
| `add word [bx], 0x1111` | `000C` | `0xBEEF + 0x1111 = 0xD000`; flags `F002` → `F096` (SF, AF, PF set — `0xF+0x1` nibble carry) |
| `sub word [bx], byte -1` | `000F` | imm8 `0xFF` sign-extends to `0xFFFF`; `0xD000 - 0xFFFF` borrows → `0xD001`, flags `F093` (CF set, PF cleared, SF kept) |
| `mov cx, [bx]`     | `0011`   | `CX = D001` reads the result back; `AX` still `BEEF`                                        |
| `hlt`              | `0012`   | halted, Step disabled in the UI                                                             |

Flag story: the ADD turns SF/AF/PF on; the SUB of a negative sign-extended
immediate clears PF (result low byte `0x01` is odd parity) and sets CF —
subtracting −1 borrows because the immediate is the full `0xFFFF`, not
`0x00FF`.

Encodings (verified by `memory_program_uses_planned_encodings`, offsets
matter for the final `ip`): store `89 07`, add `81 07 11 11` (imm16 form),
sub `83 2F FF` (imm8 form), load `8B 0F`, hlt `F4` — 18 bytes total.

## Replay determinism

`replay_produces_identical_results` runs every program, resets, reloads and
runs again, and requires both final snapshots to be equal — reset state is a
single source of truth (see `Cpu::reset`).
