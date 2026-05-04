//! Library entry for `testanyware-cli`. Exposes the canonical command
//! surface (§1), the §8 discoverability handlers, and the per-command
//! handler functions so both `main.rs` and the integration test suite
//! consume them from one place.
//!
//! The crate keeps its `[[bin]]` target unchanged — adding `lib.rs` does
//! not displace the binary; cargo builds both.

pub mod commands;
pub mod discoverability;
pub mod output;
pub mod resolve;
pub mod surface;
