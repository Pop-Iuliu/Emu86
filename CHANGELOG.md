# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Tests: `countdown` — the shared terminating-loop demonstration. One
  fixture (NASM source + committed binary) drives the core integration
  tests and, via `demo_program()`, the browser demo: `AX` supplies 3,
  memory at `DS:0020` counts 3→2→1→0 through `83 /5`, the `83 /7` compare
  raises ZF, the backward `75` branch exits, and `CX` (sentinel `FFFF`)
  receives the final 0. Checkpoint test records ip, memory word and FLAGS
  at every step, pins the `83 /5`, `83 /7` and backward-branch encodings,
  asserts the branch is taken exactly twice, and replay determinism
  re-runs it from reset.
- Core: `CMP` — `CMP AX, imm16` (`3D`), `CMP r/m16, imm16` (`81 /7`) and
  `CMP r/m16, imm8` (`83 /7`, sign-extended immediate) reuse the operand
  decoder and update the arithmetic flags without writing the operand.
- Core: relative jumps — `JMP short` (`EB`, signed disp8), `JMP near`
  (`E9`, disp16), `JE/JZ short` (`74`) and `JNE/JNZ short` (`75`); targets
  are computed from IP after the instruction bytes are fetched and wrap at
  16 bits; jumps leave flags unchanged.
- Tests: `loop` program — a counted loop whose back-edge is `JNE` on
  `CMP CX, 0`, proving flags decide whether execution repeats or continues
  forward (core suite + Playwright e2e stepping it through the browser UI).
- Web: **Load binary…** — open a raw flat `.bin` (NASM `-f bin` output) and
  run it in the debugger; size-validated (non-empty, ≤ 1 MiB) before it
  replaces the current program, with the filename, byte count and load
  address (`FFFF:0000`) shown in a new Program panel; the loading
  convention is documented in README ("Loading a program").
- Web: **Reset** replays the loaded program — reloads the image at
  `FFFF:0000` on top of the reset state so stepping can restart without
  reselecting the file; the demo load is now **Load demo**.
- Web: loading clears last-step comparisons and execution history; load
  errors are shown even before any program is loaded.
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

### Fixed

- Web: no more layout wobble while stepping — the busy indicator moved
  inline into the controls row (it no longer mounts a `<p>` that shifts
  the page), register values reserve their highlight padding up front,
  and memory bytes keep a fixed `2ch` width in both weights.
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
