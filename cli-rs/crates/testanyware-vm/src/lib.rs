//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`.

pub mod detached;
pub mod error;
pub mod health;
pub mod id;
pub mod lifecycle;
pub mod meta;
pub mod monitor;
pub mod paths;
pub mod preflight;
pub mod process;
pub mod qemu;
pub mod qemu_profile;
pub mod spec;
/// tart wraps Apple's Virtualization.framework — macOS-host only (ADR-0003
/// per-target gating). Absent from Linux/Windows builds.
#[cfg(target_os = "macos")]
pub mod tart;

pub use detached::spawn_detached;
pub use error::VmError;
pub use health::wait_for_agent;
pub use id::generate_id;
pub use lifecycle::{Platform, RunningEntry, VmLifecycle, VmListing, VmStartOptions, VmStartResult};
pub use meta::{VmMeta, VmTool};
pub use monitor::QemuMonitorClient;
pub use paths::VmPaths;
pub use qemu::{GoldenImage, QemuRunner};
pub use spec::{AgentEndpoint, VmSpec, VncEndpoint};
