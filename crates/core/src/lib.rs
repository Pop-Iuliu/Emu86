//! Inspectable Intel 8086 CPU emulator core.
//!
//! This crate is fully independent of any UI. It owns registers, flags,
//! decode/execute, segmented addressing, interrupts and the port-I/O
//! interface. Consumers (CLI, WASM adapter) drive it and render snapshots.

/// Placeholder so the CI test path is meaningful from the first commit.
/// Replaced by real decoder tests in the first execution slice.
#[cfg(test)]
mod tests {
    #[test]
    fn ci_path_is_alive() {
        assert_eq!(2 + 2, 4);
    }
}
