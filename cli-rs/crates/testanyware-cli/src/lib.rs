//! Library entry for `testanyware-cli`. Exposes the canonical command
//! surface (§1) and the §8 discoverability handlers so both `main.rs` and
//! the `cli-contract.rs` integration test consume them from one place.
//!
//! The crate keeps its `[[bin]]` target unchanged — adding `lib.rs` does
//! not displace the binary; cargo builds both.

pub mod surface;
pub mod discoverability;
