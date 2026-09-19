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
  unknown opcodes reported as `UnknownOpcode`.

- Cargo workspace with `emu86-core`, `emu86-cli`, `emu86-wasm` crates.
- Pinned stable toolchain (`rust-toolchain.toml`), rustfmt config,
  shared workspace lints (clippy), and `.editorconfig`.
- GitHub Actions CI: formatting, clippy, tests, wasm build — wired
  through the justfile.
- ADR-0001: workspace layout and toolchain pinning.
