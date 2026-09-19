# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Core: register file (word + 8-bit byte views, segment registers), 8086
  flags with static bits, 1 MiB memory with segment-to-physical addressing
  and wraparound, repeatable reset state, flat binary loading.
- Core: fetch/decode/execute for `MOV reg16, imm16` and `MOV reg8, imm8`;
  unknown opcodes reported via `StepError`.
- Core: `ADD AX, imm16` and `SUB AX, imm16` with full 8086 flag updates
  (CF, PF, AF, ZF, SF, OF) in a dedicated ALU module; `HLT` halted state.
- Web: debugger UI (Load/Step/Reset, registers, flags, change highlight,
  halted banner) driving the core through a Web Worker + WASM adapter.
- Web: Playwright smoke test running the demo program end-to-end.
- Tests: NASM program-level suite (`tests/programs/`) with committed flat
  binaries, expected final state, and CI source/binary drift detection.

- Cargo workspace with `emu86-core`, `emu86-cli`, `emu86-wasm` crates.
- Pinned stable toolchain (`rust-toolchain.toml`), rustfmt config,
  shared workspace lints (clippy), and `.editorconfig`.
- GitHub Actions CI: formatting, clippy, tests, wasm build — wired
  through the justfile.
- ADR-0001: workspace layout and toolchain pinning.
