# emu86

Educational Intel 8086 CPU emulator with a visual debugger, written in Rust
and runnable in the browser. This is an **8086 CPU emulator first** — not a
DOS environment, not a cycle-accurate PC.

## Layout

| Path              | Purpose                                       |
| ----------------- | --------------------------------------------- |
| `crates/core/`    | The CPU core. Zero UI coupling.               |
| `crates/cli/`     | Headless CLI for running/tracing programs.    |
| `crates/wasm/`    | wasm-bindgen adapter for the browser.         |
| `web/`            | React + TypeScript + Vite frontend.           |
| `tests/programs/` | NASM program-level tests.                     |
| `docs/adr/`       | Architecture Decision Records.                |

## Development

All tasks go through the justfile:

```sh
just ci        # everything CI runs
just fmt       # format
just lint      # clippy
just test      # unit tests
just build     # debug build
```

## Loading a program

The debugger runs raw flat binaries — NASM output, no headers, no relocations:

```sh
nasm -O0 -f bin -o prog.bin prog.asm
```

- **Load demo** embeds the shared `countdown` demo
  (`tests/programs/countdown.bin`) and is the quickest way to start.
- **Load binary…** opens any `.bin` and runs it in the debugger.

Loading convention: the image is copied verbatim at `FFFF:0000` (physical
`0xFFFF0`) into a freshly reset CPU (`CS=FFFF`, `IP=0000`, `FLAGS=F002`), so
execution starts at the first byte of the file. Files must be non-empty and
at most 1 MiB (`0x100000` bytes — the 8086 address space); anything else is
rejected and the currently loaded program is kept. Images longer than 16
bytes wrap past `0xFFFFF` the way 8086 addressing does (no A20 gate).

**Reset** restores the loaded image and the reset state, ready to step
again — no need to reselect the file.

## Toolchains

Pinned by `rust-toolchain.toml` (stable Rust) — rustup will fetch it
automatically. CI pins the same version. Lockfiles (`Cargo.lock`,
`pnpm-lock.yaml`) are committed.

## Conventions

- Conventional Commits (`chore:`, `feat:`, `ci:`, `docs:`, `test:`, `fix:`)
- Keep a Changelog format in `CHANGELOG.md`
- Every decision gets an ADR in `docs/adr/`
- Every behavior backed by evidence (tests); instruction coverage is
  tracked, not just code coverage
