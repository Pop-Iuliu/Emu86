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
- Core: ModRM operand decoder (`modrm.rs`) resolving register-direct and all
  eight effective-address forms (mod 0/1/2, direct address, disp8 sign
  extension, DS default / SS for BP-based addressing, 16-bit wrap).
- Core: `ADD/SUB r/m16, imm16` (`81 /0`, `81 /5`), `ADD/SUB r/m16, imm8`
  (`83 /0`, `83 /5`), and `MOV` between r/m and reg for byte and word
  (`88`–`8B`); MOV leaves flags untouched; unsupported ModRM forms reported
  via `StepError::UnsupportedForm` with instruction address and bytes.
- Core/wasm/web: single `memory` demonstration program — stored as NASM
  source + committed binary, embedded by `demo_program()` for the browser
  demo, checkpointed in integration tests (registers, `DS:0020` bytes,
  flag transitions `F002 → F096 → F093`) and asserted by the Playwright
  smoke test; the hardcoded web-side byte copy was removed.
- Web: memory inspector showing a fixed 32-byte window around the demo
  data location (`0x0010`–`0x002F`) with per-byte change highlights;
  snapshot snapshots carry only the displayed bytes through the
  core → wasm → worker path.
- Web: "Last step" panel listing changed registers, the flags word with
  changed bit names, and visible memory bytes (previous → current);
  states when nothing displayed changed; comparisons cleared on load
  and reset.
- Web: execution panel labels CS:IP as the next instruction address with
  the physical address and a byte window; after HLT it shows the last
  executed address and explains IP advanced past HLT; flag hover
  explanations (CF vs OF); focus-visible outlines, tabular hex
  alignment, distinct busy/halted/error styling.
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
