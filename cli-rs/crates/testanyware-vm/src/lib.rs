//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`.

pub mod error;
pub mod id;
pub mod meta;
pub mod paths;
pub mod spec;

pub use error::VmError;
pub use id::generate_id;
pub use meta::{VmMeta, VmTool};
pub use paths::VmPaths;
pub use spec::{AgentEndpoint, VmSpec, VncEndpoint};
