//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`.

pub mod error;
pub mod id;
pub mod paths;

pub use error::VmError;
pub use id::generate_id;
pub use paths::VmPaths;
