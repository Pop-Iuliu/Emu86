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
