//! WASM adapter exposing snapshots of emu86-core to the browser.
//!
//! Owns no authoritative state — every call forwards to the core and
//! serializes the result for the Web Worker boundary.

use wasm_bindgen::prelude::*;

/// Minimal binding so the wasm target compiles from the first commit.
/// Snapshot API arrives with the first execution slice.
#[wasm_bindgen]
pub fn core_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
