//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`.

pub mod detached;
pub mod error;
pub mod id;
pub mod meta;
pub mod monitor;
pub mod paths;
pub mod preflight;
pub mod process;
pub mod qemu;
pub mod qemu_profile;
pub mod spec;

pub use detached::spawn_detached;
pub use error::VmError;
pub use id::generate_id;
pub use meta::{VmMeta, VmTool};
pub use monitor::QemuMonitorClient;
pub use paths::VmPaths;
pub use spec::{AgentEndpoint, VmSpec, VncEndpoint};
