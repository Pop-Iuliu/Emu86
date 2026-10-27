# Program tests

Each `.asm` is restricted to `cpu 8086` (NASM rejects 386+ mnemonics) and to
the opcodes emu86 currently decodes:

- `MOV reg, imm` (`B0+r`/`B8+r`), `MOV` between r/m and reg for byte and
  word (`88`–`8B`).
- The full group-1 ALU set — ADD, OR, ADC, SBB, AND, SUB, XOR, CMP — in
  three shapes: register/memory ↔ register both directions (`00`–`3B`
  bytes x0–x3: opcode bit 1 = direction, bit 0 = width), the
  accumulator-immediate family (`04`–`3D`: fixed AL/AX, raw `imm8` per
  byte op, full `imm16` per word op) and `r/m, imm` (`80`/`81`/`83 /0`–`/7`
  with `83` sign-extending its immediate).
- `INC/DEC r16` (`40`–`4F`), `INC/DEC r/m` via `FE /0 /1` and `FF /0 /1`
  (CF untouched, all other arithmetic flags set).
- `JMP short/near` (`EB`, `E9`), all sixteen `Jcc short` (`70`–`7F` —
  including the signed pair `JL/JGE/JLE/JG` that tests SF against OF),
  `LOOP/LOOPE/LOOPNE` (`E2`/`E1`/`E0`), `HLT` (`F4`).
- Stack primitives `PUSH/POP r16` (`50`–`5F`), segment pushes/pops
  (`06`/`07`, `0E`/`0F`, `16`/`17`, `1E`/`1F` — `0F` is the 8086-only
  `POP CS`), `PUSHF/POPF` (`9C`/`9D`), and near calls `CALL rel16` (`E8`),
  `RET imm16` (`C2`), `RET` (`C3`).

Two NASM quirks follow from that list. NASM is invoked with `-O0` so
`add ax, imm` assembles to the canonical `05 iw` accumulator form (then
`81 /0 iw`), which is what a hand-written `add ax, imm` actually produces;
the accumulator family covers every group-1 field, so any
`<op> ax/al, imm` source is safe. For `r/m, imm` forms NASM still picks
`81`/`80`/`83` automatically — `add bx, -1` assembles to `83 C3 FF`
(sign-extended), while `add bx, 0x100` is `81 C3 00 01`.

## Group 1 semantics

The eight `/N` fields of `80`/`81`/`83` share operand decoding, immediate
width and instruction length — only the ALU operation differs:

| `/N` | Mnemonic | Reads prior CF | CF after op  | OF after op     | AF after op  | SF/ZF/PF |
| ---- | -------- | -------------- | ------------ | --------------- | ------------ | -------- |
| 0    | `add`    | no             | carry out    | signed overflow | bit-3 carry  | result   |
| 1    | `or`     | no             | 0            | 0               | 0            | result   |
| 2    | `adc`    | yes            | carry out    | signed overflow | bit-3 carry  | result   |
| 3    | `sbb`    | yes            | borrow       | signed overflow | bit-3 borrow | result   |
| 4    | `and`    | no             | 0            | 0               | 0            | result   |
| 5    | `sub`    | no             | borrow       | signed overflow | bit-3 borrow | result   |
| 6    | `xor`    | no             | 0            | 0               | 0            | result   |
| 7    | `cmp`    | no             | as `sub`     | as `sub`        | as `sub`     | result   |

Only `adc` and `sbb` read CF from the previous instruction, so a
`cmp …` followed by `adc ax, 0` adds one extra via the carry. The
logical operators (`or`/`and`/`xor`) always clear CF/AF/OF — a stray
carry cannot leak past them. `cmp` computes `sub`'s flags but leaves
its destination untouched; every other `/N` writes the result.

| Opcode | Width | Immediate interpretation                          |
| ------ | ----- | ------------------------------------------------- |
| `80`   | byte  | raw `imm8`, never sign-extended (`0xFF` = +255)   |
| `81`   | word  | `imm16` verbatim                                  |
| `83`   | word  | sign-extended `imm8` (`0xFF` = `0xFFFF`, `0x7F` = `0x007F`) |

NASM uses `83` when the immediate fits in −128..127 — `add bx, -1`
assembles to `83 C3 FF`, but `add bx, 0x100` assembles to `81 C3 00 01`.

The operand-carrier base bytes (`00`–`3B`, shapes x0–x3) carry the same
eight operations with two extra bit decisions straight in the opcode:
bit 0 = width, bit 1 = direction (1 = the register field is the
destination). One dispatch (`opcode >> 3 & 7`) routes every form —
immediate, accumulator and register/memory — to the same ALU helpers, so
the flag word for `add al, 01` is byte-for-byte the one `80 /0 01`
produces. The accumulator slots (`04`–`3D`, shapes x4/x5) keep AL/AX as
the fixed destination: byte immediates are raw bytes, word immediates are
full `imm16`.

## INC/DEC — the pro-carrier ops

`INC r/m` and `DEC r/m` (`40`–`4F`, `FE /0 /1`, `FF /0 /1`) run
`add`/`sub` by one for every flag *except* CF: OF, SF, ZF, AF and PF
follow the by-one arithmetic bit-for-bit, while CF is never written —
the carry the previous instruction produced survives the increment.
That property is pinned as a table test (`inc == add(x, 1)` /
`dec == sub(x, 1)` for every flag except CF across wrap and overflow
boundaries, byte and word).

## Signed jumps and the loop family

The sixteen short jumps (`70`–`7F`) test one condition each; the
unsigned rows read CF/ZF (CF, ZF, CF|ZF and their inverses), parity and
sign rows read their single flag, and the signed rows `JL`/`JGE`/`JLE`
/`JG` compare SF against OF explicitly (`7D` takes when SF == OF, `7E`
when ZF is set *or* SF ≠ OF, `7F` when ¬ZF *and* SF == OF). They leave
flags untouched and consume exactly two bytes.

`LOOP` (`E2`) decrements CX and re-enters while it is non-zero — with
no flag write of any kind, so flags the body wrote survive the back
edge (`LOOPE`/`LOOPNE` (`E1`/`E0`) layer a ZF requirement on top).
That makes `loop` the flags-apart sibling of the `jne`-driven `loop`
fixture: same count, same adds, but the flag word is the last `add`'s,
not a compare's.

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
| `loopreg`     | LOOP re-enters on CX itself  | `0006`     | `F006`      | the `loop` fixture's sibling: 3× `add ax, 2` with a real `E2` back-edge; LOOP decrements CX without touching flags, so the flag word is the last add's (6 → PF) — exactly what the `jne` sibling cannot show |
| `countdown`   | browser demo — terminating countdown | `0003` | `F046`  | `AX = 3` stored at `DS:0020`; `83 /5` decrements the word 3→2→1→0; `83 /7` compare raises ZF at 0; backward `75` branch exits; `CX` (pre-set `FFFF`) loads the final 0 |
| `call`        | nested calls with stack arguments | `0001` | `F002`    | two args pushed, `call outer` → `call inner`; `inner` reads them through a BP frame (`push bp` / `mov bp, sp`, `[bp+6]`/`[bp+8]`); `outer` cleans up with `ret 4`; `AX` sentinel survives, SP unwinds to its start |

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

## The `call` demonstration

A core fixture for the stack: two levels of near calls with stack-passed
arguments and a `RET imm16` cleanup. `SS = 0` at reset, so the stack grows
down from physical `0x0000` (offsets `FFFE`, `FFFC`, …) while the program
sits at `FFFF:0000` — the two never meet.

Story: **main pushes two arguments → `call outer` → `call inner` → inner
reads the arguments through a BP frame → both returns unwind → `ret 4`
drops the arguments → `AX` sentinel survives → halt.**

| After                    | ip     | State                                                                                     |
| ------------------------ | ------ | ----------------------------------------------------------------------------------------- |
| `mov ax, 0x0001`         | `0003` | sentinel, never touched afterwards                                                        |
| `mov bx, 0x00AA`         | `0006` | first argument                                                                            |
| `mov cx, 0x00BB`         | `0009` | second argument                                                                           |
| `push bx`                | `000A` | `SP = FFFE`, word `00AA` at `0000:FFFE`                                                    |
| `push cx`                | `000B` | `SP = FFFC`, word `00BB` on top                                                            |
| `call outer`             | `000F` | `SP = FFFA`, return address `000E` pushed, `IP = 000F`                                     |
| `call inner`             | `0015` | `SP = FFF8`, return address `0012` pushed — two return addresses above the args            |
| `push bp` / `mov bp, sp` | `0018` | `BP = FFF6`; args now at `[bp+6]` and `[bp+8]`                                             |
| `mov dx, [bp+6]`         | `001B` | `DX = 00BB` — the last-pushed argument sits at the lower address                           |
| `mov cx, [bp+8]`         | `001E` | `CX = 00AA`                                                                                |
| `pop bp` / `ret`         | `0012` | frame torn down, `SP = FFFA`; inner returns into `outer`                                   |
| `ret 4`                  | `000E` | pops `outer`'s return address *and* adds 4: `SP = 0000`, both arguments dropped             |
| `hlt`                    | `000F` | halted, `AX = 0001`, flags `F002` — stack traffic never touches flags                      |

Encodings (verified by `call_program_uses_planned_encodings`): calls
`E8 01 00` / `E8 03 00`, cleanup `C2 04 00`, frame `55 89 E5`, arg loads
`8B 56 06` / `8B 4E 08` (`MOV r16, [bp+d8]`), `pop bp`/`ret` `5D C3` —
32 bytes total. The checkpoint test asserts SP, the pushed return
addresses and the BP-frame reads at every step; replay determinism
re-runs the same fixture from reset.

## The `loopreg` demonstration

The `loop` fixture's sibling that swings on the real `LOOP` opcode
instead of a compare-and-branch. Both count the same way — `AX` walks
`0 → 2 → 4 → 6` over three trips — but the mechanism differs: `loop`
drives the counted loop with the register instruction, the compare
never happens, and the flag word is whatever the last `add` left.

| After                   | ip     | State                                                              |
| ----------------------- | ------ | ------------------------------------------------------------------ |
| `mov cx, 0x0003`        | `0003` | the trip counter                                                    |
| `mov ax, 0x0000`        | `0006` | accumulator for the sum                                            |
| `add ax, 0x0002`        | `0009` | `0 → 2`; flags `F002` (low byte `02` has odd parity, so PF clear)   |
| `loop top` (taken)      | `0006` | CX `3 → 2`, flags unchanged — LOOP never writes them                |
| `add`, `loop`           | `0006` | `2 → 4`; back-edge taken again with CX `2 → 1`                      |
| `add`                   | `0009` | `4 → 6`, flags `F002 → F006` (PF set: `06` has two bits)            |
| `loop` (CX drains)      | `000B` | CX `1 → 0`, falls through, `F006` untouched                         |
| `hlt`                   | `000C` | halted, `AX = 0006`, `CX = 0000`                                    |

That final `F006` is the whole point: the flag word belongs to the last
`add`, because `loop` cannot smear anything else onto it. The `jne`
fixture's `F046`, by contrast, is the `cmp cx, 0` verdict — the pair
makes the store-vs-compute split of flag ownership visible.

Encodings (verified by `loopreg_program_uses_planned_encodings`): counts
`B9 03 00`, `B8 00 00`, sum `05 02 00` (accumulator immediate), back-edge
`E2 FB` (disp8 `FB` = −5 from `000B`), `F4` — 12 bytes total. The
checkpoint test asserts ip, AX, CX and the flag word at every step,
recording that `loop` re-enters `0006` leaving the `add`'s flags intact,
and replay determinism re-runs the same fixture from reset.

## The PUSH SP quirk

On the 8086, `PUSH SP` (`54`) stores the **decremented** stack pointer,
not the original value: with `SP = 0x1234`, the word `0x1232` lands at
`SS:0x1230`. Processors from the 286 on store the original `0x1234` —
the unit test `push_sp_pushes_the_new_sp_8086_quirk` pins the 8086
behaviour, and the SingleStepTests hardware corpus (sprint 05) doubles
as a second opinion. Its mirror image, `POP SP` (`5C`), discards the
usual `+2`: the popped value becomes SP outright.

## Replay determinism

`replay_produces_identical_results` runs every program, resets, reloads and
runs again, and requires both final snapshots to be equal — reset state is a
single source of truth (see `Cpu::reset`).
