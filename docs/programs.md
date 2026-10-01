# Program tests

Each `.asm` is restricted to `cpu 8086` (NASM rejects 386+ mnemonics) and to
the opcodes emu86 currently decodes: `MOV reg, imm` (`B0+r`/`B8+r`),
`ADD/SUB/CMP r/m16, imm` (`81 /0`, `81 /5`, `81 /7`, `83 /0`, `83 /5`,
`83 /7`), `MOV` between r/m and reg for byte and word (`88`–`8B`),
`ADD AX, imm16` (`05 iw`), `SUB/CMP AX, imm16` (`2D iw`, `3D iw`),
`JMP short/near` (`EB`, `E9`), `JE/JNE short` (`74`/`75`), `HLT` (`F4`).
NASM is invoked with `-O0` so `add ax, imm` assembles to the canonical
`05 iw` form rather than the r/m form `83 /0`.

Binaries (`.bin`) are committed flat images loaded at reset state
`CS:IP = FFFF:0000` (physical `0xFFFF0`). The browser **Load binary…** control accepts the
same layout: any non-empty flat image up to 1 MiB, loaded at `FFFF:0000` from reset state
(see README → "Loading a program"). CI assembles every `.asm` with
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
| `memory`      | RAM roundtrip                | `BEEF`     | `F093`      | `0xBEEF` stored at `DS:0020`, `+0x1111` → `0xD000`, `byte -1` sign-extends to `0xFFFF` → borrow; result loaded into `CX` = `0xD001` |
| `loop`        | flags drive a counted loop   | `0006`     | `F046`      | 3× `add ax, 2` / `sub cx, 1` / `cmp cx, 0` with a `jne top` back-edge; the final `cmp` sets ZF, so `jne` falls through to `hlt` — flags decide repeat vs continue |
| `countdown`   | browser demo — terminating countdown | `0003` | `F046`  | `AX = 3` stored at `DS:0020`; `83 /5` decrements the word 3→2→1→0; `83 /7` compare raises ZF at 0; backward `75` branch exits; `CX` (pre-set `FFFF`) loads the final 0 |

`wrap` byte layout: 16 bytes fill `0xFFFF0..0xFFFFF` exactly; the
immediate of `MOV AX, 0x1234` and the `HLT` wrap to `0x0000..0x0001`.

## The countdown demonstration

The browser demo is this program: the WASM binding (`demo_program()`)
embeds the committed `countdown.bin`, and the program tests run the same
fixture. Data lives at `DS:0020` (`DS = 0` at reset); the browser memory
inspector shows the fixed physical window `0x0010–0x002F` (32 bytes)
around that location.

Story: **AX supplies 3 → memory counts 3, 2, 1, 0 → ZF ends the loop →
CX receives 0 → halt.** Starting `CX` at `FFFF` makes the final load
visibly change a register.

| After                   | ip     | State                                                         |
| ----------------------- | ------ | ------------------------------------------------------------- |
| `mov bx, 0x0020`        | `0003` | `BX = 0020` — data pointer                                    |
| `mov ax, 0x0003`        | `0006` | supplies the countdown                                        |
| `mov cx, 0xFFFF`        | `0009` | sentinel, visibly replaced at the end                         |
| `mov [bx], ax`          | `000B` | word `0003` at physical `00020` (bytes `03 00`), flags `F002` |
| `sub word [bx], byte 1` | `000E` | `3 → 2`, flags stay `F002`                                    |
| `cmp word [bx], byte 0` | `0011` | `2 − 0`; ZF clear                                             |
| `jne countdown` (taken) | `000B` | back-edge, disp8 `F8` (−8 from `0013`)                        |
| `sub`, `cmp`, `jne`     | `000B` | `2 → 1`; branch taken again                                   |
| `sub`                   | `000E` | `1 → 0`, flags `F002 → F046` (ZF, PF set)                     |
| `cmp`                   | `0011` | `0 − 0`; ZF set                                               |
| `jne` (falls through)   | `0013` | branch was taken twice, then ZF ends the loop                 |
| `mov cx, [bx]`          | `0015` | `CX = 0000` (was `FFFF`); `AX` still `0003`, `BX` still `0020` |
| `hlt`                   | `0016` | halted                                                        |

Encodings (verified by `countdown_program_uses_planned_encodings`): store
`89 07`, decrement `83 2F 01` (`83 /5`), compare `83 3F 00` (`83 /7`),
back-edge `75 F8` (backward disp8), load `8B 0F`, hlt `F4` — 22 bytes
total. The checkpoint test asserts ip, memory word and FLAGS at every
step above, that the branch is taken exactly twice before falling
through, and the final state `AX=0003`, `BX=0020`, `CX=0000`, memory
word `0000`; replay determinism re-runs the same fixture from reset.

## The `memory` demonstration

A core-fixture RAM roundtrip. Data lives at the dedicated location
`DS:0020` (`DS = 0` at reset), inside the fixed physical window
`0x0010–0x002F` (32 bytes) that the browser memory inspector displays.

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
