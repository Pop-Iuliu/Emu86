# ADR-0001: Workspace layout and toolchain pinning

- Status: Accepted
- Date: 2026-09-19
- Deciders: emu86 team

## Context

Sprint 01 "First Pulse" starts from an empty repository. Decisions made now
(compiler version, lint policy, crate boundaries, task entrypoint) shape every
later sprint and are expensive to change once code accumulates. The project
brief fixes the stack (Rust stable, wasm-bindgen, React/Vite, clap CLI,
GitHub Actions) and the architecture (UI never owns CPU state).

## Decision

1. **Single Cargo workspace** with three crates:
   - `crates/core` — the CPU. Depends on `std` only. No wasm, no UI, no
     external crates. It must compile and test identically on host and wasm.
   - `crates/cli` — depends on core (+ `clap`). Talks to the same core the
     browser uses.
   - `crates/wasm` — depends on core (+ `wasm-bindgen`). Thin adapter; owns
     no authoritative state.

2. **Pin the toolchain exactly.** `rust-toolchain.toml` declares an exact
   stable version (not a floating channel). Both lockfiles (`Cargo.lock`,
   `pnpm-lock.yaml`) are committed. GitHub Actions are pinned by commit SHA,
   not version tag.

3. **Lint policy lives once**, in the workspace root
   (`[workspace.lints]`), inherited by every crate via `[lints]
   workspace = true`. `unsafe_code = deny`, clippy `panic = deny`,
   warnings treated as errors in CI.

4. **Overflow policy via profiles.** Debug builds keep
   `overflow-checks = true` (the default). Guest-visible arithmetic uses
   explicit `wrapping_*` operations, which behave identically in every
   profile — so an accidental raw `+`/`-`/`*` on guest values panics loudly
   in debug tests and CI, while release stays correct. This enforces the
   "deliberate wrapping arithmetic" rule by construction.

5. **The justfile is the only task entrypoint.** CI invokes `just ci`,
   never raw `cargo`/`pnpm` commands, so contributors run byte-identical
   checks locally and in CI.

6. **Every CI job must be able to fail.** No checks are added for components
   that do not exist yet (e.g. the frontend ESLint job waits until `web/`
   has content); green checks over empty scaffolding are rejected.

## Consequences

- Toolchain updates are one-line diffs in `rust-toolchain.toml` (and
  lockfiles), reviewed explicitly.
- A `panic = deny` lint means guest-visible code cannot rely on panicking
  paths; errors must be surfaced as values.
- The frontend task surface stays out of the justfile until `web/` exists,
  then gains its own recipes (`web-lint`, `web-test`, ...).
- Bumping `clap`/`wasm-bindgen` happens through the lockfile commit, which
  CI validates.

## Alternatives considered

- **Floating `stable` channel**: rejected — CI breaks weeks later through
  no code change.
- **Per-crate lint config**: rejected — drift and duplication across three
  crates.
- **CI calling raw commands**: rejected — local/CI drift is the classic
  source of "works locally, red in Actions".
- **Disabling overflow checks in dev** (to "match release"): rejected —
  explicit `wrapping_*` already guarantees release semantics; keeping
  overflow checks on turns accidental arithmetic into a loud test failure
  instead of silent divergence.
