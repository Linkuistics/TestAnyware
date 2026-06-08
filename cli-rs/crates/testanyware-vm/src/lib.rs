//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`.

/// Shared RFB frame-capture + PNG encode (ADR-0008). Platform-neutral: the
/// CLI's `screen find-text` and the macOS recovery driver share it. **Not**
/// macOS-gated.
pub mod capture;
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
/// Windows golden-image creation: unattended ISO install + provisioning over
/// the in-VM agent's HTTP surface (no sshd on Windows). macOS-host only (FAT32
/// media built with `hdiutil`) — grove leaf `220/020`.
#[cfg(target_os = "macos")]
pub mod golden_windows;
/// In-process macOS Recovery automation: SIP toggle over RFB + OCR (ADR-0008,
/// grove leaf `110/030/010`). macOS-host only, like `tart`/`golden`.
#[cfg(target_os = "macos")]
pub mod recovery;
/// Top-level macOS golden creation: the SIP/TCC cycle + finalize + clone, wiring
/// `golden` (boot-1) and `recovery` together (grove leaf `110/030/020`). macOS-
/// host only, like `tart`/`golden`/`recovery`.
#[cfg(target_os = "macos")]
pub mod finalize;
/// Linux golden-image creation: a tart-based normal-mode SSH provisioning pass +
/// one apply-settings reboot, reusing `110`'s `ssh`/`tart`/`golden` layers
/// (ADR-0007). No SIP/TCC/recovery cycle (Linux has none), so it is far simpler
/// than `finalize`. macOS-host only (built on this Mac via tart) — grove leaf
/// `230-vm-create-golden-linux`.
#[cfg(target_os = "macos")]
pub mod golden_linux;

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
