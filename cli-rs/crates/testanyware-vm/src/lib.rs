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
/// `russh`-backed SSH/SFTP provisioning helper for `vm create-golden`
/// (ADR-0007). Pure Rust + async, **not** macOS-gated: the Tier-2
/// linux/win goldens reuse it. Consumed by the `110` boot leaves.
pub mod ssh;
/// tart wraps Apple's Virtualization.framework — macOS-host only (ADR-0003
/// per-target gating). Absent from Linux/Windows builds.
#[cfg(target_os = "macos")]
pub mod tart;
/// Golden-image creation (boot-1 normal-mode provisioning over the `ssh`
/// seam). macOS-host only, like `tart` — grove node `110-vm-create-golden`.
#[cfg(target_os = "macos")]
pub mod golden;

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
pub use ssh::{ExecOutput, SshSession};
