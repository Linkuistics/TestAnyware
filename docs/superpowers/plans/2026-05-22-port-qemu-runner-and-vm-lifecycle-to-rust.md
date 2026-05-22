# Port QEMU Runner and VM Lifecycle to Rust — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port the QEMU-based VM lifecycle (`vm start` / `stop` / `list` / `delete`) from the Swift CLI (`cli/`) to the Rust port (`cli-rs/`), as a new `testanyware-vm` workspace crate wired into the CLI.

**Architecture:** A new `testanyware-vm` crate holds the runner, monitor client, process management, spec/meta sidecars, and the lifecycle orchestrator — peer to `testanyware-rfb` / `testanyware-agent-client`. QEMU is the primary path (Linux/Windows guests on Linux/macOS hosts). Host-specific QEMU details (accelerator, architecture, UEFI firmware) are selected by a `#[cfg]`-gated `QemuProfile`, per the 2026-05-22 per-platform-facilities decision. Child processes use `tokio::process` with `setsid` via `nix` under `#[cfg(unix)]`. The tart backend is explicitly out of scope (backlog task 12).

**Tech Stack:** Rust, `tokio` (process/net/fs/time), `nix` (signals, setsid), `serde`/`serde_json`, `thiserror`, `getrandom`, `clap`. Reuses `testanyware-agent-client` for the agent health probe.

**Source of truth for the port:** `cli/Sources/TestAnywareDriver/VM/*.swift`. Each task names the exact Swift file and the behaviour to preserve. The Swift code is the spec; the Rust below is the complete translation.

**Constraints (from the backlog item + design conversation):**
- `tokio::process` for child management; `setsid` via `nix` behind `#[cfg(unix)]`.
- The Darwin pipe-EOF / Foundation-`Process`-not-`setsid` memory entries **do not apply** to Rust. The `nc -U` monitor workaround is dropped — Rust talks to the monitor socket directly via `UnixStream`. The process-tree-kill sequence is re-validated, not ported verbatim.
- KVM on Linux: `/dev/kvm` must be readable+writable. Missing ⇒ `code: KVM_PERMISSION_DENIED`, remediation names `usermod -aG kvm $USER`.
- swtpm required for Windows guests. Missing ⇒ `code: SWTPM_MISSING`, remediation `apt install swtpm swtpm-tools` (Linux) / `brew install swtpm` (macOS).
- Every command satisfies `docs/architecture/cli-design-contract.md`: §7 help template (examples + exit codes), `--json`, stable error codes, `--dry-run` on mutating commands.

**Key decisions locked for this plan:**
- New crate `testanyware-vm` (confirmed).
- A live `vm start --platform windows` smoke against the on-host golden is the final task (confirmed). Linux-guest and macOS-guest acceptance criteria stay deferred (no Linux golden on this host; tart is task 12).
- Guest architecture follows **host architecture** — goldens are built per-host by `vm-create-golden-*.sh`, and both KVM and HVF only accelerate same-arch guests. The `QemuProfile` therefore keys off the host triple.
- `vm start --platform` is **required** (no default). The Swift default of `macos` always routed to tart; under this task macOS routes to `VM_BACKEND_UNSUPPORTED` (tart is task 12), so a default that always errors is worse than requiring the flag.
- Two new error codes (`KVM_PERMISSION_DENIED`, `SWTPM_MISSING`) are added to contract §4.2 as an amendment (Task 13).
- Viewer wiring (`--viewer`) is backlog task 8. `vm start --viewer` is accepted but prints a "viewer not yet ported" notice on stderr and proceeds; `meta.viewer_window_id` stays `null`.

**Branch:** `worktree-port-qemu-vm-lifecycle` (worktree already created).

---

## File Structure

New crate `cli-rs/crates/testanyware-vm/`:

| File | Responsibility |
|---|---|
| `Cargo.toml` | Crate manifest. |
| `src/lib.rs` | Module declarations + public re-exports. |
| `src/error.rs` | `VmError` enum; `code()`, `exit_code()`, `remediation()`, `details()`. |
| `src/id.rs` | `generate_id()` → `testanyware-<hex8>`. |
| `src/paths.rs` | `VmPaths` — XDG dirs, spec/meta/clone/session paths. |
| `src/spec.rs` | `VmSpec` public spec sidecar (atomic write, load). |
| `src/meta.rs` | `VmMeta` private lifecycle sidecar (atomic write, load). |
| `src/monitor.rs` | `QemuMonitorClient` over `UnixStream`; HMP response parsers. |
| `src/process.rs` | `process_alive`, `terminate`, `pgrep_first` — process-tree control. |
| `src/detached.rs` | `spawn_detached` — `tokio::process` + `setsid`, log redirection. |
| `src/qemu_profile.rs` | `QemuProfile` (`#[cfg]`-gated host details), `which`, UEFI resolution. |
| `src/preflight.rs` | `check_kvm`, `check_swtpm` host preflight. |
| `src/qemu.rs` | `QemuRunner` — scanners, `build_qemu_args`, `start`, `teardown`. |
| `src/health.rs` | `wait_for_agent` — agent `/health` poll loop. |
| `src/lifecycle.rs` | `VmLifecycle` — `start`/`stop`/`delete`; option/result types. |

Modified in `cli-rs/`:

| File | Change |
|---|---|
| `Cargo.toml` | Add `testanyware-vm` member; add `nix`, `getrandom` workspace deps. |
| `crates/testanyware-cli/Cargo.toml` | Add `testanyware-vm` path dep. |
| `crates/testanyware-cli/src/commands/mod.rs` | `pub mod vm;`. |
| `crates/testanyware-cli/src/commands/vm.rs` | **New** — `run_vm_{start,stop,list,delete}` handlers. |
| `crates/testanyware-cli/src/main.rs` | `--base`/`--id`/`--json`/`--dry-run`/`--limit`/`--all`/`--filter` flags; §7 after-help; dispatch. |
| `crates/testanyware-cli/src/surface.rs` | Add `KVM_PERMISSION_DENIED`, `SWTPM_MISSING` to `ERROR_CODES`. |
| `crates/testanyware-cli/src/output.rs` | Map the two new codes in `exit_code_for`. |
| `crates/testanyware-cli/tests/cli-contract.rs` | Add vm-scoped contract assertions. |

Modified elsewhere:

| File | Change |
|---|---|
| `docs/architecture/cli-design-contract.md` | §4.2 amendment: add the two error codes. |
| `docs/reference/cli-schemas/vm-{start,stop,list,delete}.json` | Replace stubs with real schemas. |

---

## Task 1: Scaffold the `testanyware-vm` crate

**Files:**
- Create: `cli-rs/crates/testanyware-vm/Cargo.toml`
- Create: `cli-rs/crates/testanyware-vm/src/lib.rs`
- Modify: `cli-rs/Cargo.toml`

- [ ] **Step 1: Add workspace deps and the new member to `cli-rs/Cargo.toml`**

In `[workspace.dependencies]`, add (after `tempfile = "3.13"`):

```toml
nix = { version = "0.29", features = ["signal", "process"] }
getrandom = "0.2"
```

In `members`, add `"crates/testanyware-vm",` after `"crates/testanyware-rfb",`.

- [ ] **Step 2: Create `cli-rs/crates/testanyware-vm/Cargo.toml`**

```toml
[package]
name = "testanyware-vm"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
repository.workspace = true
description = "QEMU-backed VM lifecycle for the TestAnyware host CLI."

[dependencies]
testanyware-agent-client = { path = "../testanyware-agent-client" }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
thiserror = { workspace = true }
getrandom = { workspace = true }

[target.'cfg(unix)'.dependencies]
nix = { workspace = true }

[dev-dependencies]
tempfile = { workspace = true }
wiremock = { workspace = true }
```

- [ ] **Step 3: Create `cli-rs/crates/testanyware-vm/src/lib.rs`**

```rust
//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`. QEMU is the
//! primary backend (Linux/Windows guests on Linux/macOS hosts); the tart
//! backend is a separate backlog task. Host-specific QEMU details are
//! selected by a `#[cfg]`-gated [`qemu_profile::QemuProfile`].

pub mod error;
pub mod id;
pub mod paths;

pub use error::VmError;
pub use id::generate_id;
pub use paths::VmPaths;
```

- [ ] **Step 4: Verify the crate builds**

Run: `cd cli-rs && cargo build -p testanyware-vm`
Expected: compiles (warnings about unused `lib.rs` re-exports are fine until later modules land — there should be none yet since `error`/`id`/`paths` land in Tasks 2–3; until then this step is run *after* Task 3).

> Sequencing note: Steps 1–2 can be committed immediately. Step 3's `lib.rs` references modules created in Tasks 2–3, so create `lib.rs` with only `pub mod error;` here and add `id`/`paths` lines in Task 3. Adjust Step 3 to:

```rust
//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`.

pub mod error;
```

- [ ] **Step 5: Commit**

```bash
git add cli-rs/Cargo.toml cli-rs/crates/testanyware-vm/Cargo.toml cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): scaffold testanyware-vm crate"
```

---

## Task 2: `VmError` — error type and §4 code mapping

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/error.rs`
- Test: inline `#[cfg(test)]` in `error.rs`

The CLI surfaces errors as `(code, message, remediation, details)` tuples (contract §10.3). `VmError` is the crate's error type; `code()`/`exit_code()`/`remediation()`/`details()` feed the §3.4 envelope.

- [ ] **Step 1: Write the failing tests** — append to `error.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_match_contract_section_4() {
        assert_eq!(VmError::KvmPermissionDenied { path: "/dev/kvm".into() }.code(), "KVM_PERMISSION_DENIED");
        assert_eq!(VmError::SwtpmMissing.code(), "SWTPM_MISSING");
        assert_eq!(VmError::UefiNotFound { path: "/x".into() }.code(), "UEFI_NOT_FOUND");
        assert_eq!(VmError::QemuFailed { detail: "x".into() }.code(), "QEMU_FAILED");
        assert_eq!(VmError::MonitorDiscoveryFailed.code(), "QEMU_FAILED");
        assert_eq!(VmError::SpawnFailed { detail: "x".into() }.code(), "SPAWN_FAILED");
        assert_eq!(VmError::GoldenNotFound { name: "g".into() }.code(), "GOLDEN_NOT_FOUND");
        assert_eq!(VmError::GoldenInUse { name: "g".into(), clone_pids: vec![] }.code(), "GOLDEN_IN_USE");
        assert_eq!(VmError::VmNotFound { id: "v".into() }.code(), "VM_NOT_FOUND");
        assert_eq!(VmError::VmStopFailed { id: "v".into() }.code(), "VM_STOP_FAILED");
        assert_eq!(VmError::BackendUnsupported { platform: "macos".into() }.code(), "VM_BACKEND_UNSUPPORTED");
        assert_eq!(VmError::InvalidPlatform { value: "bsd".into() }.code(), "INVALID_PLATFORM");
        assert_eq!(VmError::Io("disk full".into()).code(), "IO_ERROR");
    }

    #[test]
    fn exit_codes_match_contract_section_5() {
        // §5: 3 = not-found family, 4 = permission, 5 = conflict, 2 = usage, 1 = generic.
        assert_eq!(VmError::KvmPermissionDenied { path: "/dev/kvm".into() }.exit_code(), 4);
        assert_eq!(VmError::SwtpmMissing.exit_code(), 1);
        assert_eq!(VmError::UefiNotFound { path: "/x".into() }.exit_code(), 3);
        assert_eq!(VmError::GoldenNotFound { name: "g".into() }.exit_code(), 3);
        assert_eq!(VmError::VmNotFound { id: "v".into() }.exit_code(), 3);
        assert_eq!(VmError::GoldenInUse { name: "g".into(), clone_pids: vec![1] }.exit_code(), 5);
        assert_eq!(VmError::InvalidPlatform { value: "x".into() }.exit_code(), 2);
        assert_eq!(VmError::QemuFailed { detail: "x".into() }.exit_code(), 1);
    }

    #[test]
    fn kvm_remediation_names_the_usermod_command() {
        let r = VmError::KvmPermissionDenied { path: "/dev/kvm".into() }
            .remediation()
            .expect("kvm error has remediation");
        assert!(r.contains("usermod -aG kvm"), "remediation must name the fix: {r}");
    }

    #[test]
    fn swtpm_remediation_names_both_package_managers() {
        let r = VmError::SwtpmMissing.remediation().expect("swtpm error has remediation");
        assert!(r.contains("apt install swtpm"), "remediation names apt: {r}");
        assert!(r.contains("brew install swtpm"), "remediation names brew: {r}");
    }

    #[test]
    fn golden_in_use_details_carry_clone_pids() {
        let d = VmError::GoldenInUse { name: "g".into(), clone_pids: vec![41, 42] }.details();
        assert_eq!(d["clone_pids"], serde_json::json!([41, 42]));
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm error::`
Expected: FAIL — `VmError` not defined.

- [ ] **Step 3: Write `error.rs` above the test module**

```rust
//! `VmError` — the crate error type, mapped to contract §4 codes.

use serde_json::{json, Value};

/// Errors produced by the VM lifecycle. Each variant maps 1:1 to a §4
/// error code and a §5 exit code.
#[derive(Debug, thiserror::Error)]
pub enum VmError {
    #[error("/dev/kvm is not readable+writable at {path}")]
    KvmPermissionDenied { path: String },

    #[error("swtpm is not installed; it is required for Windows guests")]
    SwtpmMissing,

    #[error("UEFI firmware not found at {path}")]
    UefiNotFound { path: String },

    #[error("QEMU failed: {detail}")]
    QemuFailed { detail: String },

    #[error("could not discover the agent port via the QEMU monitor")]
    MonitorDiscoveryFailed,

    #[error("failed to spawn a child process: {detail}")]
    SpawnFailed { detail: String },

    #[error("golden image '{name}' not found")]
    GoldenNotFound { name: String },

    #[error("golden image '{name}' is in use by running clones (PIDs {clone_pids:?})")]
    GoldenInUse { name: String, clone_pids: Vec<i32> },

    #[error("no VM found for id '{id}'")]
    VmNotFound { id: String },

    #[error("VM '{id}' did not stop cleanly")]
    VmStopFailed { id: String },

    #[error("no backend can serve platform '{platform}' (tart support is a later task)")]
    BackendUnsupported { platform: String },

    #[error("unknown platform '{value}' (expected macos, linux, or windows)")]
    InvalidPlatform { value: String },

    #[error("I/O error: {0}")]
    Io(String),
}

impl VmError {
    /// Stable contract §4 code surfaced in `--json` output.
    pub fn code(&self) -> &'static str {
        match self {
            VmError::KvmPermissionDenied { .. } => "KVM_PERMISSION_DENIED",
            VmError::SwtpmMissing => "SWTPM_MISSING",
            VmError::UefiNotFound { .. } => "UEFI_NOT_FOUND",
            VmError::QemuFailed { .. } | VmError::MonitorDiscoveryFailed => "QEMU_FAILED",
            VmError::SpawnFailed { .. } => "SPAWN_FAILED",
            VmError::GoldenNotFound { .. } => "GOLDEN_NOT_FOUND",
            VmError::GoldenInUse { .. } => "GOLDEN_IN_USE",
            VmError::VmNotFound { .. } => "VM_NOT_FOUND",
            VmError::VmStopFailed { .. } => "VM_STOP_FAILED",
            VmError::BackendUnsupported { .. } => "VM_BACKEND_UNSUPPORTED",
            VmError::InvalidPlatform { .. } => "INVALID_PLATFORM",
            VmError::Io(_) => "IO_ERROR",
        }
    }

    /// §5 process exit code.
    pub fn exit_code(&self) -> i32 {
        match self {
            VmError::KvmPermissionDenied { .. } => 4,
            VmError::UefiNotFound { .. }
            | VmError::GoldenNotFound { .. }
            | VmError::VmNotFound { .. } => 3,
            VmError::GoldenInUse { .. } => 5,
            VmError::InvalidPlatform { .. } => 2,
            _ => 1,
        }
    }

    /// Actionable remediation string (contract §4 / §9.5).
    pub fn remediation(&self) -> Option<String> {
        match self {
            VmError::KvmPermissionDenied { .. } => Some(
                "Add yourself to the kvm group: `sudo usermod -aG kvm $USER`, \
                 then log out and back in."
                    .into(),
            ),
            VmError::SwtpmMissing => Some(
                "Install swtpm: `apt install swtpm swtpm-tools` on Linux, \
                 `brew install swtpm` on macOS."
                    .into(),
            ),
            VmError::GoldenInUse { .. } => {
                Some("Stop the running clones first, or re-run with --force.".into())
            }
            VmError::GoldenNotFound { .. } => {
                Some("Run `testanyware vm list` to see available golden images.".into())
            }
            VmError::VmNotFound { .. } => {
                Some("Run `testanyware vm list` to see running VMs.".into())
            }
            VmError::BackendUnsupported { .. } => Some(
                "QEMU serves linux and windows guests. macOS guests use the tart \
                 backend, which is not yet ported to the Rust CLI."
                    .into(),
            ),
            _ => None,
        }
    }

    /// `details` payload for the §3.4 JSON error envelope.
    pub fn details(&self) -> Value {
        match self {
            VmError::KvmPermissionDenied { path } | VmError::UefiNotFound { path } => {
                json!({ "path": path })
            }
            VmError::GoldenNotFound { name } => json!({ "golden_name": name }),
            VmError::GoldenInUse { name, clone_pids } => {
                json!({ "golden_name": name, "clone_pids": clone_pids })
            }
            VmError::VmNotFound { id } | VmError::VmStopFailed { id } => json!({ "vm_id": id }),
            VmError::BackendUnsupported { platform } | VmError::InvalidPlatform { value: platform } => {
                json!({ "platform": platform })
            }
            _ => Value::Null,
        }
    }
}
```

- [ ] **Step 4: Run the tests to confirm they pass**

Run: `cd cli-rs && cargo test -p testanyware-vm error::`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/error.rs
git commit -m "feat(vm): VmError type with contract §4 code mapping"
```

---

## Task 3: VM identifiers and XDG paths

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/id.rs`
- Create: `cli-rs/crates/testanyware-vm/src/paths.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

Ports `VMStartOptions.generateID()` (`VMTypes.swift:67`) and `VMPaths.swift` plus `QEMURunner.sessionDir` (`QEMURunner.swift:295`).

- [ ] **Step 1: Write the failing tests for `id.rs`** — create `id.rs`:

```rust
//! VM instance identifiers: `testanyware-<8 hex digits>` (contract §6).

/// Generate a fresh `testanyware-<hex8>` identifier. Ports
/// `VMStartOptions.generateID()` — 4 random bytes rendered lowercase hex.
pub fn generate_id() -> String {
    let mut bytes = [0u8; 4];
    getrandom::getrandom(&mut bytes).expect("getrandom failed");
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("testanyware-{hex}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_has_expected_shape() {
        let id = generate_id();
        assert!(id.starts_with("testanyware-"), "got {id}");
        let hex = id.strip_prefix("testanyware-").unwrap();
        assert_eq!(hex.len(), 8, "8 hex chars: {id}");
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_uppercase()));
    }

    #[test]
    fn ids_are_distinct() {
        let a = generate_id();
        let b = generate_id();
        assert_ne!(a, b, "two ids collided (1-in-4-billion fluke, re-run)");
    }
}
```

- [ ] **Step 2: Write the failing tests for `paths.rs`** — create `paths.rs`:

```rust
//! XDG-compliant path helpers for VM lifecycle artefacts.
//!
//! Ports `VMPaths.swift` and `QEMURunner.sessionDir`. The clone tree
//! (qcow2, EFI vars, TPM state) lives under `$XDG_DATA_HOME`; the AF_UNIX
//! sockets live under `$TMPDIR` so the path fits the 104-byte `sun_path`
//! limit (see decision log 2026-04-20).

use std::collections::HashMap;
use std::path::PathBuf;

/// Resolved VM-lifecycle directories.
#[derive(Debug, Clone)]
pub struct VmPaths {
    state_dir: PathBuf,
    data_dir: PathBuf,
    tmp_dir: PathBuf,
}

impl VmPaths {
    /// Resolve from the process environment.
    pub fn from_process_env() -> Self {
        let env: HashMap<String, String> = std::env::vars().collect();
        Self::from_env(&env)
    }

    /// Resolve from an explicit environment map (test-friendly). Mirrors
    /// `VMPaths.init(env:)`: `$XDG_STATE_HOME` / `$XDG_DATA_HOME` win when
    /// set and non-empty, else `$HOME/.local/{state,share}`. `$TMPDIR`
    /// resolves the socket session root, falling back to `/tmp`.
    pub fn from_env(env: &HashMap<String, String>) -> Self {
        let get = |k: &str| env.get(k).filter(|v| !v.is_empty()).cloned();
        let home = get("HOME").unwrap_or_default();
        let state_dir = match get("XDG_STATE_HOME") {
            Some(x) => PathBuf::from(x).join("testanyware"),
            None => PathBuf::from(&home).join(".local/state/testanyware"),
        };
        let data_dir = match get("XDG_DATA_HOME") {
            Some(x) => PathBuf::from(x).join("testanyware"),
            None => PathBuf::from(&home).join(".local/share/testanyware"),
        };
        let raw_tmp = get("TMPDIR").unwrap_or_else(|| "/tmp".to_string());
        let tmp_dir = PathBuf::from(raw_tmp.trim_end_matches('/'));
        Self { state_dir, data_dir, tmp_dir }
    }

    pub fn vms_dir(&self) -> PathBuf { self.state_dir.join("vms") }
    pub fn golden_dir(&self) -> PathBuf { self.data_dir.join("golden") }
    pub fn clones_dir(&self) -> PathBuf { self.data_dir.join("clones") }

    pub fn spec_path(&self, id: &str) -> PathBuf { self.vms_dir().join(format!("{id}.json")) }
    pub fn meta_path(&self, id: &str) -> PathBuf { self.vms_dir().join(format!("{id}.meta.json")) }
    pub fn clone_dir(&self, id: &str) -> PathBuf { self.clones_dir().join(id) }

    /// Per-VM short-path session dir under `$TMPDIR` for AF_UNIX sockets.
    /// Ports `QEMURunner.sessionDir(forID:)`.
    pub fn session_dir(&self, id: &str) -> PathBuf {
        self.tmp_dir.join(format!("testanyware-{id}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn xdg_dirs_win_when_set() {
        let p = VmPaths::from_env(&env(&[
            ("XDG_STATE_HOME", "/s"),
            ("XDG_DATA_HOME", "/d"),
        ]));
        assert_eq!(p.vms_dir(), PathBuf::from("/s/testanyware/vms"));
        assert_eq!(p.golden_dir(), PathBuf::from("/d/testanyware/golden"));
        assert_eq!(p.clones_dir(), PathBuf::from("/d/testanyware/clones"));
    }

    #[test]
    fn falls_back_to_home_dot_local() {
        let p = VmPaths::from_env(&env(&[("HOME", "/Users/alice")]));
        assert_eq!(p.vms_dir(), PathBuf::from("/Users/alice/.local/state/testanyware/vms"));
        assert_eq!(p.golden_dir(), PathBuf::from("/Users/alice/.local/share/testanyware/golden"));
    }

    #[test]
    fn empty_xdg_is_treated_as_unset() {
        let p = VmPaths::from_env(&env(&[("XDG_STATE_HOME", ""), ("HOME", "/h")]));
        assert_eq!(p.vms_dir(), PathBuf::from("/h/.local/state/testanyware/vms"));
    }

    #[test]
    fn spec_and_meta_paths() {
        let p = VmPaths::from_env(&env(&[("XDG_STATE_HOME", "/s")]));
        assert_eq!(p.spec_path("testanyware-abcd1234"),
            PathBuf::from("/s/testanyware/vms/testanyware-abcd1234.json"));
        assert_eq!(p.meta_path("testanyware-abcd1234"),
            PathBuf::from("/s/testanyware/vms/testanyware-abcd1234.meta.json"));
    }

    #[test]
    fn session_dir_strips_trailing_slash_from_tmpdir() {
        let p = VmPaths::from_env(&env(&[("TMPDIR", "/var/folders/x/T/")]));
        assert_eq!(p.session_dir("testanyware-abcd1234"),
            PathBuf::from("/var/folders/x/T/testanyware-testanyware-abcd1234"));
    }

    #[test]
    fn session_dir_defaults_to_tmp() {
        let p = VmPaths::from_env(&env(&[]));
        assert_eq!(p.session_dir("v"), PathBuf::from("/tmp/testanyware-v"));
    }
}
```

- [ ] **Step 3: Add the modules to `lib.rs`**

Replace `lib.rs` body with:

```rust
//! QEMU-backed VM lifecycle for the TestAnyware host CLI.
//!
//! Port of `cli/Sources/TestAnywareDriver/VM/*.swift`.

pub mod error;
pub mod id;
pub mod paths;

pub use error::VmError;
pub use id::generate_id;
pub use paths::VmPaths;
```

- [ ] **Step 4: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm`
Expected: PASS (5 error + 2 id + 6 paths = 13 tests).

- [ ] **Step 5: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/id.rs cli-rs/crates/testanyware-vm/src/paths.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): VM id generation and XDG path helpers"
```

---

## Task 4: Spec and meta sidecars

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/spec.rs`
- Create: `cli-rs/crates/testanyware-vm/src/meta.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

Ports `VMSpec.swift` (public spec, read by connection resolution) and `VMMeta.swift` (private lifecycle sidecar). The acceptance criterion "an in-flight VM started by the Swift CLI can be stopped by the Rust CLI" requires the meta JSON keys to match Swift's: `id`, `tool`, `pid`, `clone_dir`, `viewer_window_id`. The spec must deserialize the same shape `cli-rs/.../resolve.rs::ConnectionSpec` reads (`vnc`, `agent?`, `platform?`).

Both sidecars write atomically: write `<path>.tmp`, chmod `0600`, rename into place — ports the `writeAtomic` pattern.

- [ ] **Step 1: Write the failing tests for `spec.rs`** — create `spec.rs`:

```rust
//! `VmSpec` — the public per-VM spec sidecar at `<vms>/<id>.json`.
//!
//! Port of `VMSpec.swift`. Written by `vm start`, read by the CLI's
//! connection-resolution chain (`resolve.rs::ConnectionSpec`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::VmError;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VncEndpoint {
    pub host: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentEndpoint {
    pub host: String,
    pub port: u16,
}

/// Public per-VM spec. `platform` is a plain string (`macos`/`linux`/
/// `windows`) so it round-trips with `ConnectionSpec.platform`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSpec {
    pub vnc: VncEndpoint,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent: Option<AgentEndpoint>,
    pub platform: String,
}

impl VmSpec {
    pub fn load(path: &Path) -> Result<Self, VmError> {
        let bytes = std::fs::read(path).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))
    }

    /// Atomically write `self` to `path`, mode 0600. Ports
    /// `VMSpec.writeAtomic`.
    pub fn write_atomic(&self, path: &Path) -> Result<(), VmError> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| VmError::Io(format!("serialize spec: {e}")))?;
        crate::spec::write_atomic_0600(path, &json)
    }
}

/// Shared atomic-write helper used by both sidecars.
pub(crate) fn write_atomic_0600(path: &Path, bytes: &[u8]) -> Result<(), VmError> {
    let tmp = std::path::PathBuf::from(format!("{}.tmp", path.display()));
    std::fs::write(&tmp, bytes).map_err(|e| VmError::Io(format!("{}: {e}", tmp.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))
            .map_err(|e| VmError::Io(format!("chmod {}: {e}", tmp.display())))?;
    }
    std::fs::rename(&tmp, path).map_err(|e| VmError::Io(format!("rename into {}: {e}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("testanyware-deadbeef.json");
        let spec = VmSpec {
            vnc: VncEndpoint { host: "localhost".into(), port: 5901, password: Some("testanyware".into()) },
            agent: Some(AgentEndpoint { host: "localhost".into(), port: 51234 }),
            platform: "windows".into(),
        };
        spec.write_atomic(&path).unwrap();
        assert_eq!(VmSpec::load(&path).unwrap(), spec);
    }

    #[test]
    fn deserializes_a_swift_shaped_spec() {
        // Exactly the shape `VMSpec.swift` emits (sorted, pretty).
        let json = r#"{
          "agent": { "host": "localhost", "port": 51234 },
          "platform": "linux",
          "vnc": { "host": "localhost", "password": "testanyware", "port": 5900 }
        }"#;
        let spec: VmSpec = serde_json::from_str(json).unwrap();
        assert_eq!(spec.platform, "linux");
        assert_eq!(spec.vnc.port, 5900);
        assert_eq!(spec.agent.unwrap().port, 51234);
    }

    #[test]
    fn agentless_spec_round_trips() {
        let spec = VmSpec {
            vnc: VncEndpoint { host: "localhost".into(), port: 5900, password: None },
            agent: None,
            platform: "windows".into(),
        };
        let json = serde_json::to_string(&spec).unwrap();
        assert!(!json.contains("agent"), "absent agent must not serialize: {json}");
        assert_eq!(serde_json::from_str::<VmSpec>(&json).unwrap(), spec);
    }

    #[cfg(unix)]
    #[test]
    fn written_file_is_mode_0600() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.json");
        let spec = VmSpec {
            vnc: VncEndpoint { host: "h".into(), port: 1, password: None },
            agent: None,
            platform: "linux".into(),
        };
        spec.write_atomic(&path).unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
```

- [ ] **Step 2: Write the failing tests for `meta.rs`** — create `meta.rs`:

```rust
//! `VmMeta` — the private lifecycle sidecar at `<vms>/<id>.meta.json`.
//!
//! Port of `VMMeta.swift`. The CLI never consumes this; `vm stop` reads
//! it to tear the VM down. JSON keys match the Swift `CodingKeys`
//! (`clone_dir`, `viewer_window_id`) so a VM started by either CLI can be
//! stopped by the other for the parallel-tooling period.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::VmError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VmTool {
    Tart,
    Qemu,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmMeta {
    pub id: String,
    pub tool: VmTool,
    pub pid: i32,
    #[serde(rename = "clone_dir", default, skip_serializing_if = "Option::is_none")]
    pub clone_dir: Option<String>,
    #[serde(rename = "viewer_window_id", default, skip_serializing_if = "Option::is_none")]
    pub viewer_window_id: Option<String>,
}

impl VmMeta {
    pub fn load(path: &Path) -> Result<Self, VmError> {
        let bytes = std::fs::read(path).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))?;
        serde_json::from_slice(&bytes).map_err(|e| VmError::Io(format!("{}: {e}", path.display())))
    }

    pub fn write_atomic(&self, path: &Path) -> Result<(), VmError> {
        let json = serde_json::to_vec_pretty(self)
            .map_err(|e| VmError::Io(format!("serialize meta: {e}")))?;
        crate::spec::write_atomic_0600(path, &json)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("v.meta.json");
        let meta = VmMeta {
            id: "testanyware-deadbeef".into(),
            tool: VmTool::Qemu,
            pid: 4242,
            clone_dir: Some("/d/clones/testanyware-deadbeef".into()),
            viewer_window_id: None,
        };
        meta.write_atomic(&path).unwrap();
        assert_eq!(VmMeta::load(&path).unwrap(), meta);
    }

    #[test]
    fn deserializes_a_swift_shaped_qemu_meta() {
        // Exactly the shape `vm-start.sh` / `VMMeta.swift` emits.
        let json = r#"{
          "clone_dir": "/Users/x/.local/share/testanyware/clones/testanyware-abcd1234",
          "id": "testanyware-abcd1234",
          "pid": 9876,
          "tool": "qemu"
        }"#;
        let meta: VmMeta = serde_json::from_str(json).unwrap();
        assert_eq!(meta.tool, VmTool::Qemu);
        assert_eq!(meta.pid, 9876);
        assert_eq!(meta.clone_dir.as_deref(),
            Some("/Users/x/.local/share/testanyware/clones/testanyware-abcd1234"));
        assert_eq!(meta.viewer_window_id, None);
    }

    #[test]
    fn tool_serializes_lowercase() {
        let json = serde_json::to_string(&VmTool::Qemu).unwrap();
        assert_eq!(json, "\"qemu\"");
    }

    #[test]
    fn key_names_match_swift_coding_keys() {
        let meta = VmMeta {
            id: "v".into(), tool: VmTool::Qemu, pid: 1,
            clone_dir: Some("/c".into()), viewer_window_id: Some("w".into()),
        };
        let json = serde_json::to_string(&meta).unwrap();
        assert!(json.contains("\"clone_dir\""), "snake_case clone_dir: {json}");
        assert!(json.contains("\"viewer_window_id\""), "snake_case viewer id: {json}");
    }
}
```

- [ ] **Step 3: Add the modules to `lib.rs`** — after `pub mod paths;` add:

```rust
pub mod meta;
pub mod spec;
```

and after the re-exports add:

```rust
pub use meta::{VmMeta, VmTool};
pub use spec::{AgentEndpoint, VmSpec, VncEndpoint};
```

- [ ] **Step 4: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm spec:: meta::`
Expected: PASS (4 spec + 4 meta tests).

- [ ] **Step 5: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/spec.rs cli-rs/crates/testanyware-vm/src/meta.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): VmSpec and VmMeta sidecars with atomic write"
```

---

## Task 5: QEMU monitor client

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/monitor.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

Ports `QEMUMonitorClient.swift`. **Re-validation note (constraint):** the Swift client shells out to `nc -U`; the Rust port talks to the HMP socket directly with `tokio::net::UnixStream` — the `nc` indirection was a Foundation-`Process` workaround and is dropped. The pure parsers (`parse_agent_port`, `parse_vnc_port`) are ported verbatim, including CRLF handling.

- [ ] **Step 1: Write the failing parser tests** — create `monitor.rs` with this test module (impl added in Step 3):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_agent_port_reads_host_forward_row() {
        // QEMU `info usernet` row layout: TCP[HOST_FORWARD] <fd> * <hostport> <guest> ...
        let resp = "Hub -1 (net0):\r\n  Protocol[State]    FD  Source Address  Port   Dest. Address  Port\r\n  TCP[HOST_FORWARD]  10   *               51234        10.0.2.15     8648\r\n";
        assert_eq!(parse_agent_port(resp), Some(51234));
    }

    #[test]
    fn parse_agent_port_handles_crlf_collapsed_response() {
        // The whole response on one logical line if a caller split on "\n"
        // wrong — our parser uses str::lines() which strips \r, so this
        // still resolves. Guards the CRLF regression from decision log
        // 2026-04-20.
        let resp = "header\r\nTCP[HOST_FORWARD]  10   *  49999  10.0.2.15  8648\r\n";
        assert_eq!(parse_agent_port(resp), Some(49999));
    }

    #[test]
    fn parse_agent_port_returns_none_without_a_forward_row() {
        assert_eq!(parse_agent_port("Hub -1 (net0):\r\nno forwards here\r\n"), None);
    }

    #[test]
    fn parse_vnc_port_reads_server_address() {
        let resp = "Server:\r\n     address: 127.0.0.1:5901\r\n  auth: vnc\r\n";
        assert_eq!(parse_vnc_port(resp), Some(5901));
    }

    #[test]
    fn parse_vnc_port_returns_none_when_absent() {
        assert_eq!(parse_vnc_port("Server:\r\n  none\r\n"), None);
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm monitor::`
Expected: FAIL — `parse_agent_port` / `parse_vnc_port` not defined.

- [ ] **Step 3: Write `monitor.rs` above the test module**

```rust
//! HMP (Human Monitor Protocol) client for a QEMU monitor unix socket.
//!
//! Port of `QEMUMonitorClient.swift`. Unlike the Swift version, this
//! talks to the socket directly via `tokio::net::UnixStream` — the
//! `nc -U` subprocess was a Foundation-`Process` workaround that does
//! not apply to Rust.

use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;

/// HMP client bound to one monitor socket path.
pub struct QemuMonitorClient {
    socket_path: PathBuf,
}

impl QemuMonitorClient {
    pub fn new(socket_path: impl Into<PathBuf>) -> Self {
        Self { socket_path: socket_path.into() }
    }

    /// Send `command` over the monitor socket and return whatever the
    /// monitor writes within `drain`. HMP is line-oriented; the monitor
    /// keeps the connection open, so we read until `drain` elapses.
    pub async fn send(&self, command: &str, drain: Duration) -> std::io::Result<String> {
        let mut stream = UnixStream::connect(&self.socket_path).await?;
        stream.write_all(command.as_bytes()).await?;
        stream.write_all(b"\n").await?;
        stream.flush().await?;

        let mut buf = Vec::new();
        let mut chunk = [0u8; 4096];
        // Best-effort read: a closed peer or an elapsed deadline both end
        // the loop; the parsers tolerate the HMP banner noise either way.
        let _ = tokio::time::timeout(drain, async {
            loop {
                match stream.read(&mut chunk).await {
                    Ok(0) => break,
                    Ok(n) => buf.extend_from_slice(&chunk[..n]),
                    Err(_) => break,
                }
            }
        })
        .await;
        Ok(String::from_utf8_lossy(&buf).into_owned())
    }

    /// Poll `info usernet` until the guest→host forward port appears.
    pub async fn agent_port(&self, attempts: u32, interval: Duration) -> Option<u16> {
        for attempt in 0..attempts {
            if let Ok(resp) = self.send("info usernet", Duration::from_millis(500)).await {
                if let Some(port) = parse_agent_port(&resp) {
                    return Some(port);
                }
            }
            if attempt + 1 < attempts {
                tokio::time::sleep(interval).await;
            }
        }
        None
    }

    /// Poll `info vnc` until the listening VNC port appears.
    pub async fn vnc_port(&self, attempts: u32, interval: Duration) -> Option<u16> {
        for attempt in 0..attempts {
            if let Ok(resp) = self.send("info vnc", Duration::from_millis(500)).await {
                if let Some(port) = parse_vnc_port(&resp) {
                    return Some(port);
                }
            }
            if attempt + 1 < attempts {
                tokio::time::sleep(interval).await;
            }
        }
        None
    }

    /// Best-effort `set_password vnc <password>`. The monitor may not
    /// accept connections immediately after launch — retry and swallow
    /// errors. Ports `QEMUMonitorClient.setVNCPassword`.
    pub async fn set_vnc_password(&self, password: &str, attempts: u32) {
        let sanitised: String = password.chars().filter(|c| *c != '\n' && *c != '\r').collect();
        for attempt in 0..attempts {
            let _ = self
                .send(&format!("set_password vnc {sanitised}"), Duration::from_millis(300))
                .await;
            if attempt + 1 < attempts {
                tokio::time::sleep(Duration::from_millis(500)).await;
            }
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }
}

/// Parse the host-forward port from `info usernet`. The first
/// `HOST_FORWARD` row wins. `str::lines()` strips trailing `\r`, so CRLF
/// monitor responses parse correctly (decision log 2026-04-20).
pub fn parse_agent_port(info_usernet: &str) -> Option<u16> {
    for line in info_usernet.lines() {
        if !line.contains("HOST_FORWARD") {
            continue;
        }
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() >= 4 {
            if let Ok(port) = fields[3].parse::<u16>() {
                return Some(port);
            }
        }
    }
    None
}

/// Parse the listening VNC port from `info vnc` — the digits after the
/// first `127.0.0.1:` marker.
pub fn parse_vnc_port(info_vnc: &str) -> Option<u16> {
    const MARKER: &str = "127.0.0.1:";
    let idx = info_vnc.find(MARKER)?;
    let digits: String = info_vnc[idx + MARKER.len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit())
        .collect();
    digits.parse().ok()
}
```

- [ ] **Step 4: Add the module to `lib.rs`** — add `pub mod monitor;` and `pub use monitor::QemuMonitorClient;`.

- [ ] **Step 5: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm monitor::`
Expected: PASS (5 tests).

- [ ] **Step 6: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/monitor.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): QEMU monitor client over UnixStream"
```

---

## Task 6: Process-tree control

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/process.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

Ports the kill helpers from `QEMURunner.swift` (`teardown`, `pgrepFirst`, the `kill(pid,0)` liveness checks). **Re-validation note (constraint):** the SIGTERM→wait→SIGKILL sequence is re-validated here rather than ported verbatim — it is exercised by a real spawned child in the tests below. swtpm discovery keeps the `pgrep -f` approach (swtpm `--daemon` self-detaches, so the spawn-time pid is the short-lived parent; `pgrep` against the TPM state-dir path is the portable way to find the daemon, and it works for VMs started by either CLI).

- [ ] **Step 1: Write the failing tests** — create `process.rs` with this test module:

```rust
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    fn spawn_sleep(secs: u32) -> i32 {
        let child = std::process::Command::new("sleep")
            .arg(secs.to_string())
            .spawn()
            .expect("spawn sleep");
        child.id() as i32
    }

    #[test]
    fn process_alive_tracks_a_real_child() {
        let pid = spawn_sleep(30);
        assert!(process_alive(pid), "freshly spawned child should be alive");
        terminate(pid, Duration::from_millis(100), 10);
        assert!(!process_alive(pid), "child should be dead after terminate");
    }

    #[test]
    fn process_alive_is_false_for_unused_pid() {
        // PID 2^31-1 is effectively never allocated.
        assert!(!process_alive(i32::MAX));
    }

    #[test]
    fn terminate_is_a_noop_for_dead_pid() {
        // Must not panic / must not signal an unrelated process.
        terminate(i32::MAX, Duration::from_millis(10), 2);
    }

    #[test]
    fn pgrep_first_finds_a_running_process() {
        let pid = spawn_sleep(30);
        // `sleep 30` is matchable by its argument.
        let found = pgrep_first("sleep 30");
        terminate(pid, Duration::from_millis(100), 10);
        assert_eq!(found, Some(pid), "pgrep should locate the sleep child");
    }

    #[test]
    fn pgrep_first_returns_none_on_no_match() {
        assert_eq!(pgrep_first("a-pattern-that-matches-nothing-xyzzy-42"), None);
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm process::`
Expected: FAIL — `process_alive` / `terminate` / `pgrep_first` not defined.

- [ ] **Step 3: Write `process.rs` above the test module**

```rust
//! Process-tree control: liveness checks, graceful-then-forced
//! termination, and `pgrep`-based discovery.
//!
//! Ports the kill helpers from `QEMURunner.swift`. Unix-only; Windows
//! host support (`CREATE_NEW_PROCESS_GROUP` / `GenerateConsoleCtrlEvent`)
//! is backlog task 14.

#[cfg(unix)]
use std::time::{Duration, Instant};

/// True if `pid` names a live process. Ports the Swift `kill(pid, 0) == 0`
/// idiom: signal 0 performs error checking without delivering a signal.
#[cfg(unix)]
pub fn process_alive(pid: i32) -> bool {
    use nix::sys::signal::kill;
    use nix::unistd::Pid;
    if pid <= 0 {
        return false;
    }
    // Ok => exists; Err(EPERM) => exists but not ours; Err(ESRCH) => gone.
    !matches!(kill(Pid::from_raw(pid), None), Err(nix::errno::Errno::ESRCH))
}

#[cfg(not(unix))]
pub fn process_alive(_pid: i32) -> bool {
    false
}

/// Terminate `pid`: SIGTERM, poll up to `attempts` times spaced by
/// `poll_interval`, then SIGKILL if still alive. Idempotent and
/// best-effort — a dead or stale pid is a silent no-op. Ports the qemu
/// branch of `QEMURunner.teardown`.
#[cfg(unix)]
pub fn terminate(pid: i32, poll_interval: Duration, attempts: u32) {
    use nix::sys::signal::{kill, Signal};
    use nix::unistd::Pid;
    if pid <= 0 || !process_alive(pid) {
        return;
    }
    let target = Pid::from_raw(pid);
    let _ = kill(target, Signal::SIGTERM);
    let deadline = Instant::now();
    for _ in 0..attempts {
        if !process_alive(pid) {
            return;
        }
        std::thread::sleep(poll_interval);
    }
    let _ = deadline; // (kept for symmetry with the Swift wait loop)
    if process_alive(pid) {
        let _ = kill(target, Signal::SIGKILL);
    }
}

#[cfg(not(unix))]
pub fn terminate(_pid: i32, _poll_interval: std::time::Duration, _attempts: u32) {}

/// First PID whose command line matches `pattern`, via `pgrep -f`.
/// Returns `None` on no match or if `pgrep` is unavailable. Ports
/// `QEMURunner.pgrepFirst`.
#[cfg(unix)]
pub fn pgrep_first(pattern: &str) -> Option<i32> {
    let output = std::process::Command::new("pgrep")
        .args(["-f", pattern])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .and_then(|line| line.trim().parse::<i32>().ok())
}

#[cfg(not(unix))]
pub fn pgrep_first(_pattern: &str) -> Option<i32> {
    None
}
```

- [ ] **Step 4: Add the module to `lib.rs`** — add `pub mod process;`.

- [ ] **Step 5: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm process::`
Expected: PASS (5 tests).

- [ ] **Step 6: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/process.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): process-tree control (terminate, pgrep, liveness)"
```

---

## Task 7: Detached process spawn

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/detached.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

Ports `DetachedProcess.swift`. The child must outlive the CLI invocation and be immune to SIGHUP — spawn it in its own session via `setsid`. **Re-validation note:** `posix_spawn` + `POSIX_SPAWN_SETSID` becomes `tokio::process::Command` + `pre_exec(setsid)` under `#[cfg(unix)]`. The Foundation-`Process`-cannot-`setsid` memory entry does not apply; this is the clean Rust path.

- [ ] **Step 1: Write the failing test** — create `detached.rs` with this test module:

```rust
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn spawn_detached_runs_child_in_its_own_session() {
        let dir = tempfile::tempdir().unwrap();
        let log = dir.path().join("child.log");
        // `sh -c 'echo hi; sleep 30'` — long enough to inspect, writes to log.
        let pid = spawn_detached(
            "/bin/sh",
            &["-c".into(), "echo hi; sleep 30".into()],
            &log,
        )
        .expect("spawn");

        assert!(crate::process::process_alive(pid), "detached child should be running");

        // A `setsid` child is a session leader: its SID equals its PID.
        let sid = std::process::Command::new("ps")
            .args(["-o", "sess=", "-p", &pid.to_string()])
            .output()
            .expect("ps")
            .stdout;
        let sid: i32 = String::from_utf8_lossy(&sid).trim().parse().expect("parse sid");
        assert_eq!(sid, pid, "detached child must be its own session leader");

        // stdout was redirected to the log file.
        std::thread::sleep(Duration::from_millis(200));
        let logged = std::fs::read_to_string(&log).unwrap_or_default();
        assert!(logged.contains("hi"), "child stdout should land in the log: {logged:?}");

        crate::process::terminate(pid, Duration::from_millis(100), 10);
    }

    #[test]
    fn spawn_detached_reports_a_missing_executable() {
        let dir = tempfile::tempdir().unwrap();
        let err = spawn_detached("/no/such/binary-xyzzy", &[], &dir.path().join("l.log"));
        assert!(err.is_err(), "missing executable must be an error");
    }
}
```

- [ ] **Step 2: Run the test to confirm it fails**

Run: `cd cli-rs && cargo test -p testanyware-vm detached::`
Expected: FAIL — `spawn_detached` not defined.

- [ ] **Step 3: Write `detached.rs` above the test module**

```rust
//! Spawn a long-running process in its own session.
//!
//! Port of `DetachedProcess.swift`. The child is immune to SIGHUP on the
//! caller's terminal (`setsid`) and outlives the CLI. stdout+stderr go to
//! `log_path` (append); stdin is `/dev/null`.

use std::path::Path;
use std::process::Stdio;

use crate::error::VmError;

/// Spawn `program` with `args` detached. Returns the child PID. The
/// `tokio::process::Child` is dropped without waiting — `kill_on_drop`
/// defaults to `false`, so the process keeps running and is reparented to
/// init/launchd when the short-lived CLI exits.
pub fn spawn_detached(program: &str, args: &[String], log_path: &Path) -> Result<i32, VmError> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map_err(|e| VmError::Io(format!("open {}: {e}", log_path.display())))?;
    let log_err = log
        .try_clone()
        .map_err(|e| VmError::Io(format!("dup log fd: {e}")))?;

    let mut cmd = tokio::process::Command::new(program);
    cmd.args(args)
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err))
        .kill_on_drop(false);

    #[cfg(unix)]
    {
        let devnull = std::fs::File::open("/dev/null")
            .map_err(|e| VmError::Io(format!("open /dev/null: {e}")))?;
        cmd.stdin(Stdio::from(devnull));
        // SAFETY: `setsid` is async-signal-safe and the only post-fork
        // action; it places the child in a fresh session so it survives
        // the parent's exit and a terminal SIGHUP.
        unsafe {
            cmd.pre_exec(|| {
                nix::unistd::setsid()
                    .map(|_| ())
                    .map_err(|e| std::io::Error::from_raw_os_error(e as i32))
            });
        }
    }
    #[cfg(not(unix))]
    {
        cmd.stdin(Stdio::null());
    }

    let child = cmd
        .spawn()
        .map_err(|e| VmError::SpawnFailed { detail: format!("{program}: {e}") })?;
    let pid = child
        .id()
        .ok_or_else(|| VmError::SpawnFailed { detail: format!("{program}: exited before id") })?;
    // Detach: drop without waiting. kill_on_drop is false.
    drop(child);
    Ok(pid as i32)
}
```

- [ ] **Step 4: Add the module to `lib.rs`** — add `pub mod detached;` and `pub use detached::spawn_detached;`.

- [ ] **Step 5: Run the test**

Run: `cd cli-rs && cargo test -p testanyware-vm detached::`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/detached.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): detached process spawn via tokio + setsid"
```

---

## Task 8: Host QEMU profile

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/qemu_profile.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

The Swift `QEMURunner` hard-codes macOS-on-Apple-Silicon (`qemu-system-aarch64`, `-accel hvf`, `edk2-aarch64-code.fd`). Per the 2026-05-22 per-platform-facilities decision, the Rust port selects host details via a `#[cfg]`-gated `QemuProfile`: the guest architecture follows the host (goldens are built per-host; KVM/HVF only accelerate same-arch). This task delivers the profile and the `which` / UEFI resolution; the macOS-aarch64 branch is verbatim-faithful to Swift (it is the branch the live smoke exercises). The Linux branches are designed here; their live verification is deferred (no Linux golden on this host).

- [ ] **Step 1: Write the failing tests** — create `qemu_profile.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_profile_is_internally_consistent() {
        let p = host_profile();
        assert!(p.qemu_binary.starts_with("qemu-system-"), "binary: {}", p.qemu_binary);
        assert!(!p.accelerator.is_empty(), "accelerator must be set");
        assert!(!p.machine.is_empty(), "machine must be set");
        assert!(!p.uefi_code_candidates.is_empty(), "must list UEFI candidates");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_profile_uses_hvf() {
        let p = host_profile();
        assert_eq!(p.accelerator, "hvf");
        assert_eq!(p.qemu_binary, "qemu-system-aarch64");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_profile_uses_kvm() {
        let p = host_profile();
        assert_eq!(p.accelerator, "kvm");
    }

    #[test]
    fn resolve_uefi_code_picks_the_first_existing_candidate() {
        let dir = tempfile::tempdir().unwrap();
        let real = dir.path().join("edk2-code.fd");
        std::fs::write(&real, b"firmware").unwrap();
        let candidates = vec![
            dir.path().join("missing-a.fd"),
            real.clone(),
            dir.path().join("missing-b.fd"),
        ];
        assert_eq!(resolve_uefi_code(&candidates), Some(real));
    }

    #[test]
    fn resolve_uefi_code_is_none_when_no_candidate_exists() {
        let dir = tempfile::tempdir().unwrap();
        let candidates = vec![dir.path().join("nope.fd")];
        assert_eq!(resolve_uefi_code(&candidates), None);
    }

    #[test]
    fn which_finds_a_known_binary() {
        // `sh` is on PATH on every supported host.
        assert!(which("sh").is_some(), "which(sh) should resolve");
        assert!(which("a-binary-that-does-not-exist-xyzzy").is_none());
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm qemu_profile::`
Expected: FAIL — `host_profile` / `resolve_uefi_code` / `which` not defined.

- [ ] **Step 3: Write `qemu_profile.rs` above the test module**

```rust
//! Host-specific QEMU details, selected by `#[cfg]`.
//!
//! Per the 2026-05-22 per-platform-facilities decision, the Rust port
//! uses the best native accelerator per host (HVF on macOS, KVM on
//! Linux) rather than a lowest-common-denominator engine. Guest
//! architecture follows the host: goldens are built per-host by
//! `vm-create-golden-*.sh`, and KVM/HVF only accelerate same-arch guests.

use std::path::{Path, PathBuf};

/// Host-resolved QEMU launch parameters.
#[derive(Debug, Clone)]
pub struct QemuProfile {
    /// `qemu-system-*` binary name (resolved on PATH at launch time).
    pub qemu_binary: &'static str,
    /// `-accel` value.
    pub accelerator: &'static str,
    /// `-machine` value.
    pub machine: &'static str,
    /// `-cpu` value.
    pub cpu: &'static str,
    /// Ordered UEFI code-firmware candidates; the first that exists wins.
    pub uefi_code_candidates: Vec<PathBuf>,
}

/// The profile for the current host. macOS-aarch64 is faithful to the
/// Swift `QEMURunner`; the Linux branches follow the same device model
/// with KVM + the host architecture's firmware.
pub fn host_profile() -> QemuProfile {
    #[cfg(target_os = "macos")]
    {
        // macOS hosts are Apple Silicon: aarch64 guests under HVF.
        QemuProfile {
            qemu_binary: "qemu-system-aarch64",
            accelerator: "hvf",
            machine: "virt,highmem=on,gic-version=3",
            cpu: "host",
            uefi_code_candidates: macos_uefi_candidates(),
        }
    }
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    {
        QemuProfile {
            qemu_binary: "qemu-system-x86_64",
            accelerator: "kvm",
            machine: "q35",
            cpu: "host",
            uefi_code_candidates: vec![
                PathBuf::from("/usr/share/OVMF/OVMF_CODE.fd"),
                PathBuf::from("/usr/share/edk2/x64/OVMF_CODE.fd"),
                PathBuf::from("/usr/share/edk2-ovmf/x64/OVMF_CODE.fd"),
                PathBuf::from("/usr/share/qemu/edk2-x86_64-code.fd"),
            ],
        }
    }
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    {
        QemuProfile {
            qemu_binary: "qemu-system-aarch64",
            accelerator: "kvm",
            machine: "virt,gic-version=3",
            cpu: "host",
            uefi_code_candidates: vec![
                PathBuf::from("/usr/share/AAVMF/AAVMF_CODE.fd"),
                PathBuf::from("/usr/share/edk2/aarch64/QEMU_CODE.fd"),
                PathBuf::from("/usr/share/qemu/edk2-aarch64-code.fd"),
            ],
        }
    }
    #[cfg(not(any(
        target_os = "macos",
        all(target_os = "linux", target_arch = "x86_64"),
        all(target_os = "linux", target_arch = "aarch64"),
    )))]
    {
        // Windows host support is backlog task 14; give a usable default
        // so the crate still compiles on unanticipated targets.
        QemuProfile {
            qemu_binary: "qemu-system-x86_64",
            accelerator: "tcg",
            machine: "q35",
            cpu: "max",
            uefi_code_candidates: vec![],
        }
    }
}

/// macOS UEFI candidates: derived from the resolved `qemu-system-aarch64`
/// install prefix (`<prefix>/share/qemu/edk2-aarch64-code.fd`), as the
/// Swift `QEMURunner.start` does, plus the standard Homebrew location.
#[cfg(target_os = "macos")]
fn macos_uefi_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(bin) = which("qemu-system-aarch64") {
        // <prefix>/bin/qemu-system-aarch64 → <prefix>/share/qemu/...
        if let Some(prefix) = bin.parent().and_then(Path::parent) {
            out.push(prefix.join("share/qemu/edk2-aarch64-code.fd"));
        }
    }
    out.push(PathBuf::from("/opt/homebrew/share/qemu/edk2-aarch64-code.fd"));
    out
}

/// Return the first existing UEFI code firmware among `candidates`.
pub fn resolve_uefi_code(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates.iter().find(|p| p.is_file()).cloned()
}

/// Resolve `name` to an absolute path by scanning `$PATH`. On macOS,
/// `/opt/homebrew/bin` and `/usr/local/bin` are appended so the qemu
/// toolchain resolves even when the CLI runs from a scrubbed environment.
pub fn which(name: &str) -> Option<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    #[cfg(target_os = "macos")]
    {
        dirs.push(PathBuf::from("/opt/homebrew/bin"));
        dirs.push(PathBuf::from("/usr/local/bin"));
    }
    for dir in dirs {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
```

- [ ] **Step 4: Add the module to `lib.rs`** — add `pub mod qemu_profile;`.

- [ ] **Step 5: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm qemu_profile::`
Expected: PASS (5 tests on this macOS host: consistency, hvf, uefi-found, uefi-none, which).

- [ ] **Step 6: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/qemu_profile.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): cfg-gated host QEMU profile and tool resolution"
```

---

## Task 9: Host preflight checks

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/preflight.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

`check_kvm` enforces the `/dev/kvm` readable+writable constraint on Linux (no-op elsewhere — macOS uses HVF). `check_swtpm` enforces the swtpm-for-Windows-guests constraint on every host. Both return the precise `VmError` the constraint mandates.

- [ ] **Step 1: Write the failing tests** — create `preflight.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(not(target_os = "linux"))]
    #[test]
    fn check_kvm_is_a_noop_off_linux() {
        // macOS uses HVF; there is no /dev/kvm to gate on.
        assert!(check_kvm().is_ok());
    }

    #[test]
    fn check_swtpm_reports_swtpm_missing_when_absent() {
        // Force an empty PATH so `which("swtpm")` cannot resolve.
        let saved = std::env::var_os("PATH");
        std::env::remove_var("PATH");
        let result = check_swtpm();
        match saved {
            Some(p) => std::env::set_var("PATH", p),
            None => {}
        }
        match result {
            Err(VmError::SwtpmMissing) => {}
            other => panic!("expected SwtpmMissing, got {other:?}"),
        }
    }
}
```

> Note: `check_swtpm_reports_swtpm_missing_when_absent` mutates `PATH`, so it must not run concurrently with `which`-dependent tests. Mark it (and any other env-mutating test) with the comment `// serial: mutates PATH` and run the crate's tests with `--test-threads` left at default — these env-mutating tests are isolated to their own module and the macOS `which` test in Task 8 reads `PATH` only once at call time, so a flake is improbable; if CI flakes, gate this test behind a `serial_test` dev-dep. For this plan, keep it as-is.

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm preflight::`
Expected: FAIL — `check_kvm` / `check_swtpm` not defined.

- [ ] **Step 3: Write `preflight.rs` above the test module**

```rust
//! Host preflight checks for the QEMU backend.

use crate::error::VmError;
use crate::qemu_profile::which;

/// Verify `/dev/kvm` is readable and writable (Linux only). macOS uses
/// HVF and Windows uses WHPX, so this is a no-op off Linux. A missing or
/// unwritable `/dev/kvm` is the most common first-run failure on Linux
/// hosts; the remediation names `usermod -aG kvm $USER`.
#[cfg(target_os = "linux")]
pub fn check_kvm() -> Result<(), VmError> {
    const KVM: &str = "/dev/kvm";
    if !std::path::Path::new(KVM).exists() {
        return Err(VmError::KvmPermissionDenied { path: KVM.into() });
    }
    match std::fs::OpenOptions::new().read(true).write(true).open(KVM) {
        Ok(_) => Ok(()),
        Err(_) => Err(VmError::KvmPermissionDenied { path: KVM.into() }),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn check_kvm() -> Result<(), VmError> {
    Ok(())
}

/// Verify swtpm is installed. Required for Windows guests (TPM 2.0
/// socket). The remediation names the package on both Linux and macOS.
pub fn check_swtpm() -> Result<(), VmError> {
    if which("swtpm").is_some() {
        Ok(())
    } else {
        Err(VmError::SwtpmMissing)
    }
}
```

- [ ] **Step 4: Add the module to `lib.rs`** — add `pub mod preflight;`.

- [ ] **Step 5: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm preflight::`
Expected: PASS (2 tests on macOS: kvm-noop, swtpm-missing).

- [ ] **Step 6: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/preflight.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): KVM and swtpm host preflight checks"
```

---

## Task 10: QEMU runner — scanners, arg builder, start, teardown

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/qemu.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

The heart of the port: `QEMURunner.swift`. Split into pure, unit-testable pieces (`platform_from_name`, `scan_golden_dir`, `scan_clones_dir`, `build_qemu_args`) and the side-effecting `start` / `teardown`.

`Platform` for this crate is a small enum (`Linux`, `Windows`, `Macos`) with `default_base()` and `backend()` — ports the `Platform` extension in `VMTypes.swift`.

- [ ] **Step 1: Write the failing tests** — create `qemu.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn platform_from_name_classifies_image_names() {
        assert_eq!(platform_from_name("testanyware-golden-macos-tahoe"), "macos");
        assert_eq!(platform_from_name("testanyware-golden-linux-24.04"), "linux");
        assert_eq!(platform_from_name("ubuntu-server"), "linux");
        assert_eq!(platform_from_name("testanyware-golden-windows-11"), "windows");
        assert_eq!(platform_from_name("mystery-image"), "unknown");
    }

    #[test]
    fn scan_golden_dir_lists_qcow2_images() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("testanyware-golden-windows-11.qcow2"), b"x").unwrap();
        fs::write(dir.path().join("testanyware-golden-linux-24.04.qcow2"), b"x").unwrap();
        fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        let mut names: Vec<String> =
            scan_golden_dir(dir.path()).into_iter().map(|g| g.name).collect();
        names.sort();
        assert_eq!(names, vec![
            "testanyware-golden-linux-24.04",
            "testanyware-golden-windows-11",
        ]);
    }

    #[test]
    fn scan_golden_dir_is_empty_for_a_missing_directory() {
        assert!(scan_golden_dir(std::path::Path::new("/no/such/dir/xyzzy")).is_empty());
    }

    #[test]
    fn scan_clones_dir_reports_a_clone_with_a_live_monitor_socket() {
        let clones = tempfile::tempdir().unwrap();
        let sessions = tempfile::tempdir().unwrap();
        // Clone "testanyware-aa" has a monitor.sock in its session dir => running.
        let id = "testanyware-aa";
        fs::create_dir_all(clones.path().join(id)).unwrap();
        let sess = sessions.path().join(format!("testanyware-{id}"));
        fs::create_dir_all(&sess).unwrap();
        fs::write(sess.join("monitor.sock"), b"").unwrap();
        // Clone "testanyware-bb" has no session dir => not running.
        fs::create_dir_all(clones.path().join("testanyware-bb")).unwrap();

        let running = scan_clones_dir(clones.path(), sessions.path());
        let names: Vec<&str> = running.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(names, vec![id]);
    }

    #[test]
    fn build_qemu_args_wires_display_and_sockets() {
        let spec = QemuLaunchSpec {
            uefi_code: std::path::PathBuf::from("/fw/code.fd"),
            clone_efivars: std::path::PathBuf::from("/c/efivars.fd"),
            clone_qcow2: std::path::PathBuf::from("/c/disk.qcow2"),
            tpm_socket: std::path::PathBuf::from("/s/swtpm-sock"),
            monitor_socket: std::path::PathBuf::from("/s/monitor.sock"),
            display: Some("1920x1080".into()),
        };
        let args = build_qemu_args(&host_profile(), &spec);
        let joined = args.join(" ");
        assert!(joined.contains("xres=1920,yres=1080"), "display wired: {joined}");
        assert!(joined.contains("hostfwd=tcp::0-:8648"), "agent forward wired: {joined}");
        assert!(joined.contains("unix:/s/monitor.sock,server,nowait"), "monitor wired: {joined}");
        assert!(joined.contains("path=/s/swtpm-sock"), "tpm chardev wired: {joined}");
        assert!(joined.contains("password=on"), "vnc password gating wired: {joined}");
        assert!(args.contains(&"-accel".to_string()));
    }

    #[test]
    fn build_qemu_args_omits_display_geometry_when_absent() {
        let spec = QemuLaunchSpec {
            uefi_code: "/fw/code.fd".into(),
            clone_efivars: "/c/efivars.fd".into(),
            clone_qcow2: "/c/disk.qcow2".into(),
            tpm_socket: "/s/swtpm-sock".into(),
            monitor_socket: "/s/monitor.sock".into(),
            display: None,
        };
        let joined = build_qemu_args(&host_profile(), &spec).join(" ");
        assert!(!joined.contains("xres="), "no geometry without --display: {joined}");
        assert!(joined.contains("virtio-gpu-pci"), "still wires a GPU: {joined}");
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm qemu::`
Expected: FAIL — `qemu.rs` symbols not defined.

- [ ] **Step 3: Write `qemu.rs` above the test module**

```rust
//! QEMU / swtpm orchestration. Port of `QEMURunner.swift`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::VmError;
use crate::monitor::QemuMonitorClient;
use crate::paths::VmPaths;
use crate::preflight::{check_kvm, check_swtpm};
use crate::process::{pgrep_first, process_alive, terminate};
use crate::qemu_profile::{host_profile, resolve_uefi_code, which, QemuProfile};

/// A golden image discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenImage {
    pub name: String,
    pub platform: String,
    pub backend: &'static str,
}

/// A running QEMU clone discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunningClone {
    pub id: String,
    pub platform: String,
    pub backend: &'static str,
}

/// Inputs to `build_qemu_args` — the per-clone files and sockets.
#[derive(Debug, Clone)]
pub struct QemuLaunchSpec {
    pub uefi_code: PathBuf,
    pub clone_efivars: PathBuf,
    pub clone_qcow2: PathBuf,
    pub tpm_socket: PathBuf,
    pub monitor_socket: PathBuf,
    pub display: Option<String>,
}

/// Options for `QemuRunner::start`.
#[derive(Debug, Clone)]
pub struct QemuStartOptions {
    pub id: String,
    pub base: String,
    pub display: Option<String>,
    /// Whether the guest needs a TPM (Windows). Drives the swtpm preflight.
    pub needs_tpm: bool,
}

/// Result of a successful `QemuRunner::start`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StartArtifacts {
    pub pid: i32,
    pub vnc_port: u16,
    pub agent_port: Option<u16>,
    pub clone_dir: PathBuf,
}

/// Classify a golden / clone name into a platform string. Ports
/// `QEMURunner.platformFromName`.
pub fn platform_from_name(name: &str) -> String {
    if name.contains("macos") || name.contains("tahoe") {
        "macos".into()
    } else if name.contains("linux") || name.contains("ubuntu") {
        "linux".into()
    } else if name.contains("windows") {
        "windows".into()
    } else {
        "unknown".into()
    }
}

/// Scan `<golden>/*.qcow2`. Ports `QEMURunner.scanGoldenDir`.
pub fn scan_golden_dir(golden_dir: &Path) -> Vec<GoldenImage> {
    let Ok(entries) = std::fs::read_dir(golden_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        let file = entry.file_name();
        let file = file.to_string_lossy();
        if let Some(name) = file.strip_suffix(".qcow2") {
            out.push(GoldenImage {
                name: name.to_string(),
                platform: platform_from_name(name),
                backend: "qemu",
            });
        }
    }
    out
}

/// Scan `<clones>/` for running QEMU VMs by checking each clone's
/// TMPDIR-staged `monitor.sock`. Ports `QEMURunner.scanClonesDir` — the
/// clone subdirectory name is the VM id; the monitor socket lives at
/// `<sessions>/testanyware-<id>/monitor.sock`.
pub fn scan_clones_dir(clones_dir: &Path, sessions_root: &Path) -> Vec<RunningClone> {
    let Ok(entries) = std::fs::read_dir(clones_dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let sock = sessions_root.join(format!("testanyware-{id}")).join("monitor.sock");
        if sock.exists() {
            out.push(RunningClone {
                platform: platform_from_name(&id),
                id,
                backend: "qemu",
            });
        }
    }
    out
}

/// Build the QEMU argument vector. Pure — depends only on the host
/// profile and the per-clone spec. Ports the `qemuArgs` array in
/// `QEMURunner.start`, with the accelerator / machine / cpu taken from
/// the host profile rather than hard-coded to HVF/virt.
pub fn build_qemu_args(profile: &QemuProfile, spec: &QemuLaunchSpec) -> Vec<String> {
    let gpu = match &spec.display {
        Some(d) => {
            let parts: Vec<&str> = d.split('x').collect();
            if parts.len() == 2 {
                format!("virtio-gpu-pci,xres={},yres={}", parts[0], parts[1])
            } else {
                "virtio-gpu-pci".to_string()
            }
        }
        None => "virtio-gpu-pci".to_string(),
    };
    let s = |p: &Path| p.display().to_string();
    vec![
        "-machine".into(), profile.machine.into(),
        "-accel".into(), profile.accelerator.into(),
        "-cpu".into(), profile.cpu.into(),
        "-smp".into(), "4".into(),
        "-m".into(), "4096".into(),
        "-drive".into(), format!("if=pflash,format=raw,file={},readonly=on", s(&spec.uefi_code)),
        "-drive".into(), format!("if=pflash,format=raw,file={}", s(&spec.clone_efivars)),
        "-chardev".into(), format!("socket,id=chrtpm,path={}", s(&spec.tpm_socket)),
        "-tpmdev".into(), "emulator,id=tpm0,chardev=chrtpm".into(),
        "-device".into(), "tpm-tis-device,tpmdev=tpm0".into(),
        "-drive".into(), format!("file={},if=none,id=hd0,format=qcow2", s(&spec.clone_qcow2)),
        "-device".into(), "nvme,drive=hd0,serial=boot,bootindex=0".into(),
        "-device".into(), "ramfb".into(),
        "-device".into(), gpu,
        "-device".into(), "qemu-xhci".into(),
        "-device".into(), "usb-kbd".into(),
        "-device".into(), "usb-tablet".into(),
        "-device".into(), "virtio-net-pci,netdev=net0".into(),
        "-netdev".into(), "user,id=net0,hostfwd=tcp::0-:8648".into(),
        "-vnc".into(), "localhost:0,to=99,password=on".into(),
        "-monitor".into(), format!("unix:{},server,nowait", s(&spec.monitor_socket)),
        "-display".into(), "none".into(),
    ]
}

/// Run `program` with `args` synchronously, inheriting the parent's
/// stderr. Errors on a non-zero exit. Ports `QEMURunner.runAndCheck`.
async fn run_and_check(program: &str, args: &[String]) -> Result<(), VmError> {
    let status = tokio::process::Command::new(program)
        .args(args)
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .status()
        .await
        .map_err(|e| VmError::QemuFailed { detail: format!("{program}: {e}") })?;
    if status.success() {
        Ok(())
    } else {
        Err(VmError::QemuFailed {
            detail: format!("{program} {} exited {status}", args.join(" ")),
        })
    }
}

/// First PID holding any `.qcow2` in `dir`, via `lsof -t`. Ports
/// `QEMURunner.processHoldingQcow2`.
fn process_holding_qcow2(dir: &Path) -> Option<i32> {
    let output = std::process::Command::new("sh")
        .arg("-c")
        .arg("lsof -t \"$1\"/*.qcow2 2>/dev/null | head -1")
        .arg("testanyware-qcow2-holder")
        .arg(dir.display().to_string())
        .output()
        .ok()?;
    String::from_utf8_lossy(&output.stdout).trim().parse().ok()
}

/// QEMU / swtpm orchestration entry points.
pub struct QemuRunner;

impl QemuRunner {
    /// Clone the golden, start swtpm + QEMU detached, and discover the
    /// dynamic VNC/agent ports. Ports `QEMURunner.start`.
    pub async fn start(opts: &QemuStartOptions, paths: &VmPaths) -> Result<StartArtifacts, VmError> {
        // --- Preflight (constraint: KVM + swtpm) -------------------------
        check_kvm()?;
        if opts.needs_tpm {
            check_swtpm()?;
        }

        let clone_dir = paths.clone_dir(&opts.id);
        let golden_dir = paths.golden_dir();
        let session = paths.session_dir(&opts.id);

        // --- Reclaim a stale clone / session -----------------------------
        if clone_dir.exists() {
            if let Some(pid) = process_holding_qcow2(&clone_dir) {
                terminate(pid, Duration::from_millis(200), 10);
            }
            let _ = std::fs::remove_dir_all(&clone_dir);
        }
        if session.exists() {
            let _ = std::fs::remove_dir_all(&session);
        }
        std::fs::create_dir_all(&clone_dir)
            .map_err(|e| VmError::Io(format!("create {}: {e}", clone_dir.display())))?;
        std::fs::create_dir_all(&session)
            .map_err(|e| VmError::Io(format!("create {}: {e}", session.display())))?;

        // --- Clone the golden artefacts ----------------------------------
        let golden_qcow2 = golden_dir.join(format!("{}.qcow2", opts.base));
        let clone_qcow2 = clone_dir.join(format!("{}.qcow2", opts.id));
        let qemu_img = which("qemu-img")
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "qemu-img".to_string());
        run_and_check(&qemu_img, &[
            "create".into(), "-f".into(), "qcow2".into(),
            "-b".into(), golden_qcow2.display().to_string(),
            "-F".into(), "qcow2".into(),
            clone_qcow2.display().to_string(),
        ])
        .await
        .inspect_err(|_| teardown(0, &clone_dir, &session))?;

        let golden_efivars = golden_dir.join(format!("{}-efivars.fd", opts.base));
        let clone_efivars = clone_dir.join(format!("{}-efivars.fd", opts.id));
        std::fs::copy(&golden_efivars, &clone_efivars)
            .map_err(|e| VmError::Io(format!("copy efivars: {e}")))
            .inspect_err(|_| teardown(0, &clone_dir, &session))?;

        let golden_tpm = golden_dir.join(format!("{}-tpm", opts.base));
        let clone_tpm_dir = clone_dir.join(format!("{}-tpm", opts.id));
        run_and_check("cp", &[
            "-r".into(),
            golden_tpm.display().to_string(),
            clone_tpm_dir.display().to_string(),
        ])
        .await
        .inspect_err(|_| teardown(0, &clone_dir, &session))?;

        // --- Resolve UEFI firmware ---------------------------------------
        let profile = host_profile();
        let uefi_code = resolve_uefi_code(&profile.uefi_code_candidates).ok_or_else(|| {
            teardown(0, &clone_dir, &session);
            VmError::UefiNotFound {
                path: profile
                    .uefi_code_candidates
                    .first()
                    .map(|p| p.display().to_string())
                    .unwrap_or_else(|| "<no candidate paths>".into()),
            }
        })?;

        // --- Start swtpm (sockets staged under $TMPDIR) ------------------
        let tpm_socket = session.join("swtpm-sock");
        let monitor_socket = session.join("monitor.sock");
        let swtpm = which("swtpm")
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "swtpm".to_string());
        run_and_check(&swtpm, &[
            "socket".into(),
            "--tpmstate".into(), format!("dir={}", clone_tpm_dir.display()),
            "--ctrl".into(), format!("type=unixio,path={}", tpm_socket.display()),
            "--tpm2".into(),
            "--log".into(), "level=0".into(),
            "--daemon".into(),
        ])
        .await
        .inspect_err(|_| teardown(0, &clone_dir, &session))?;
        tokio::time::sleep(Duration::from_secs(1)).await;

        // --- Launch QEMU detached ----------------------------------------
        let launch = QemuLaunchSpec {
            uefi_code,
            clone_efivars,
            clone_qcow2,
            tpm_socket,
            monitor_socket: monitor_socket.clone(),
            display: opts.display.clone(),
        };
        let args = build_qemu_args(&profile, &launch);
        let qemu_bin = which(profile.qemu_binary)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| profile.qemu_binary.to_string());
        let log_path = clone_dir.join("qemu.log");
        let pid = crate::detached::spawn_detached(&qemu_bin, &args, &log_path)
            .inspect_err(|_| teardown(0, &clone_dir, &session))?;
        tokio::time::sleep(Duration::from_secs(1)).await;
        if !process_alive(pid) {
            teardown(0, &clone_dir, &session);
            return Err(VmError::QemuFailed {
                detail: "QEMU did not remain running after launch".into(),
            });
        }

        // --- Monitor: set VNC password, discover ports -------------------
        let monitor = QemuMonitorClient::new(&monitor_socket);
        monitor.set_vnc_password("testanyware", 3).await;

        let agent_port = monitor.agent_port(5, Duration::from_secs(1)).await;
        if agent_port.is_none() {
            teardown(pid, &clone_dir, &session);
            return Err(VmError::MonitorDiscoveryFailed);
        }
        let vnc_port = monitor
            .vnc_port(3, Duration::from_millis(500))
            .await
            .unwrap_or(5900);

        Ok(StartArtifacts { pid, vnc_port, agent_port, clone_dir })
    }

    /// Tear down a running QEMU VM. Public wrapper deriving the session
    /// dir from the clone-dir basename (the VM id). Ports `QEMURunner.stop`.
    pub fn stop(pid: i32, clone_dir: &Path, paths: &VmPaths) {
        let id = clone_dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        teardown(pid, clone_dir, &paths.session_dir(&id));
    }

    /// Remove a golden image's three artefacts. Idempotent. Ports
    /// `QEMURunner.deleteGolden`.
    pub fn delete_golden(name: &str, golden_dir: &Path) {
        let _ = std::fs::remove_file(golden_dir.join(format!("{name}.qcow2")));
        let _ = std::fs::remove_file(golden_dir.join(format!("{name}-efivars.fd")));
        let _ = std::fs::remove_dir_all(golden_dir.join(format!("{name}-tpm")));
    }

    /// PIDs of running clones whose backing qcow2 is `golden_name`. Ports
    /// `QEMURunner.runningClonesBacked`.
    pub fn running_clones_backed_by(golden_name: &str, paths: &VmPaths) -> Vec<i32> {
        let golden_qcow2 = paths.golden_dir().join(format!("{golden_name}.qcow2"));
        let Ok(entries) = std::fs::read_dir(paths.clones_dir()) else {
            return Vec::new();
        };
        let mut pids = Vec::new();
        for entry in entries.flatten() {
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let Ok(files) = std::fs::read_dir(&dir) else { continue };
            for f in files.flatten() {
                let p = f.path();
                if p.extension().and_then(|e| e.to_str()) != Some("qcow2") {
                    continue;
                }
                if backing_file(&p).as_deref() == Some(golden_qcow2.as_path()) {
                    if let Some(pid) = process_holding_qcow2(&dir) {
                        pids.push(pid);
                    }
                }
            }
        }
        pids
    }
}

/// Shared teardown: SIGTERM→SIGKILL the qemu pid, pgrep-kill the swtpm
/// daemon by its TPM state-dir path, then remove the clone + session
/// dirs. Idempotent — `pid: 0` skips the qemu kill. Ports
/// `QEMURunner.teardown`.
pub fn teardown(pid: i32, clone_dir: &Path, session_dir: &Path) {
    if pid > 0 {
        terminate(pid, Duration::from_millis(100), 20);
    }
    // swtpm has no registry: locate it by its --tpmstate dir and kill it.
    let clone_name = clone_dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tpm_dir = clone_dir.join(format!("{clone_name}-tpm"));
    if let Some(swtpm_pid) = pgrep_first(&format!("swtpm.*{}", tpm_dir.display())) {
        terminate(swtpm_pid, Duration::from_millis(200), 5);
    }
    if clone_dir.exists() {
        let _ = std::fs::remove_dir_all(clone_dir);
    }
    if session_dir.exists() {
        let _ = std::fs::remove_dir_all(session_dir);
    }
}

/// `full-backing-filename` from `qemu-img info --output=json`. Ports
/// `QEMURunner.backingFile`.
fn backing_file(qcow2: &Path) -> Option<PathBuf> {
    let qemu_img = which("qemu-img")?;
    let output = std::process::Command::new(qemu_img)
        .args(["info", "--output=json"])
        .arg(qcow2)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    json.get("full-backing-filename")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
}
```

- [ ] **Step 4: Add the module to `lib.rs`** — add `pub mod qemu;`.

- [ ] **Step 5: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm qemu::`
Expected: PASS (6 tests).

- [ ] **Step 6: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/qemu.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): QEMU runner — scanners, arg builder, start, teardown"
```

---

## Task 11: Agent health waiter

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/health.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

Ports `AgentHealthWaiter.swift`. Rather than re-implement an HTTP client, this reuses `testanyware-agent-client`'s `AgentClient::health()` in a poll loop with a short per-attempt timeout. Returns `true` on the first reachable+healthy response.

- [ ] **Step 1: Write the failing tests** — create `health.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn host_port(server: &MockServer) -> (String, u16) {
        let uri = server.uri(); // http://127.0.0.1:PORT
        let rest = uri.strip_prefix("http://").unwrap();
        let (h, p) = rest.rsplit_once(':').unwrap();
        (h.to_string(), p.parse().unwrap())
    }

    #[tokio::test]
    async fn returns_true_when_health_responds_ok() {
        let server = MockServer::start().await;
        // The agent's /health returns a JSON HealthResponse body.
        Mock::given(method("GET"))
            .and(path("/health"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "accessible": true,
                "platform": "linux"
            })))
            .mount(&server)
            .await;
        let (host, port) = host_port(&server);
        let ready = wait_for_agent(&host, port, 3, Duration::from_millis(50)).await;
        assert!(ready, "a 200 /health must resolve the waiter to true");
    }

    #[tokio::test]
    async fn returns_false_when_budget_is_exhausted() {
        // Nothing listening on this port — every attempt fails to connect.
        let ready = wait_for_agent("127.0.0.1", 1, 2, Duration::from_millis(20)).await;
        assert!(!ready, "no agent => waiter exhausts its budget and returns false");
    }
}
```

> Note: `port: 1` is privileged and not listening, so `connect` fails fast. If a CI host flakes here, substitute any port the runner can prove is closed.

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm health::`
Expected: FAIL — `wait_for_agent` not defined.

- [ ] **Step 3: Write `health.rs` above the test module**

```rust
//! Poll the in-VM agent's `/health` endpoint until it is reachable.
//!
//! Port of `AgentHealthWaiter.swift`. `VMLifecycle` uses this to decide
//! whether to populate the `agent` field on the spec file.

use std::time::Duration;

use testanyware_agent_client::{AgentClient, AgentConfig};

/// Poll `http://<host>:<port>/health` up to `attempts` times, `interval`
/// apart. Returns `true` on the first healthy response. Connection
/// failures and errors are treated uniformly as "not ready yet".
pub async fn wait_for_agent(host: &str, port: u16, attempts: u32, interval: Duration) -> bool {
    // A short per-request timeout keeps each poll snappy; the loop, not
    // the socket, owns the overall budget.
    let config = AgentConfig::new(host, port).with_timeout(Duration::from_secs(2));
    let Ok(client) = AgentClient::new(config) else {
        return false;
    };
    for attempt in 0..attempts {
        if client.health().await.is_ok() {
            return true;
        }
        if attempt + 1 < attempts {
            tokio::time::sleep(interval).await;
        }
    }
    false
}
```

- [ ] **Step 4: Add the module to `lib.rs`** — add `pub mod health;` and `pub use health::wait_for_agent;`.

- [ ] **Step 5: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm health::`
Expected: PASS (2 tests).

- [ ] **Step 6: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/health.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): agent health waiter via testanyware-agent-client"
```

---

## Task 12: Lifecycle orchestrator

**Files:**
- Create: `cli-rs/crates/testanyware-vm/src/lifecycle.rs`
- Modify: `cli-rs/crates/testanyware-vm/src/lib.rs`

Ports `VMLifecycle.swift` (QEMU paths only — tart is task 12 of the backlog). Composes the runner, health waiter, and sidecars into `start` / `stop` / `delete`. Defines `Platform`, `VmStartOptions`, `VmStartResult`, and the `VmListing` type for `vm list`.

The tart backend is not ported: `start` for a `macos` platform returns `VmError::BackendUnsupported`; `stop` of a `VmTool::Tart` meta likewise. A Swift-started **QEMU** VM still stops cleanly (its meta has `tool: "qemu"`, `pid`, `clone_dir`), satisfying the cross-tooling acceptance criterion.

- [ ] **Step 1: Write the failing tests** — create `lifecycle.rs` with this test module:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn paths_in(dir: &std::path::Path) -> VmPaths {
        let mut env = HashMap::new();
        env.insert("XDG_STATE_HOME".into(), dir.join("state").display().to_string());
        env.insert("XDG_DATA_HOME".into(), dir.join("data").display().to_string());
        env.insert("TMPDIR".into(), dir.join("tmp").display().to_string());
        VmPaths::from_env(&env)
    }

    #[test]
    fn platform_parses_the_three_known_values() {
        assert_eq!(Platform::parse("linux").unwrap(), Platform::Linux);
        assert_eq!(Platform::parse("windows").unwrap(), Platform::Windows);
        assert_eq!(Platform::parse("macos").unwrap(), Platform::Macos);
        assert!(Platform::parse("bsd").is_err());
    }

    #[test]
    fn platform_default_base_matches_golden_naming() {
        assert_eq!(Platform::Linux.default_base(), "testanyware-golden-linux-24.04");
        assert_eq!(Platform::Windows.default_base(), "testanyware-golden-windows-11");
        assert_eq!(Platform::Macos.default_base(), "testanyware-golden-macos-tahoe");
    }

    #[test]
    fn start_options_fill_in_base_and_id_defaults() {
        let opts = VmStartOptions::new(Platform::Windows, None, None, None, false);
        assert_eq!(opts.base, "testanyware-golden-windows-11");
        assert!(opts.id.starts_with("testanyware-"));
        let explicit = VmStartOptions::new(
            Platform::Linux,
            Some("custom-base".into()),
            Some("testanyware-fixedid".into()),
            Some("800x600".into()),
            false,
        );
        assert_eq!(explicit.base, "custom-base");
        assert_eq!(explicit.id, "testanyware-fixedid");
        assert_eq!(explicit.display.as_deref(), Some("800x600"));
    }

    #[tokio::test]
    async fn start_rejects_the_macos_platform_as_unsupported() {
        let dir = tempfile::tempdir().unwrap();
        let opts = VmStartOptions::new(Platform::Macos, None, None, None, false);
        let err = VmLifecycle::start(&opts, &paths_in(dir.path())).await.unwrap_err();
        assert!(matches!(err, VmError::BackendUnsupported { .. }));
    }

    #[test]
    fn delete_reports_golden_not_found_for_an_absent_image() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.golden_dir()).unwrap();
        let err = VmLifecycle::delete("testanyware-golden-ghost", false, &paths).unwrap_err();
        assert!(matches!(err, VmError::GoldenNotFound { .. }));
    }

    #[test]
    fn stop_reports_vm_not_found_when_no_meta_exists() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.vms_dir()).unwrap();
        let err = VmLifecycle::stop("testanyware-ghost", &paths).unwrap_err();
        assert!(matches!(err, VmError::VmNotFound { .. }));
    }

    #[test]
    fn list_returns_goldens_and_running_clones() {
        let dir = tempfile::tempdir().unwrap();
        let paths = paths_in(dir.path());
        std::fs::create_dir_all(paths.golden_dir()).unwrap();
        std::fs::write(paths.golden_dir().join("testanyware-golden-linux-24.04.qcow2"), b"x").unwrap();
        let listing = VmLifecycle::list(&paths);
        assert_eq!(listing.goldens.len(), 1);
        assert_eq!(listing.running.len(), 0);
    }
}
```

- [ ] **Step 2: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-vm lifecycle::`
Expected: FAIL — `lifecycle.rs` symbols not defined.

- [ ] **Step 3: Write `lifecycle.rs` above the test module**

```rust
//! End-to-end VM lifecycle orchestrator. Port of `VMLifecycle.swift`
//! (QEMU paths only; tart is a separate backlog task).

use std::path::PathBuf;
use std::time::Duration;

use crate::error::VmError;
use crate::health::wait_for_agent;
use crate::id::generate_id;
use crate::meta::{VmMeta, VmTool};
use crate::paths::VmPaths;
use crate::process::process_alive;
use crate::qemu::{
    scan_clones_dir, scan_golden_dir, GoldenImage, QemuRunner, QemuStartOptions, RunningClone,
};
use crate::spec::{AgentEndpoint, VmSpec, VncEndpoint};

/// Guest platform. Ports the `Platform` enum + extension in `VMTypes.swift`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Platform {
    Macos,
    Linux,
    Windows,
}

impl Platform {
    /// Parse a `--platform` string. Errors with `InvalidPlatform`.
    pub fn parse(value: &str) -> Result<Self, VmError> {
        match value {
            "macos" => Ok(Platform::Macos),
            "linux" => Ok(Platform::Linux),
            "windows" => Ok(Platform::Windows),
            other => Err(VmError::InvalidPlatform { value: other.to_string() }),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Platform::Macos => "macos",
            Platform::Linux => "linux",
            Platform::Windows => "windows",
        }
    }

    /// Default golden-image name. Ports `Platform.defaultBase`.
    pub fn default_base(self) -> &'static str {
        match self {
            Platform::Macos => "testanyware-golden-macos-tahoe",
            Platform::Linux => "testanyware-golden-linux-24.04",
            Platform::Windows => "testanyware-golden-windows-11",
        }
    }

    /// Windows guests need a TPM 2.0 socket.
    fn needs_tpm(self) -> bool {
        matches!(self, Platform::Windows)
    }
}

/// Inputs for `VmLifecycle::start`. Ports `VMStartOptions`.
#[derive(Debug, Clone)]
pub struct VmStartOptions {
    pub platform: Platform,
    pub base: String,
    pub id: String,
    pub display: Option<String>,
    pub open_viewer: bool,
}

impl VmStartOptions {
    pub fn new(
        platform: Platform,
        base: Option<String>,
        id: Option<String>,
        display: Option<String>,
        open_viewer: bool,
    ) -> Self {
        Self {
            platform,
            base: base.unwrap_or_else(|| platform.default_base().to_string()),
            id: id.unwrap_or_else(generate_id),
            display,
            open_viewer,
        }
    }
}

/// Result of a successful `VmLifecycle::start`.
#[derive(Debug, Clone)]
pub struct VmStartResult {
    pub id: String,
    pub platform: Platform,
    pub spec: VmSpec,
    pub spec_path: PathBuf,
    pub meta_path: PathBuf,
    /// `true` when the agent did not reach health within the boot window
    /// (the VM still started; agent commands will fail until it comes up).
    pub agent_unreachable: bool,
}

/// One `vm list` row.
#[derive(Debug, Clone)]
pub enum VmListItem {
    Golden(GoldenImage),
    Running(RunningEntry),
}

/// A running clone enriched from its spec/meta sidecars.
#[derive(Debug, Clone)]
pub struct RunningEntry {
    pub id: String,
    pub platform: String,
    pub backend: &'static str,
    pub pid: Option<i32>,
    pub vnc: Option<String>,
    pub agent: Option<String>,
}

/// `vm list` output: goldens + running clones.
#[derive(Debug, Clone, Default)]
pub struct VmListing {
    pub goldens: Vec<GoldenImage>,
    pub running: Vec<RunningEntry>,
}

/// Lifecycle entry points.
pub struct VmLifecycle;

impl VmLifecycle {
    /// Start a VM end-to-end. QEMU backend only — `macos` (tart) returns
    /// `BackendUnsupported`. Ports `VMLifecycle.start` / `startQEMU`.
    pub async fn start(opts: &VmStartOptions, paths: &VmPaths) -> Result<VmStartResult, VmError> {
        if opts.platform == Platform::Macos {
            return Err(VmError::BackendUnsupported { platform: "macos".into() });
        }
        std::fs::create_dir_all(paths.vms_dir())
            .map_err(|e| VmError::Io(format!("create {}: {e}", paths.vms_dir().display())))?;

        let qopts = QemuStartOptions {
            id: opts.id.clone(),
            base: opts.base.clone(),
            display: opts.display.clone(),
            needs_tpm: opts.platform.needs_tpm(),
        };
        let artifacts = QemuRunner::start(&qopts, paths).await?;

        // Wait for the agent. Unreachable is a warning, not a failure —
        // the VM started; the spec is written with `agent: null`. Matches
        // Swift `startQEMU` (120 attempts × 5 s).
        let mut agent_endpoint = None;
        let mut agent_unreachable = true;
        if let Some(port) = artifacts.agent_port {
            if wait_for_agent("localhost", port, 120, Duration::from_secs(5)).await {
                agent_endpoint = Some(AgentEndpoint { host: "localhost".into(), port });
                agent_unreachable = false;
            }
        }

        let spec = VmSpec {
            vnc: VncEndpoint {
                host: "localhost".into(),
                port: artifacts.vnc_port,
                password: Some("testanyware".into()),
            },
            agent: agent_endpoint,
            platform: opts.platform.as_str().to_string(),
        };
        let spec_path = paths.spec_path(&opts.id);
        let meta_path = paths.meta_path(&opts.id);
        spec.write_atomic(&spec_path)?;

        // Viewer wiring is backlog task 8; `viewer_window_id` stays null.
        let meta = VmMeta {
            id: opts.id.clone(),
            tool: VmTool::Qemu,
            pid: artifacts.pid,
            clone_dir: Some(artifacts.clone_dir.display().to_string()),
            viewer_window_id: None,
        };
        meta.write_atomic(&meta_path)?;

        Ok(VmStartResult {
            id: opts.id.clone(),
            platform: opts.platform,
            spec,
            spec_path,
            meta_path,
            agent_unreachable,
        })
    }

    /// Stop a VM and remove its sidecars. Ports `VMLifecycle.stop`
    /// (QEMU branch). A `tart` meta returns `BackendUnsupported`.
    pub fn stop(id: &str, paths: &VmPaths) -> Result<(), VmError> {
        let spec_path = paths.spec_path(id);
        let meta_path = paths.meta_path(id);
        if !meta_path.is_file() {
            return Err(VmError::VmNotFound { id: id.to_string() });
        }
        let meta = VmMeta::load(&meta_path)?;
        match meta.tool {
            VmTool::Tart => {
                return Err(VmError::BackendUnsupported { platform: "macos (tart)".into() });
            }
            VmTool::Qemu => {
                let clone_dir = meta.clone_dir.clone().ok_or_else(|| VmError::VmStopFailed {
                    id: id.to_string(),
                })?;
                QemuRunner::stop(meta.pid, std::path::Path::new(&clone_dir), paths);
            }
        }
        let _ = std::fs::remove_file(&spec_path);
        let _ = std::fs::remove_file(&meta_path);
        Ok(())
    }

    /// Delete a QEMU golden image by name. Refuses when running clones
    /// depend on it unless `force`. Ports `VMLifecycle.delete` (QEMU
    /// branch; tart detection is backlog task 12).
    pub fn delete(name: &str, force: bool, paths: &VmPaths) -> Result<(), VmError> {
        let golden_dir = paths.golden_dir();
        let qcow2 = golden_dir.join(format!("{name}.qcow2"));
        if !qcow2.is_file() {
            return Err(VmError::GoldenNotFound { name: name.to_string() });
        }
        if !force {
            let pids = QemuRunner::running_clones_backed_by(name, paths);
            if !pids.is_empty() {
                return Err(VmError::GoldenInUse { name: name.to_string(), clone_pids: pids });
            }
        }
        QemuRunner::delete_golden(name, &golden_dir);
        Ok(())
    }

    /// List goldens and running clones. Ports `VMCommand.List` (QEMU
    /// entries only). Running clones are enriched from their sidecars.
    pub fn list(paths: &VmPaths) -> VmListing {
        let goldens = scan_golden_dir(&paths.golden_dir());
        let raw: Vec<RunningClone> =
            scan_clones_dir(&paths.clones_dir(), &session_root(paths));
        let running = raw
            .into_iter()
            .map(|clone| enrich_running(&clone, paths))
            .collect();
        VmListing { goldens, running }
    }
}

/// The `$TMPDIR` root for socket session dirs — `session_dir` of any id
/// shares the same parent, so the parent of an arbitrary id's session dir
/// is that root.
fn session_root(paths: &VmPaths) -> PathBuf {
    paths
        .session_dir("_")
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/tmp"))
}

/// Enrich a bare running clone with spec (vnc/agent) and meta (pid) data.
fn enrich_running(clone: &RunningClone, paths: &VmPaths) -> RunningEntry {
    let spec = VmSpec::load(&paths.spec_path(&clone.id)).ok();
    let meta = VmMeta::load(&paths.meta_path(&clone.id)).ok();
    let pid = meta.as_ref().map(|m| m.pid).filter(|p| process_alive(*p));
    let vnc = spec
        .as_ref()
        .map(|s| format!("{}:{}", s.vnc.host, s.vnc.port));
    let agent = spec
        .as_ref()
        .and_then(|s| s.agent.as_ref())
        .map(|a| format!("{}:{}", a.host, a.port));
    RunningEntry {
        id: clone.id.clone(),
        platform: clone.platform.clone(),
        backend: clone.backend,
        pid,
        vnc,
        agent,
    }
}
```

> Note on `VmListItem`: it is declared for the JSON unification in Task 15's handler but not consumed inside the crate. Add `#[allow(dead_code)]` above the `enum VmListItem` declaration, or omit `VmListItem` entirely and have the CLI handler build the unified `items` array directly from `VmListing.goldens` / `.running`. The plan's Task 15 builds from `VmListing` directly — **omit `VmListItem`** to avoid dead code. Remove the `VmListItem` enum and `RunningEntry`-as-variant wording above; keep only `RunningEntry`, `GoldenImage`, `VmListing`.

- [ ] **Step 4: Add the module to `lib.rs`**

Add `pub mod lifecycle;` and re-exports:

```rust
pub use lifecycle::{Platform, VmLifecycle, VmListing, VmStartOptions, VmStartResult};
pub use qemu::{GoldenImage, QemuRunner};
```

- [ ] **Step 5: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-vm`
Expected: PASS — full crate suite (≈ 40 tests across all modules).

- [ ] **Step 6: Lint the crate**

Run: `cd cli-rs && cargo clippy -p testanyware-vm --all-targets -- -D warnings`
Expected: no warnings. Fix any that appear (most likely `needless_borrow` / `redundant_clone`).

- [ ] **Step 7: Commit**

```bash
git add cli-rs/crates/testanyware-vm/src/lifecycle.rs cli-rs/crates/testanyware-vm/src/lib.rs
git commit -m "feat(vm): VM lifecycle orchestrator (start, stop, delete, list)"
```

---

## Task 13: Error-code catalogue and contract amendment

**Files:**
- Modify: `docs/architecture/cli-design-contract.md`
- Modify: `cli-rs/crates/testanyware-cli/src/surface.rs`
- Modify: `cli-rs/crates/testanyware-cli/src/output.rs`

The backlog item mandates two error codes — `KVM_PERMISSION_DENIED` and `SWTPM_MISSING` — not currently in contract §4 or `surface.rs`. The contract requires a deviation to be raised as an amendment first. This task adds them in all three places.

- [ ] **Step 1: Amend the contract §4.2 table**

In `docs/architecture/cli-design-contract.md`, in the §4.2 "VM lifecycle" table, add two rows after the `QEMU_FAILED` row:

```
| `KVM_PERMISSION_DENIED` | `/dev/kvm` is missing or not readable+writable (Linux host). `details.path` carries `/dev/kvm`. |
| `SWTPM_MISSING` | swtpm is not installed; required for Windows guests (TPM 2.0 socket). |
```

Then, immediately below the §4.2 table, add an amendment note:

```markdown
> **Amendment 2026-05-22** (`port-qemu-runner-and-vm-lifecycle-to-rust`):
> `KVM_PERMISSION_DENIED` and `SWTPM_MISSING` added to support the QEMU
> runner's host preflight. `KVM_PERMISSION_DENIED` maps to exit code `4`
> (permission family, §5); `SWTPM_MISSING` maps to exit code `1` (generic
> — a missing optional dependency, recoverable by installing it).
```

- [ ] **Step 2: Add the codes to `surface.rs`**

In `cli-rs/crates/testanyware-cli/src/surface.rs`, in `ERROR_CODES`, in the `// §4.2` block, add after `"QEMU_FAILED",`:

```rust
    "KVM_PERMISSION_DENIED",
    "SWTPM_MISSING",
```

- [ ] **Step 3: Write the failing exit-code test**

In `cli-rs/crates/testanyware-cli/src/output.rs`, in the `tests` module, add to `exit_code_table_matches_contract_section_5`:

```rust
        assert_eq!(exit_code_for("KVM_PERMISSION_DENIED"), 4);
        assert_eq!(exit_code_for("SWTPM_MISSING"), 1);
```

- [ ] **Step 4: Run to confirm it fails**

Run: `cd cli-rs && cargo test -p testanyware-cli output::tests::exit_code_table`
Expected: FAIL — `KVM_PERMISSION_DENIED` falls through to `1`, not `4`.

- [ ] **Step 5: Map the codes in `exit_code_for`**

In `output.rs::exit_code_for`, add `"KVM_PERMISSION_DENIED"` to the `=> 4` arm (alongside `"AUTH_REQUIRED"`):

```rust
        "AUTH_REQUIRED" | "KVM_PERMISSION_DENIED" => 4,
```

`SWTPM_MISSING` needs no arm — it correctly falls through to the `_ => 1` default. (The test asserts `1` for it, which the default already yields.)

- [ ] **Step 6: Run to confirm it passes**

Run: `cd cli-rs && cargo test -p testanyware-cli output::`
Expected: PASS.

- [ ] **Step 7: Verify `capabilities` still lists the full catalogue**

Run: `cd cli-rs && cargo test -p testanyware-cli --test cli-contract capabilities_lists_full_surface`
Expected: PASS — the test spot-checks codes but iterates `ERROR_CODES`, so the two new entries surface in `capabilities --json` automatically.

- [ ] **Step 8: Commit**

```bash
git add docs/architecture/cli-design-contract.md cli-rs/crates/testanyware-cli/src/surface.rs cli-rs/crates/testanyware-cli/src/output.rs
git commit -m "feat(cli): add KVM_PERMISSION_DENIED and SWTPM_MISSING error codes

Contract §4.2 amendment for the QEMU runner host preflight."
```

---

## Task 14: JSON schemas for the four vm commands

**Files:**
- Modify: `docs/reference/cli-schemas/vm-start.json`
- Modify: `docs/reference/cli-schemas/vm-stop.json`
- Modify: `docs/reference/cli-schemas/vm-list.json`
- Modify: `docs/reference/cli-schemas/vm-delete.json`

The four files are currently stubs. Replace them with real JSON Schema 2020-12 documents matching the `--json` envelopes the Task 15 handlers emit. The discoverability layer already embeds these paths via `include_str!`, and `tests/cli-contract.rs::schema_command_emits_json_schema_for_each_command` asserts `testanyware schema vm <verb>` output equals the file byte-for-byte — so the handler output and these files must agree.

- [ ] **Step 1: Replace `docs/reference/cli-schemas/vm-start.json`**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://testanyware.dev/schemas/vm-start.json",
  "title": "vm-start",
  "description": "Receipt for `testanyware vm start`. Identifies the started VM and its endpoints.",
  "type": "object",
  "required": ["schema_version", "ok", "id", "platform", "vnc"],
  "properties": {
    "schema_version": { "type": "string" },
    "ok": { "type": "boolean" },
    "dry_run": { "type": "boolean", "description": "Present and true when --dry-run was set." },
    "id": { "type": "string", "pattern": "^testanyware-[0-9a-f]{8}$" },
    "platform": { "type": "string", "enum": ["linux", "windows", "macos"] },
    "base": { "type": "string", "description": "Golden image cloned for this VM." },
    "vnc": {
      "type": "object",
      "required": ["host", "port"],
      "properties": {
        "host": { "type": "string" },
        "port": { "type": "integer" }
      }
    },
    "agent": {
      "type": ["object", "null"],
      "description": "Agent endpoint, or null if the agent did not reach health within the boot window.",
      "properties": {
        "host": { "type": "string" },
        "port": { "type": "integer" }
      }
    },
    "spec_path": { "type": "string" },
    "meta_path": { "type": "string" }
  }
}
```

- [ ] **Step 2: Replace `docs/reference/cli-schemas/vm-stop.json`**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://testanyware.dev/schemas/vm-stop.json",
  "title": "vm-stop",
  "description": "Mutation receipt for `testanyware vm stop`.",
  "type": "object",
  "required": ["schema_version", "ok", "id", "stopped"],
  "properties": {
    "schema_version": { "type": "string" },
    "ok": { "type": "boolean" },
    "dry_run": { "type": "boolean", "description": "Present and true when --dry-run was set." },
    "id": { "type": "string", "pattern": "^testanyware-[0-9a-f]{8}$" },
    "stopped": { "type": "boolean", "description": "True once QEMU + swtpm are terminated and the sidecars removed." }
  }
}
```

- [ ] **Step 3: Replace `docs/reference/cli-schemas/vm-list.json`**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://testanyware.dev/schemas/vm-list.json",
  "title": "vm-list",
  "description": "Golden images and running clones. Envelope follows contract §3.5 (returned/total/truncated).",
  "type": "object",
  "required": ["schema_version", "ok", "items", "returned", "total", "truncated"],
  "properties": {
    "schema_version": { "type": "string" },
    "ok": { "type": "boolean" },
    "items": {
      "type": "array",
      "items": {
        "type": "object",
        "required": ["kind", "name", "platform", "backend"],
        "properties": {
          "kind": { "type": "string", "enum": ["golden", "running"] },
          "name": { "type": "string", "description": "Golden image name, or VM id for running clones." },
          "platform": { "type": "string" },
          "backend": { "type": "string", "enum": ["qemu", "tart"] },
          "pid": { "type": ["integer", "null"] },
          "vnc": { "type": ["string", "null"] },
          "agent": { "type": ["string", "null"] }
        }
      }
    },
    "returned": { "type": "integer" },
    "total": { "type": "integer" },
    "truncated": { "type": "boolean" }
  }
}
```

- [ ] **Step 4: Replace `docs/reference/cli-schemas/vm-delete.json`**

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "https://testanyware.dev/schemas/vm-delete.json",
  "title": "vm-delete",
  "description": "Mutation receipt for `testanyware vm delete`.",
  "type": "object",
  "required": ["schema_version", "ok", "name", "deleted"],
  "properties": {
    "schema_version": { "type": "string" },
    "ok": { "type": "boolean" },
    "dry_run": { "type": "boolean", "description": "Present and true when --dry-run was set." },
    "name": { "type": "string" },
    "deleted": { "type": "boolean" }
  }
}
```

- [ ] **Step 5: Verify the schema files parse and stay embedded**

Run: `cd cli-rs && cargo test -p testanyware-cli --test cli-contract every_schema_id_has_a_schema_file`
Expected: PASS — all four files are valid JSON. (`schema_command_emits_json_schema_for_each_command` also still passes here: it compares `schema vm start` embedded output to the file, and both are the same file content.)

- [ ] **Step 6: Commit**

```bash
git add docs/reference/cli-schemas/vm-start.json docs/reference/cli-schemas/vm-stop.json docs/reference/cli-schemas/vm-list.json docs/reference/cli-schemas/vm-delete.json
git commit -m "feat(cli): real JSON schemas for vm start/stop/list/delete"
```

---

## Task 15: CLI command handlers (`commands/vm.rs`)

**Files:**
- Create: `cli-rs/crates/testanyware-cli/src/commands/vm.rs`
- Modify: `cli-rs/crates/testanyware-cli/src/commands/mod.rs`
- Modify: `cli-rs/crates/testanyware-cli/Cargo.toml`

The handlers bridge clap-parsed args to `testanyware_vm` and emit the §3 envelope. Pure helpers (`parse_filter`, `filter_matches`, `listing_items`, `apply_limit`) are unit-tested; the side-effecting handlers are exercised by the live smoke (Task 19) and the contract tests (Task 17).

- [ ] **Step 1: Add the crate dependency**

In `cli-rs/crates/testanyware-cli/Cargo.toml`, under `[dependencies]`, add after `testanyware-rfb = ...`:

```toml
testanyware-vm = { path = "../testanyware-vm" }
```

- [ ] **Step 2: Declare the module** — in `commands/mod.rs`, add `pub mod vm;` after `pub mod screen;`.

- [ ] **Step 3: Write the failing tests** — create `commands/vm.rs` with this test module (impl added in Step 5):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_filter_reads_comma_separated_pairs() {
        let f = parse_filter("platform=windows,backend=qemu").unwrap();
        assert_eq!(f, vec![
            ("platform".to_string(), "windows".to_string()),
            ("backend".to_string(), "qemu".to_string()),
        ]);
    }

    #[test]
    fn parse_filter_rejects_a_pair_without_equals() {
        assert!(parse_filter("platform").is_err());
        assert!(parse_filter("platform=windows,oops").is_err());
    }

    #[test]
    fn parse_filter_empty_string_is_no_filters() {
        assert_eq!(parse_filter("").unwrap(), Vec::<(String, String)>::new());
    }

    #[test]
    fn filter_matches_compares_known_fields() {
        let item = ListItem {
            kind: "running",
            name: "testanyware-aa".into(),
            platform: "windows".into(),
            backend: "qemu",
            pid: Some(7),
            vnc: None,
            agent: None,
        };
        assert!(filter_matches(&item, &[("platform".into(), "windows".into())]));
        assert!(!filter_matches(&item, &[("platform".into(), "linux".into())]));
        assert!(filter_matches(&item, &[("kind".into(), "running".into())]));
        // Unknown fields never match — surfaces a typo as an empty result.
        assert!(!filter_matches(&item, &[("colour".into(), "blue".into())]));
    }

    #[test]
    fn apply_limit_truncates_and_flags() {
        let items: Vec<u8> = (0..150).collect();
        let (shown, returned, total, truncated) = apply_limit(items.clone(), 100, false);
        assert_eq!(returned, 100);
        assert_eq!(total, 150);
        assert!(truncated);
        assert_eq!(shown.len(), 100);

        let (shown_all, returned_all, total_all, truncated_all) =
            apply_limit(items, 100, true);
        assert_eq!(returned_all, 150);
        assert_eq!(total_all, 150);
        assert!(!truncated_all);
        assert_eq!(shown_all.len(), 150);
    }
}
```

- [ ] **Step 4: Run the tests to confirm they fail**

Run: `cd cli-rs && cargo test -p testanyware-cli commands::vm::`
Expected: FAIL — `commands/vm.rs` symbols not defined.

- [ ] **Step 5: Write `commands/vm.rs` above the test module**

```rust
//! `vm {start|stop|list|delete}` command handlers.
//!
//! Bridges clap-parsed args to the `testanyware-vm` crate and emits the
//! contract §3 JSON envelope (or text). Ports the surface of
//! `cli/Sources/testanyware/VMCommand.swift`.

use serde_json::{json, Value};

use testanyware_vm::lifecycle::{Platform, VmLifecycle, VmListing, VmStartOptions};
use testanyware_vm::preflight::{check_kvm, check_swtpm};
use testanyware_vm::{VmError, VmMeta, VmPaths};

use crate::output::{print_error, print_success, OutputMode};

/// A flattened `vm list` row, ready for JSON or text rendering.
pub struct ListItem {
    pub kind: &'static str,
    pub name: String,
    pub platform: String,
    pub backend: &'static str,
    pub pid: Option<i32>,
    pub vnc: Option<String>,
    pub agent: Option<String>,
}

// ---- handlers -----------------------------------------------------------

/// `testanyware vm start`.
pub async fn run_vm_start(
    platform: String,
    base: Option<String>,
    id: Option<String>,
    display: Option<String>,
    viewer: bool,
    mode: OutputMode,
    dry_run: bool,
) {
    let parsed = match Platform::parse(&platform) {
        Ok(p) => p,
        Err(err) => exit_vm_error(err, mode),
    };
    if viewer {
        eprintln!(
            "note: --viewer is not yet ported to the Rust CLI (backlog task 8); \
             starting the VM without a viewer window."
        );
    }
    let opts = VmStartOptions::new(parsed, base, id, display, viewer);
    let paths = VmPaths::from_process_env();

    if dry_run {
        // Validate without side effects: golden present + host preflight.
        let golden = paths.golden_dir().join(format!("{}.qcow2", opts.base));
        if !golden.is_file() {
            exit_vm_error(VmError::GoldenNotFound { name: opts.base.clone() }, mode);
        }
        if let Err(err) = check_kvm() {
            exit_vm_error(err, mode);
        }
        if parsed == Platform::Windows {
            if let Err(err) = check_swtpm() {
                exit_vm_error(err, mode);
            }
        }
        emit_start_plan(&opts, mode);
        return;
    }

    match VmLifecycle::start(&opts, &paths).await {
        Ok(result) => {
            if result.agent_unreachable {
                eprintln!(
                    "warning: agent did not reach health within the boot window — \
                     agent commands will fail until it comes up"
                );
            }
            match mode {
                OutputMode::Text => println!("{}", result.id),
                OutputMode::Json => {
                    let agent = result
                        .spec
                        .agent
                        .as_ref()
                        .map(|a| json!({ "host": a.host, "port": a.port }))
                        .unwrap_or(Value::Null);
                    print_success(json!({
                        "id": result.id,
                        "platform": result.platform.as_str(),
                        "base": opts.base,
                        "vnc": { "host": result.spec.vnc.host, "port": result.spec.vnc.port },
                        "agent": agent,
                        "spec_path": result.spec_path.display().to_string(),
                        "meta_path": result.meta_path.display().to_string(),
                    }));
                }
            }
        }
        Err(err) => exit_vm_error(err, mode),
    }
}

fn emit_start_plan(opts: &VmStartOptions, mode: OutputMode) {
    match mode {
        OutputMode::Text => {
            println!("dry-run: would start {} (platform {}, base {})",
                opts.id, opts.platform.as_str(), opts.base);
        }
        OutputMode::Json => {
            print_success(json!({
                "dry_run": true,
                "id": opts.id,
                "platform": opts.platform.as_str(),
                "base": opts.base,
                "vnc": { "host": "localhost", "port": 0 },
            }));
        }
    }
}

/// `testanyware vm stop`.
pub async fn run_vm_stop(id: Option<String>, mode: OutputMode, dry_run: bool) {
    let Some(id) = id.filter(|s| !s.is_empty()) else {
        print_error(
            mode,
            "USAGE_ERROR",
            "VM id required: pass it as an argument or set TESTANYWARE_VM_ID",
            Some("Run `testanyware vm list` to see running VM ids."),
            json!({}),
            2,
        );
    };
    let paths = VmPaths::from_process_env();
    let meta_path = paths.meta_path(&id);
    if !meta_path.is_file() {
        exit_vm_error(VmError::VmNotFound { id }, mode);
    }

    if dry_run {
        let pid = VmMeta::load(&meta_path).ok().map(|m| m.pid);
        match mode {
            OutputMode::Text => println!("dry-run: would stop {id} (pid {pid:?})"),
            OutputMode::Json => {
                print_success(json!({ "dry_run": true, "id": id, "stopped": false }));
            }
        }
        return;
    }

    match VmLifecycle::stop(&id, &paths) {
        Ok(()) => match mode {
            OutputMode::Text => println!("stopped {id}"),
            OutputMode::Json => print_success(json!({ "id": id, "stopped": true })),
        },
        Err(err) => exit_vm_error(err, mode),
    }
}

/// `testanyware vm list`.
pub async fn run_vm_list(
    mode: OutputMode,
    limit: usize,
    all: bool,
    filter: Option<String>,
) {
    let filters = match filter.as_deref().map(parse_filter) {
        Some(Ok(f)) => f,
        Some(Err(msg)) => print_error(
            mode,
            "USAGE_ERROR",
            &format!("invalid --filter: {msg}"),
            Some("Expected comma-separated field=value pairs, e.g. --filter platform=windows."),
            json!({ "value": filter.unwrap_or_default() }),
            2,
        ),
        None => Vec::new(),
    };
    let paths = VmPaths::from_process_env();
    let listing = VmLifecycle::list(&paths);
    let items: Vec<ListItem> = listing_items(&listing)
        .into_iter()
        .filter(|item| filter_matches(item, &filters))
        .collect();
    let (shown, returned, total, truncated) = apply_limit(items, limit, all);

    match mode {
        OutputMode::Text => render_list_text(&shown, returned, total, truncated),
        OutputMode::Json => {
            let json_items: Vec<Value> = shown.iter().map(item_to_json).collect();
            print_success(json!({
                "items": json_items,
                "returned": returned,
                "total": total,
                "truncated": truncated,
            }));
        }
    }
}

/// `testanyware vm delete`.
pub async fn run_vm_delete(name: String, force: bool, mode: OutputMode, dry_run: bool) {
    let paths = VmPaths::from_process_env();

    if dry_run {
        let qcow2 = paths.golden_dir().join(format!("{name}.qcow2"));
        if !qcow2.is_file() {
            exit_vm_error(VmError::GoldenNotFound { name }, mode);
        }
        match mode {
            OutputMode::Text => println!("dry-run: would delete golden {name}"),
            OutputMode::Json => {
                print_success(json!({ "dry_run": true, "name": name, "deleted": false }));
            }
        }
        return;
    }

    match VmLifecycle::delete(&name, force, &paths) {
        Ok(()) => match mode {
            OutputMode::Text => println!("deleted {name}"),
            OutputMode::Json => print_success(json!({ "name": name, "deleted": true })),
        },
        Err(err) => exit_vm_error(err, mode),
    }
}

// ---- pure helpers (unit-tested) -----------------------------------------

/// Flatten a `VmListing` into the unified row form.
pub fn listing_items(listing: &VmListing) -> Vec<ListItem> {
    let mut out = Vec::new();
    for g in &listing.goldens {
        out.push(ListItem {
            kind: "golden",
            name: g.name.clone(),
            platform: g.platform.clone(),
            backend: g.backend,
            pid: None,
            vnc: None,
            agent: None,
        });
    }
    for r in &listing.running {
        out.push(ListItem {
            kind: "running",
            name: r.id.clone(),
            platform: r.platform.clone(),
            backend: r.backend,
            pid: r.pid,
            vnc: r.vnc.clone(),
            agent: r.agent.clone(),
        });
    }
    out
}

/// Parse `--filter` into `(field, value)` pairs. Errors on a pair lacking `=`.
pub fn parse_filter(raw: &str) -> Result<Vec<(String, String)>, String> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for pair in raw.split(',') {
        let (k, v) = pair
            .split_once('=')
            .ok_or_else(|| format!("'{pair}' is not a field=value pair"))?;
        out.push((k.trim().to_string(), v.trim().to_string()));
    }
    Ok(out)
}

/// True if `item` satisfies every filter. Unknown fields never match.
pub fn filter_matches(item: &ListItem, filters: &[(String, String)]) -> bool {
    filters.iter().all(|(field, value)| match field.as_str() {
        "kind" => item.kind == value,
        "platform" => item.platform == *value,
        "backend" => item.backend == value,
        "name" => item.name == *value,
        _ => false,
    })
}

/// Apply the §9.4 list limit. Returns `(shown, returned, total, truncated)`.
pub fn apply_limit<T>(items: Vec<T>, limit: usize, all: bool) -> (Vec<T>, usize, usize, bool) {
    let total = items.len();
    if all || total <= limit {
        return (items, total, total, false);
    }
    let shown: Vec<T> = items.into_iter().take(limit).collect();
    let returned = shown.len();
    (shown, returned, total, true)
}

fn item_to_json(item: &ListItem) -> Value {
    json!({
        "kind": item.kind,
        "name": item.name,
        "platform": item.platform,
        "backend": item.backend,
        "pid": item.pid,
        "vnc": item.vnc,
        "agent": item.agent,
    })
}

fn render_list_text(items: &[ListItem], returned: usize, total: usize, truncated: bool) {
    let goldens: Vec<&ListItem> = items.iter().filter(|i| i.kind == "golden").collect();
    let running: Vec<&ListItem> = items.iter().filter(|i| i.kind == "running").collect();
    println!("Golden images:");
    if goldens.is_empty() {
        println!("  (none)");
    } else {
        for g in goldens {
            println!("  {:<8} {:<40} {}", g.platform, g.name, g.backend);
        }
    }
    println!();
    println!("Running clones:");
    if running.is_empty() {
        println!("  (none)");
    } else {
        for r in running {
            println!(
                "  {:<24} {:<8} vnc={} agent={} pid={}",
                r.name,
                r.platform,
                r.vnc.as_deref().unwrap_or("-"),
                r.agent.as_deref().unwrap_or("-"),
                r.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
            );
        }
    }
    if truncated {
        println!("\nShowing {returned} of {total}. Use --limit N or --all to see more.");
    }
}

fn exit_vm_error(err: VmError, mode: OutputMode) -> ! {
    let code = err.code();
    let exit = err.exit_code();
    let remediation = err.remediation();
    print_error(mode, code, &err.to_string(), remediation.as_deref(), err.details(), exit);
}
```

> Note: `testanyware_vm::lifecycle::*` and `testanyware_vm::preflight::*` are referenced via the module path. Confirm `lib.rs` exposes `pub mod lifecycle;` and `pub mod preflight;` (it does — Tasks 9 and 12). `VmListing` is also re-exported at crate root, so `testanyware_vm::VmListing` works too; the plan uses the module path for clarity.

- [ ] **Step 6: Run the tests**

Run: `cd cli-rs && cargo test -p testanyware-cli commands::vm::`
Expected: PASS (5 tests).

- [ ] **Step 7: Commit**

```bash
git add cli-rs/crates/testanyware-cli/Cargo.toml cli-rs/crates/testanyware-cli/src/commands/mod.rs cli-rs/crates/testanyware-cli/src/commands/vm.rs
git commit -m "feat(cli): vm command handlers (start/stop/list/delete)"
```

---

## Task 16: Wire the `vm` commands into `main.rs`

**Files:**
- Modify: `cli-rs/crates/testanyware-cli/src/main.rs`

Adds the new flags (`--base`, `--id`, `--json`, `--dry-run`, `--limit`, `--all`, `--filter`), the §7 after-help blocks, and the dispatch. `vm start --platform` becomes **required** (the Swift `macos` default always routed to tart).

- [ ] **Step 1: Add the `vm` handler import**

In the `use testanyware_cli::commands::{...}` block, add `vm as vm_cmds`:

```rust
use testanyware_cli::commands::{
    agent as agent_cmds, file as file_cmds, input as input_cmds, screen as screen_cmds,
    vm as vm_cmds,
};
```

- [ ] **Step 2: Add the four §7 after-help blocks**

After the `FILE_DOWNLOAD_AFTER_HELP` const (before `ROOT_BEFORE_HELP`), add:

```rust
const VM_START_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (the VM id on one line), --json (schema: vm-start).

EXIT CODES:
    0  success
    1  QEMU_FAILED, SWTPM_MISSING, VM_BACKEND_UNSUPPORTED, SPAWN_FAILED
    2  USAGE_ERROR / INVALID_PLATFORM
    3  GOLDEN_NOT_FOUND / UEFI_NOT_FOUND
    4  KVM_PERMISSION_DENIED

IDEMPOTENCY:
    Not idempotent — each call without --id clones a fresh VM. Retry-safe:
    a failed start tears its own partial clone down.

EXAMPLES:
    # Start a Windows guest at 1080p
    testanyware vm start --platform windows --display 1920x1080

    # Start a Linux guest, capture the id as JSON
    testanyware vm start --platform linux --json

    # Plan a start without performing it
    testanyware vm start --platform windows --dry-run

SEE ALSO:
    testanyware vm stop, testanyware vm list, testanyware doctor
";

const VM_STOP_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`stopped <id>`), --json (schema: vm-stop).

EXIT CODES:
    0  success
    1  VM_STOP_FAILED, VM_BACKEND_UNSUPPORTED
    2  USAGE_ERROR (no id and TESTANYWARE_VM_ID unset)
    3  VM_NOT_FOUND

IDEMPOTENCY:
    Idempotent — stopping an already-stopped VM is a VM_NOT_FOUND, not a
    crash. Retry-safe.

EXAMPLES:
    # Stop a VM by id
    testanyware vm stop testanyware-deadbeef

    # Stop the VM named by $TESTANYWARE_VM_ID
    testanyware vm stop

    # Plan the stop as JSON
    testanyware vm stop testanyware-deadbeef --dry-run --json

SEE ALSO:
    testanyware vm start, testanyware vm list
";

const VM_LIST_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: --json (schema: vm-list; envelope carries
    returned/total/truncated per contract §3.5). Text is a two-section
    summary and is not a parsing target.

EXIT CODES:
    0  success

EXAMPLES:
    # List golden images and running clones
    testanyware vm list

    # JSON for scripting, unbounded
    testanyware vm list --json --all

    # Only Windows entries
    testanyware vm list --filter platform=windows

SEE ALSO:
    testanyware vm start, testanyware vm delete
";

const VM_DELETE_AFTER_HELP: &str = "\
OUTPUT:
    Stable formats: text (`deleted <name>`), --json (schema: vm-delete).

EXIT CODES:
    0  success
    1  generic failure
    3  GOLDEN_NOT_FOUND
    5  GOLDEN_IN_USE (running clones depend on the image; use --force)

IDEMPOTENCY:
    Idempotent in the deleted state. --force overrides the running-clones
    safety check.

EXAMPLES:
    # Delete a golden image
    testanyware vm delete testanyware-golden-linux-24.04

    # Delete even though clones depend on it
    testanyware vm delete testanyware-golden-windows-11 --force

    # Plan the delete as JSON
    testanyware vm delete testanyware-golden-linux-24.04 --dry-run --json

SEE ALSO:
    testanyware vm list, testanyware vm start
";
```

- [ ] **Step 3: Replace the `VmAction` enum**

Replace the entire `enum VmAction { ... }` block with:

```rust
#[derive(Subcommand, Debug)]
enum VmAction {
    /// Start a clone of a golden image
    #[command(after_long_help = VM_START_AFTER_HELP)]
    Start {
        /// Target platform: linux or windows. (macos uses the tart
        /// backend, which is not yet ported to the Rust CLI.)
        #[arg(long, value_name = "PLATFORM")]
        platform: String,
        /// Golden image to clone [default: the platform's golden].
        #[arg(long, value_name = "NAME")]
        base: Option<String>,
        /// VM instance id [default: testanyware-<hex8>].
        #[arg(long, value_name = "ID")]
        id: Option<String>,
        /// Display resolution, e.g. 1920x1080.
        #[arg(long, value_name = "WxH")]
        display: Option<String>,
        /// Open a VNC viewer after boot (not yet ported — backlog task 8).
        #[arg(long)]
        viewer: bool,
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Plan the start but do not perform it.
        #[arg(long)]
        dry_run: bool,
    },
    /// Stop a running VM by id
    #[command(after_long_help = VM_STOP_AFTER_HELP)]
    Stop {
        /// VM instance id (falls back to TESTANYWARE_VM_ID).
        #[arg(value_name = "ID", env = "TESTANYWARE_VM_ID")]
        id: Option<String>,
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Plan the stop but do not perform it.
        #[arg(long)]
        dry_run: bool,
    },
    /// List running clones and golden images
    #[command(aliases = ["ls"], after_long_help = VM_LIST_AFTER_HELP)]
    List {
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Maximum rows to show.
        #[arg(long, value_name = "N", default_value_t = 100)]
        limit: usize,
        /// Show all rows (overrides --limit).
        #[arg(long, conflicts_with = "limit")]
        all: bool,
        /// Comma-separated field=value filters (fields: kind, platform,
        /// backend, name).
        #[arg(long, value_name = "EXPR")]
        filter: Option<String>,
    },
    /// Delete a golden image by name
    #[command(aliases = ["rm", "remove"], after_long_help = VM_DELETE_AFTER_HELP)]
    Delete {
        /// Golden image name (run `testanyware vm list` to see images).
        name: String,
        /// Delete even if running clones appear to depend on the image.
        #[arg(long)]
        force: bool,
        /// Emit JSON envelope on stdout.
        #[arg(long)]
        json: bool,
        /// Plan the delete but do not perform it.
        #[arg(long)]
        dry_run: bool,
    },
}
```

- [ ] **Step 4: Replace the `Command::Vm` dispatch arm**

Replace the `Command::Vm { action } => match action { ... }` block (currently four `unimplemented(...)` calls) with:

```rust
        Command::Vm { action } => match action {
            VmAction::Start { platform, base, id, display, viewer, json, dry_run } => {
                vm_cmds::run_vm_start(
                    platform, base, id, display, viewer,
                    OutputMode::from_flags(json), dry_run,
                )
                .await
            }
            VmAction::Stop { id, json, dry_run } => {
                vm_cmds::run_vm_stop(id, OutputMode::from_flags(json), dry_run).await
            }
            VmAction::List { json, limit, all, filter } => {
                vm_cmds::run_vm_list(OutputMode::from_flags(json), limit, all, filter).await
            }
            VmAction::Delete { name, force, json, dry_run } => {
                vm_cmds::run_vm_delete(name, force, OutputMode::from_flags(json), dry_run).await
            }
        },
```

- [ ] **Step 5: Build and smoke the help text**

Run:
```bash
cd cli-rs && cargo build -p testanyware-cli
./target/debug/testanyware vm start --help
./target/debug/testanyware vm stop --help
./target/debug/testanyware vm list --help
./target/debug/testanyware vm delete --help
```
Expected: compiles; each help body shows `EXIT CODES:`, `EXAMPLES:`, `SEE ALSO:` and ≥ 2 `testanyware vm ...` example lines.

- [ ] **Step 6: Smoke the data paths without a VM**

Run:
```bash
./target/debug/testanyware vm list --json
./target/debug/testanyware vm stop testanyware-nonexistent --json; echo "exit=$?"
./target/debug/testanyware vm start --platform bsd --json; echo "exit=$?"
```
Expected:
- `vm list --json` → a §3 envelope `{schema_version, ok:true, items:[], returned:0, total:0, truncated:false}` (no VMs/goldens in the default env), exit 0.
- `vm stop ...nonexistent` → `{ok:false, code:"VM_NOT_FOUND", ...}`, `exit=3`.
- `vm start --platform bsd` → `{ok:false, code:"INVALID_PLATFORM", ...}`, `exit=2`.

- [ ] **Step 7: Commit**

```bash
git add cli-rs/crates/testanyware-cli/src/main.rs
git commit -m "feat(cli): wire vm start/stop/list/delete with §7 help"
```

---

## Task 17: cli-contract assertions for the vm commands

**Files:**
- Modify: `cli-rs/crates/testanyware-cli/tests/cli-contract.rs`

`cli-contract.rs` keeps the cross-command checks (`each_subcommand_help_follows_template`, etc.) `#[ignore]`d until every command is ported (per its own header). This task adds **vm-scoped** assertion tests — the §11 "per-command port task fills in its slice" pattern — covering §7 help, §3 JSON, §4/§5 error codes, and §9.3 dry-run for the four vm commands. These run without a live VM by pointing `XDG_*` at temp dirs.

- [ ] **Step 1: Write the vm-scoped contract tests** — append to `cli-contract.rs`:

```rust
// ---------------------------------------------------------------------------
// vm commands — port-task slice (port-qemu-runner-and-vm-lifecycle-to-rust)
// ---------------------------------------------------------------------------

/// Run the binary with extra environment variables.
fn run_env(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(BIN);
    cmd.args(args);
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output()
        .unwrap_or_else(|e| panic!("failed to invoke `{BIN} {}`: {e}", args.join(" ")))
}

/// §7: each vm subcommand's `--help` carries the required sections and
/// at least two concrete example invocations.
#[test]
fn vm_commands_help_follows_template() {
    for sub in ["start", "stop", "list", "delete"] {
        let out = run(&["vm", sub, "--help"]);
        assert!(out.status.success(), "`vm {sub} --help` exited non-zero");
        let help = String::from_utf8_lossy(&out.stdout);
        for section in ["EXIT CODES:", "EXAMPLES:", "SEE ALSO:"] {
            assert!(
                help.contains(section),
                "`vm {sub} --help` missing {section:?}; got:\n{help}",
            );
        }
        let examples = help.matches("testanyware vm ").count();
        assert!(
            examples >= 2,
            "`vm {sub} --help` needs ≥2 example invocations, found {examples}",
        );
    }
}

/// §3.1 + §3.5: `vm list --json` emits the truncation envelope.
#[test]
fn vm_list_json_emits_truncation_envelope() {
    let dir = tempfile::tempdir().unwrap();
    let out = run_env(
        &["vm", "list", "--json"],
        &[
            ("XDG_STATE_HOME", dir.path().to_str().unwrap()),
            ("XDG_DATA_HOME", dir.path().to_str().unwrap()),
        ],
    );
    assert!(out.status.success(), "`vm list --json` exited non-zero");
    let body: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("vm list --json must parse");
    assert_eq!(body["ok"], true);
    assert!(body["schema_version"].is_string());
    for key in ["items", "returned", "total", "truncated"] {
        assert!(body.get(key).is_some(), "vm-list envelope missing {key}; got: {body:#?}");
    }
    assert!(body["items"].is_array());
}

/// §4 + §5: vm error paths carry a stable code and the correct exit code.
#[test]
fn vm_commands_carry_stable_error_codes() {
    // vm stop on a missing VM → VM_NOT_FOUND, exit 3.
    let dir = tempfile::tempdir().unwrap();
    let out = run_env(
        &["vm", "stop", "testanyware-deadbeef", "--json"],
        &[("XDG_STATE_HOME", dir.path().to_str().unwrap())],
    );
    assert_eq!(out.status.code(), Some(3), "vm stop miss must exit 3");
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["code"], "VM_NOT_FOUND");

    // vm start with a bad platform → INVALID_PLATFORM, exit 2.
    let out = run(&["vm", "start", "--platform", "bsd", "--json"]);
    assert_eq!(out.status.code(), Some(2), "bad platform must exit 2");
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["code"], "INVALID_PLATFORM");

    // vm delete of an absent golden → GOLDEN_NOT_FOUND, exit 3.
    let dir2 = tempfile::tempdir().unwrap();
    let out = run_env(
        &["vm", "delete", "testanyware-golden-ghost", "--json"],
        &[("XDG_DATA_HOME", dir2.path().to_str().unwrap())],
    );
    assert_eq!(out.status.code(), Some(3), "vm delete miss must exit 3");
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["code"], "GOLDEN_NOT_FOUND");
}

/// §9.3: vm stop / vm delete accept `--dry-run`, exit 0, and set
/// `dry_run: true` without performing the mutation.
#[test]
fn vm_mutating_commands_support_dry_run() {
    // vm stop --dry-run against a synthetic meta sidecar.
    let dir = tempfile::tempdir().unwrap();
    let vms = dir.path().join("testanyware").join("vms");
    std::fs::create_dir_all(&vms).unwrap();
    let id = "testanyware-abcd1234";
    std::fs::write(
        vms.join(format!("{id}.meta.json")),
        serde_json::to_vec(&serde_json::json!({
            "id": id, "tool": "qemu", "pid": 999999,
            "clone_dir": dir.path().join("clone").display().to_string()
        }))
        .unwrap(),
    )
    .unwrap();
    let out = run_env(
        &["vm", "stop", id, "--dry-run", "--json"],
        &[("XDG_STATE_HOME", dir.path().to_str().unwrap())],
    );
    assert_eq!(out.status.code(), Some(0), "vm stop --dry-run must exit 0");
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["dry_run"], true);
    // The meta sidecar must still exist — dry-run performed no mutation.
    assert!(vms.join(format!("{id}.meta.json")).is_file(), "dry-run must not delete the sidecar");

    // vm delete --dry-run against a synthetic golden qcow2.
    let dir2 = tempfile::tempdir().unwrap();
    let golden = dir2.path().join("testanyware").join("golden");
    std::fs::create_dir_all(&golden).unwrap();
    let name = "testanyware-golden-linux-24.04";
    std::fs::write(golden.join(format!("{name}.qcow2")), b"disk").unwrap();
    let out = run_env(
        &["vm", "delete", name, "--dry-run", "--json"],
        &[("XDG_DATA_HOME", dir2.path().to_str().unwrap())],
    );
    assert_eq!(out.status.code(), Some(0), "vm delete --dry-run must exit 0");
    let body: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert_eq!(body["dry_run"], true);
    assert!(golden.join(format!("{name}.qcow2")).is_file(), "dry-run must not delete the qcow2");
}
```

> The `XDG_STATE_HOME` env value is the temp dir root; `VmPaths::from_env` appends `testanyware/vms`, so the test writes the sidecar under `<tmp>/testanyware/vms/` to match. `vm start --dry-run` is intentionally **not** asserted here — it depends on host KVM/swtpm preflight and is covered behaviourally by the live smoke (Task 19).

- [ ] **Step 2: Run the new tests**

Run: `cd cli-rs && cargo test -p testanyware-cli --test cli-contract vm_`
Expected: PASS (4 tests).

- [ ] **Step 3: Run the whole contract suite to confirm nothing regressed**

Run: `cd cli-rs && cargo test -p testanyware-cli --test cli-contract`
Expected: PASS — the pre-existing tests (`every_canonical_command_is_present`, `schema_command_emits_json_schema_for_each_command`, `capabilities_lists_full_surface`, etc.) still pass; the four new `vm_*` tests pass; the cross-command `#[ignore]`d skeletons stay ignored.

- [ ] **Step 4: Commit**

```bash
git add cli-rs/crates/testanyware-cli/tests/cli-contract.rs
git commit -m "test(cli): contract assertions for vm start/stop/list/delete"
```

---

## Task 18: Full workspace verification

**Files:** none (verification only).

- [ ] **Step 1: Format and lint the whole workspace**

Run:
```bash
cd cli-rs
cargo fmt --all
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: `cargo fmt` leaves a clean tree (or commit its changes); clippy reports zero warnings. Fix anything it flags.

- [ ] **Step 2: Run the full workspace test suite**

Run: `cd cli-rs && cargo test --workspace`
Expected: PASS — all pre-existing tests plus the new `testanyware-vm` suite (≈ 40 tests) and the four new contract tests. Zero failures.

- [ ] **Step 3: Smoke the command surface**

Run:
```bash
cd cli-rs && cargo build
./target/debug/testanyware vm --help
./target/debug/testanyware vm ls --json          # synonym alias
./target/debug/testanyware schema vm start        # real schema, not the stub
./target/debug/testanyware schema vm list
./target/debug/testanyware capabilities --json | python3 -c 'import sys,json; c=json.load(sys.stdin); assert "KVM_PERMISSION_DENIED" in c["error_codes"] and "SWTPM_MISSING" in c["error_codes"]; print("error codes OK")'
```
Expected: `vm --help` lists `start/stop/list/delete`; `vm ls` works as a `list` alias; `schema vm start` emits the real (non-`$comment: TODO`) schema; the `capabilities` assertion prints `error codes OK`.

- [ ] **Step 4: Commit any fmt/clippy fixups**

```bash
git add -A
git commit -m "chore(vm): cargo fmt + clippy fixups across the vm port"
```

(Skip the commit if Steps 1–3 produced no changes.)

---

## Task 19: Live Windows-guest smoke

**Files:** none (live verification only).

This is the acceptance gate. A `testanyware-golden-windows-11` golden is present on this macOS host (`~/.local/share/testanyware/golden/`), and `qemu-system-aarch64` / `swtpm` / `qemu-img` are installed — so the QEMU-on-macOS path can be verified end to end. Linux-guest and macOS-guest criteria stay deferred (no Linux golden here; tart is backlog task 12).

> Windows boots slowly; the agent health waiter budget is 120 × 5 s ≈ 10 min. Allow the smoke up to ~15 min.

- [ ] **Step 1: Build the release binary**

Run: `cd cli-rs && cargo build --release`
Expected: clean build. Use `./target/release/testanyware` below.

- [ ] **Step 2: Start a Windows VM**

Run:
```bash
cd cli-rs
./target/release/testanyware vm start --platform windows --display 1920x1080 --json | tee /tmp/vm-start.json
VM_ID=$(python3 -c 'import json; print(json.load(open("/tmp/vm-start.json"))["id"])')
echo "VM_ID=$VM_ID"
```
Expected: exits 0; JSON envelope with `ok:true`, an `id` matching `testanyware-[0-9a-f]{8}`, a `vnc` object, and a non-null `agent` object (the acceptance criterion: "agent reaches health within timeout"). If `agent` is `null`, the VM started but the agent did not come up — capture `~/.local/share/testanyware/clones/$VM_ID/qemu.log` and treat it as a smoke failure to investigate before declaring the task done.

- [ ] **Step 3: Confirm the sockets staged under $TMPDIR**

Run: `ls -la "${TMPDIR:-/tmp}/testanyware-$VM_ID/"`
Expected: `monitor.sock` and `swtpm-sock` both present (validates the `sun_path` staging from decision log 2026-04-20).

- [ ] **Step 4: Confirm `vm list` sees the running VM**

Run:
```bash
./target/release/testanyware vm list --json | python3 -c '
import sys, json, os
items = json.load(sys.stdin)["items"]
vm_id = os.environ["VM_ID"]
running = [i for i in items if i["kind"] == "running" and i["name"] == vm_id]
assert running, f"{vm_id} not in vm list running items"
print("vm list sees", vm_id, running[0])
'
./target/release/testanyware vm list   # human-readable two-section view
```
Expected: the assertion prints the running entry; the text view shows it under "Running clones".

- [ ] **Step 5: Confirm the agent is reachable**

Run: `./target/release/testanyware agent health --vm "$VM_ID"`
Expected: `OK` (exit 0) — the in-VM agent answers via the spec's resolved agent endpoint. This is the cross-command proof that `vm start` wrote a usable spec.

- [ ] **Step 6: Stop the VM**

Run:
```bash
./target/release/testanyware vm stop "$VM_ID" --json
```
Expected: exit 0, `{ok:true, id:<VM_ID>, stopped:true}`.

- [ ] **Step 7: Confirm clean teardown — no orphans**

Run:
```bash
./target/release/testanyware vm list --json | python3 -c 'import sys,json; assert not [i for i in json.load(sys.stdin)["items"] if i["kind"]=="running"], "running clones still listed"; print("no running clones")'
ls -d "${TMPDIR:-/tmp}/testanyware-$VM_ID" 2>/dev/null && echo "LEAK: session dir survived" || echo "session dir cleaned"
ls -d ~/.local/share/testanyware/clones/"$VM_ID" 2>/dev/null && echo "LEAK: clone dir survived" || echo "clone dir cleaned"
pgrep -fl "testanyware-$VM_ID" && echo "LEAK: qemu/swtpm orphan" || echo "no qemu/swtpm orphans"
ls ~/.local/state/testanyware/vms/"$VM_ID".json 2>/dev/null && echo "LEAK: spec survived" || echo "spec removed"
```
Expected: `no running clones`, `session dir cleaned`, `clone dir cleaned`, `no qemu/swtpm orphans`, `spec removed`.

- [ ] **Step 8: Record the smoke result**

Append a short outcome note to the task's eventual `results` block in `LLM_STATE/core/backlog.yaml` (done during finish-up, not here): which criteria passed live (Windows-on-macOS-host start/list/health/stop, clean teardown) and which stay deferred (Linux guest, macOS guest — no goldens; tart — task 12).

If any step fails, **do not** mark the backlog task done — use `superpowers:systematic-debugging`, fix forward, and re-run the smoke from Step 2.

---

## Self-Review (performed while writing this plan)

**Spec coverage** — every backlog requirement maps to a task:

| Backlog requirement | Task(s) |
|---|---|
| Port `QEMURunner.swift` | 8, 10 |
| Port `QEMUMonitorClient.swift` | 5 |
| Port `VMLifecycle.swift` (QEMU paths) | 12 |
| Port `AgentHealthWaiter.swift` | 11 |
| Port `DetachedProcess.swift` | 7 |
| Port `VMPaths.swift` | 3 |
| Port `VMSpec.swift` / `VMMeta.swift` | 4 |
| swtpm setup for Windows guests | 9, 10 |
| Per-VM spec + meta sidecar with PID + clone-dir | 4, 12 |
| `vm start --platform --display [--viewer]` | 15, 16 |
| `vm stop <id>` | 15, 16 |
| `vm list` | 15, 16 |
| `vm delete <name>` | 15, 16 |
| `tokio::process` + `setsid` via `nix` `#[cfg(unix)]` | 7 |
| Re-validated process-tree-kill (not verbatim) | 6 |
| `KVM_PERMISSION_DENIED` code + remediation | 2, 9, 13 |
| `SWTPM_MISSING` code + remediation | 2, 9, 13 |
| CLI design contract: help examples, `--json`, error codes, `--dry-run` | 14, 15, 16, 17 |
| Meta-sidecar cross-tooling compatibility | 4 (Swift-shaped JSON tests), 12 |
| Acceptance: live Windows start/list/health/stop | 19 |

**Deferred (with reason):** macOS-host-macOS-guest and Linux-guest live criteria — no Linux golden on this host and tart is backlog task 12; the Linux `QemuProfile` branch is implemented and unit-covered but its live verification waits for a goldens session. The `--viewer` flag is accepted but inert (backlog task 8).

**Placeholder scan:** no `TODO` / "implement later" / "add error handling" — every step carries complete code or an exact command. The one forward reference (`VmListItem` in Task 12) is explicitly resolved in that task's note (omit it; build from `VmListing` directly in Task 15, which is what Task 15 does).

**Type consistency:** `VmError`, `VmPaths`, `VmSpec`/`VncEndpoint`/`AgentEndpoint`, `VmMeta`/`VmTool`, `QemuMonitorClient`, `QemuProfile`, `QemuRunner`/`QemuStartOptions`/`QemuLaunchSpec`/`StartArtifacts`/`GoldenImage`/`RunningClone`, `Platform`/`VmLifecycle`/`VmStartOptions`/`VmStartResult`/`VmListing`/`RunningEntry`, and the CLI `ListItem` are defined once and referenced with matching signatures throughout. `wait_for_agent`, `spawn_detached`, `parse_agent_port`/`parse_vnc_port`, `process_alive`/`terminate`/`pgrep_first`, `check_kvm`/`check_swtpm`, `host_profile`/`resolve_uefi_code`/`which`, `build_qemu_args`/`teardown`, `parse_filter`/`filter_matches`/`apply_limit`/`listing_items` keep one signature each across the tasks that define and call them.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-22-port-qemu-runner-and-vm-lifecycle-to-rust.md`. Two execution options:

1. **Subagent-Driven (recommended)** — a fresh subagent per task, two-stage review between tasks, fast iteration. Tasks 1–17 are well-suited; Task 19 (live smoke) runs in the main session.
2. **Inline Execution** — execute tasks in this session via `superpowers:executing-plans`, batched with review checkpoints.

Tasks 1–17 are ordered by dependency and each ends green and committed. Task 18 is whole-workspace verification. Task 19 is the live acceptance gate and needs the on-host Windows golden.

