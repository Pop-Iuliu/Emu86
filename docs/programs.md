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
| `memory`      | store → modify in RAM → load | `0100`     | `F013`      | `MOV [bx],ax`, `ADD word [bx],0x1000`, `SUB word [bx],byte -1` (sign-extended to 0xFFFF → CF+AF), `MOV cx,[bx]` loads `0x1101` back |

`wrap` byte layout: 16 bytes fill `0xFFFF0..0xFFFFF` exactly; the
immediate of `MOV AX, 0x1234` and the `HLT` wrap to `0x0000..0x0001`.

## Replay determinism

`replay_produces_identical_results` runs every program, resets, reloads and
runs again, and requires both final snapshots to be equal — reset state is a
single source of truth (see `Cpu::reset`).
