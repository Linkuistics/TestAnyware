//! Golden-image creation: **boot-1 normal-mode provisioning** (macOS host).
//!
//! Ports **boot 1** of `provisioner/scripts/vm-create-golden-macos.sh`
//! (grove leaf `110-vm-create-golden-macos/020`): clone the vanilla Cirrus
//! Labs image into a throwaway *setup VM*, boot it, and provision it over
//! the `010` [`SshSession`](crate::ssh::SshSession) layer (ADR-0007) — host
//! pubkey install, macOS defaults, solid wallpaper, hide widgets, Xcode CLT,
//! Homebrew, and the agent binary + LaunchAgent plist.
//!
//! The handoff to leaf `030` (the SIP/TCC recovery cycle + finalize) is a
//! [`SetupVm`]: **provisioned and still running, pid + IP known**. This
//! module does *not* do the recovery cycle, TCC grants, health gate, or the
//! final `tart clone` to golden — those are `030`.
//!
//! **macOS-host only** (`tart` wraps Virtualization.framework — ADR-0003
//! per-target gating), so the module is `#[cfg(target_os = "macos")]`-gated
//! at the crate root, like `tart.rs`.
//!
//! ## What is in-process vs the bash script
//!
//! Two things shrink relative to the script:
//! - **No host-CLI-binary resolution.** The script resolves `_TESTANYWARE_BIN`
//!   only because its recovery automation shells out to `testanyware
//!   find-text`/`input` over VNC. Leaf `030` runs recovery *in-process*
//!   (RFB + OCR), so boot-1 resolves only the **agent** binary.
//! - **No `provisioner/helpers/` path resolution.** The wallpaper helper and
//!   the LaunchAgent plist are [`include_str!`]-embedded, so the command is
//!   self-contained — which matters because `030` deletes the shell script.
//!
//! ## Progress narration
//!
//! Provisioning is a multi-minute interactive flow whose value (like the
//! script's) is its running narration; this module emits progress to
//! **stderr** via `eprintln!`, keeping `--json` stdout clean. The pure
//! decision/parse helpers are unit-tested; the live `tart`/SSH orchestration
//! is verified by actually creating a golden on the Mac (cheap —
//! [[vm-costs]]).

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::VmError;
use crate::id::generate_id;
use crate::paths::VmPaths;
use crate::ssh::SshSession;

/// The vanilla image's first-contact credentials (Cirrus Labs `admin/admin`,
/// password-only until the host pubkey is installed). Ports `_VANILLA_USER`
/// / `_VANILLA_PASS`.
const VANILLA_USER: &str = "admin";
const VANILLA_PASS: &str = "admin";

/// The macOS agent LaunchAgent plist, embedded so the command does not
/// depend on a `provisioner/` tree beside the binary (`030` deletes it).
const AGENT_PLIST: &str =
    include_str!("../../../../provisioner/helpers/com.linkuistics.testanyware.agent.plist");

/// The solid-wallpaper helper source (`NSWorkspace.setDesktopImageURL`),
/// compiled on the host with `swiftc` and uploaded to the guest. Embedded
/// for the same reason as the plist.
const SET_WALLPAPER_SWIFT: &str =
    include_str!("../../../../provisioner/helpers/set-wallpaper.swift");

/// A 1×1 mid-gray (128,128,128) PNG, base64-encoded — scaled to the display
/// size on the guest with `sips`. Ports the literal in the script's wallpaper
/// step (line ~209).
const SOLID_GRAY_PNG_BASE64: &str =
    "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAIAAACQd1PeAAAADElEQVR4nGNoaGgAAAMEAYFL09IQAAAAAElFTkSuQmCC";

/// Inputs for golden creation, resolved by the command layer.
#[derive(Debug, Clone)]
pub struct GoldenOptions {
    /// macOS version selector — `tahoe`, `sequoia`, `sonoma` (the vanilla
    /// image tag). Ports `--version`.
    pub version: String,
    /// Final golden image name. Ports `--name`
    /// (default `testanyware-golden-macos-<version>`).
    pub name: String,
}

/// The handoff state boot-1 leaves for leaf `030`: a provisioned setup VM,
/// still running, with its detached `tart run` pid and guest IP known.
#[derive(Debug, Clone)]
pub struct SetupVm {
    /// The throwaway setup VM's tart name (`testanyware-setup-<hex8>`).
    pub id: String,
    /// Detached `tart run` pid.
    pub pid: i32,
    /// Guest IP, SSH-reachable.
    pub ip: String,
    /// Private key whose public half is installed in the guest's
    /// `authorized_keys` — `030` reconnects with it.
    pub key_path: PathBuf,
}

// ---- pure helpers (unit-tested) -----------------------------------------

/// The vanilla Cirrus Labs image ref for a macOS `version`. Ports
/// `_VANILLA="ghcr.io/cirruslabs/macos-$_VERSION-vanilla:latest"`.
pub fn vanilla_image(version: &str) -> String {
    format!("ghcr.io/cirruslabs/macos-{version}-vanilla:latest")
}

/// Ordered host SSH **public**-key candidates under `home`. Ports the
/// script's `~/.ssh/id_ed25519.pub` → `~/.ssh/id_rsa.pub` search (ed25519
/// preferred). The caller picks the first that exists on disk.
pub fn ssh_pubkey_candidates(home: &Path) -> Vec<PathBuf> {
    ["id_ed25519.pub", "id_rsa.pub"]
        .iter()
        .map(|name| home.join(".ssh").join(name))
        .collect()
}

/// The private-key path matching a `…/id_xxx.pub` public key —
/// `russh`'s `connect_key` signs with the private half. A path without the
/// `.pub` suffix is returned unchanged (defensive; callers pass a `.pub`).
pub fn private_key_for(pubkey: &Path) -> PathBuf {
    let s = pubkey.to_string_lossy();
    PathBuf::from(s.strip_suffix(".pub").unwrap_or(&s).to_string())
}

/// The brew-bundled macOS agent binary under a `brew --prefix testanyware`.
/// Ports `$_BREW_PREFIX/share/testanyware/agents/macos/testanyware-agent`.
pub fn agent_bin_under_prefix(brew_prefix: &Path) -> PathBuf {
    brew_prefix
        .join("share/testanyware/agents/macos/testanyware-agent")
}

/// The four `defaults write` commands that disable session restore so the
/// golden boots to a clean desktop. Ports script lines ~194–197.
pub fn session_restore_defaults() -> [&'static str; 4] {
    [
        "defaults write NSGlobalDomain NSQuitAlwaysKeepsWindows -bool false",
        "defaults write com.apple.loginwindow TALLogoutSavesState -bool false",
        "defaults write com.apple.loginwindow LoginwindowLaunchesRelaunchApps -bool false",
        "defaults write com.apple.Terminal NSQuitAlwaysKeepsWindows -bool false",
    ]
}

/// Guest shell command that materialises the scaled solid-gray wallpaper PNG
/// at `~/Pictures/solid_gray.png`. Ports the `base64 -d … | sips … | mv`
/// pipeline (script line ~209).
fn wallpaper_png_cmd() -> String {
    format!(
        "echo \"{SOLID_GRAY_PNG_BASE64}\" | base64 -d > /tmp/solid.png && \
         sips -z 1080 1920 /tmp/solid.png >/dev/null 2>&1 && \
         mkdir -p ~/Pictures && mv /tmp/solid.png ~/Pictures/solid_gray.png"
    )
}

/// Extract the Xcode Command Line Tools install label from `softwareupdate
/// -l` output. Ports the script's
/// `grep -B1 'Command Line Tools' | grep '*' | sed 's/^.*\* Label: //'`
/// pipeline (line ~226): the `* Label:` line sits immediately above the
/// `Title: … Command Line Tools …` line. Returns the label
/// (e.g. `Command Line Tools for Xcode-16.0`) or `None`.
pub fn parse_clt_label(softwareupdate_output: &str) -> Option<String> {
    let lines: Vec<&str> = softwareupdate_output.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        let label = line.split_once("Label:").map(|(_, rest)| rest.trim());
        let Some(label) = label.filter(|l| !l.is_empty()) else {
            continue;
        };
        // The `* Label:` line qualifies when it, or the following detail
        // line, names the Command Line Tools — matching the script's
        // `grep -B1 'Command Line Tools'` window.
        let names_clt = line.contains("Command Line Tools")
            || lines.get(i + 1).is_some_and(|n| n.contains("Command Line Tools"));
        if names_clt {
            return Some(label.to_string());
        }
    }
    None
}

// ---- live orchestration (verified on the Mac, not unit-tested) -----------
//
// These shell out to `tart` and drive the guest over SSH, so they are
// exercised by actually creating a golden on the Mac (clone+boot is cheap —
// `vm-costs`), not by unit tests — the same policy as `tart.rs`.

/// Resolve the host's SSH keypair: the first existing public-key candidate
/// and its matching private key. Hard error if neither pair is present — the
/// whole flow depends on key auth. Ports script lines ~70–81.
fn resolve_ssh_key() -> Result<(PathBuf, PathBuf), VmError> {
    let home = std::env::var("HOME").map_err(|_| {
        VmError::GoldenCreateFailed { detail: "HOME is not set; cannot locate ~/.ssh key".into() }
    })?;
    for pubkey in ssh_pubkey_candidates(Path::new(&home)) {
        let private = private_key_for(&pubkey);
        if pubkey.is_file() && private.is_file() {
            return Ok((pubkey, private));
        }
    }
    Err(VmError::GoldenCreateFailed {
        detail: "no SSH keypair found (~/.ssh/id_ed25519[.pub] or ~/.ssh/id_rsa[.pub]); \
                 generate one with `ssh-keygen -t ed25519`"
            .into(),
    })
}

/// Resolve the macOS agent binary to install. Honors
/// `TESTANYWARE_AGENT_BIN_OVERRIDE` (contributor builds); otherwise the
/// brew-bundled artifact under `brew --prefix testanyware`. Ports the agent
/// half of `install_agent` (script lines ~283–305) — the host-CLI half is
/// dropped (recovery is in-process; see the module doc).
fn resolve_agent_bin() -> Result<PathBuf, VmError> {
    if let Ok(over) = std::env::var("TESTANYWARE_AGENT_BIN_OVERRIDE") {
        if !over.is_empty() {
            let path = PathBuf::from(over);
            if !path.is_file() {
                return Err(VmError::GoldenCreateFailed {
                    detail: format!(
                        "TESTANYWARE_AGENT_BIN_OVERRIDE points at a missing file: {}",
                        path.display()
                    ),
                });
            }
            return Ok(path);
        }
    }
    let prefix = brew_prefix("testanyware").ok_or_else(|| VmError::GoldenCreateFailed {
        detail: "`brew --prefix testanyware` failed — install with \
                 `brew install Linkuistics/taps/testanyware`, or set \
                 TESTANYWARE_AGENT_BIN_OVERRIDE for a contributor build"
            .into(),
    })?;
    let bin = agent_bin_under_prefix(&prefix);
    if !bin.is_file() {
        return Err(VmError::GoldenCreateFailed {
            detail: format!("macOS agent binary not found at {}", bin.display()),
        });
    }
    Ok(bin)
}

/// `brew --prefix <formula>` stdout (trimmed), or `None` when brew is absent
/// or the formula is not installed.
fn brew_prefix(formula: &str) -> Option<PathBuf> {
    let brew = crate::qemu_profile::which("brew")?;
    let out = std::process::Command::new(brew).args(["--prefix", formula]).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!prefix.is_empty()).then(|| PathBuf::from(prefix))
}

/// Run a provisioning command, treating a non-zero exit as a **hard
/// failure** (`GOLDEN_CREATE_FAILED`). For steps whose failure must abort
/// boot-1 (pubkey install, agent install).
async fn exec_checked(session: &SshSession, command: &str, what: &str) -> Result<(), VmError> {
    let out = session.exec(command).await?;
    if out.exit_code != 0 {
        return Err(VmError::GoldenCreateFailed {
            detail: format!(
                "{what} failed (exit {}): {}",
                out.exit_code,
                out.stderr.trim()
            ),
        });
    }
    Ok(())
}

/// Run a command, **tolerating** failure with a warning — matches the
/// script's `… || true` / "WARNING:" steps (CLT, Homebrew, wallpaper, desktop
/// cleanup), where a failure must not abort the golden.
async fn exec_tolerant(session: &SshSession, command: &str, what: &str) {
    match session.exec(command).await {
        Ok(out) if out.exit_code == 0 => {}
        Ok(out) => eprintln!(
            "  warning: {what} returned exit {} (continuing): {}",
            out.exit_code,
            out.stderr.trim()
        ),
        Err(e) => eprintln!("  warning: {what} could not run (continuing): {e}"),
    }
}

/// Best-effort teardown of the setup VM on a boot-1 failure — `tart stop` +
/// `tart delete`, then SIGTERM the detached pid. Ports the script's
/// `trap cleanup EXIT`. The deliberate *handoff* (success) does **not** call
/// this: `030` takes over the running VM.
fn cleanup_setup_vm(id: &str, pid: i32) {
    crate::tart::remove_existing(id);
    if pid > 0 {
        crate::process::terminate(pid, Duration::from_millis(200), 10);
    }
}

/// Write `contents` to `dir/name`, creating `dir`. A host-side scratch file
/// (compiled wallpaper helper, plist) staged for SFTP upload.
fn write_scratch(dir: &Path, name: &str, contents: &[u8]) -> Result<PathBuf, VmError> {
    std::fs::create_dir_all(dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", dir.display())))?;
    let path = dir.join(name);
    std::fs::write(&path, contents)
        .map_err(|e| VmError::Io(format!("write {}: {e}", path.display())))?;
    Ok(path)
}

/// Boot-1 entry point: clone+boot the vanilla setup VM and provision it over
/// SSH, returning the [`SetupVm`] handoff for leaf `030`. On any hard
/// failure the setup VM is torn down before returning the error; on success
/// it is left **running** by design.
pub async fn provision_boot1(opts: &GoldenOptions, paths: &VmPaths) -> Result<SetupVm, VmError> {
    // Fail fast on host prerequisites before booting anything.
    let (pubkey, private_key) = resolve_ssh_key()?;
    let agent_bin = resolve_agent_bin()?;
    eprintln!("Using SSH key: {}", pubkey.display());
    eprintln!("Using macOS agent: {}", agent_bin.display());

    // Delete any existing golden of the same name first (script lines ~98–105).
    if crate::tart::vm_exists(&opts.name) {
        eprintln!("Deleting existing golden image '{}'...", opts.name);
        crate::tart::remove_existing(&opts.name);
    }

    // A distinct setup-VM id so it is never mistaken for a `vm start` clone
    // or a golden. `generate_id()` yields `testanyware-<hex8>`; splice in a
    // `setup-` marker. Ports `_SETUP_VM="testanyware-setup-$$"`.
    let setup_id = generate_id().replacen("testanyware-", "testanyware-setup-", 1);
    let vanilla = vanilla_image(&opts.version);

    eprintln!("Cloning {vanilla} → {setup_id}...");
    eprintln!("(This may pull the image on first run — can take several minutes)");
    crate::tart::clone(&vanilla, &setup_id)?;

    // Boot detached (`tart run --no-graphics --vnc-experimental`); the VNC
    // line in the run log is the booting signal.
    eprintln!("Booting setup VM...");
    let (pid, log_path) = crate::tart::run_detached(&setup_id, &paths.vms_dir())?;
    if crate::tart::poll_vnc_url(&log_path, 60, Duration::from_secs(1)).is_none() {
        cleanup_setup_vm(&setup_id, pid);
        return Err(VmError::VmBootTimeout { id: setup_id });
    }

    // Run the provisioning, tearing the VM down on any hard failure.
    match provision_running(opts, &setup_id, pid, &private_key, &pubkey, &agent_bin, paths).await {
        Ok(setup) => Ok(setup),
        Err(e) => {
            cleanup_setup_vm(&setup_id, pid);
            Err(e)
        }
    }
}

/// The provisioning body, factored out so `provision_boot1` can wrap it in a
/// single cleanup-on-failure guard. Resolves the guest IP, opens the
/// password session to install the pubkey, then drives every later step over
/// a pubkey session.
#[allow(clippy::too_many_arguments)]
async fn provision_running(
    opts: &GoldenOptions,
    setup_id: &str,
    pid: i32,
    private_key: &Path,
    pubkey: &Path,
    agent_bin: &Path,
    paths: &VmPaths,
) -> Result<SetupVm, VmError> {
    // Wait for a state-gated guest IP (`tart-ip-lies`: trust the `running`
    // state, not a bare `tart ip`).
    eprint!("Waiting for guest IP...");
    let Some(ip) = crate::tart::poll_ip(setup_id, 60, Duration::from_secs(3)) else {
        eprintln!(" timed out.");
        return Err(VmError::VmBootTimeout { id: setup_id.to_string() });
    };
    eprintln!(" {ip}");

    // First contact: password auth to the vanilla image (russh does this
    // in-process — no SSH_ASKPASS dance; ADR-0007). Retry until sshd answers.
    eprint!("Waiting for SSH...");
    let pw_session = SshSession::wait_for_password(
        &ip, 22, VANILLA_USER, VANILLA_PASS, 60, Duration::from_secs(3),
    )
    .await
    .inspect_err(|_| eprintln!(" not reachable."))?;
    eprintln!(" ready.");

    // --- Install the host pubkey (script lines ~175–189) ---
    eprintln!("Installing SSH key...");
    exec_checked(&pw_session, "mkdir -p ~/.ssh && chmod 700 ~/.ssh", "create ~/.ssh").await?;
    pw_session.upload(pubkey, "/tmp/host_key.pub").await?;
    exec_checked(
        &pw_session,
        "cat /tmp/host_key.pub >> ~/.ssh/authorized_keys && \
         chmod 600 ~/.ssh/authorized_keys && rm /tmp/host_key.pub",
        "append authorized_keys",
    )
    .await?;
    drop(pw_session);

    // Switch to pubkey auth for everything after (the script unsets askpass
    // here and relies on key auth). Verify it works without a password.
    let session = SshSession::connect_key(&ip, 22, VANILLA_USER, private_key).await?;
    let verify = session.exec("echo ok").await?;
    if verify.exit_code != 0 || !verify.stdout.contains("ok") {
        return Err(VmError::GoldenCreateFailed {
            detail: "SSH key auth verification failed — password auth still required".into(),
        });
    }
    eprintln!("SSH key auth verified.");

    // --- macOS defaults: disable session restore (script lines ~191–197) ---
    eprintln!("Configuring macOS defaults...");
    for cmd in session_restore_defaults() {
        exec_tolerant(&session, cmd, "defaults write").await;
    }

    // --- Solid-gray wallpaper (script lines ~199–215) ---
    // Compile the helper on the host (needs AppKit), upload, run. Tolerate a
    // missing `swiftc` exactly as the script does (warn + skip).
    provision_wallpaper(&session, setup_id, paths).await;

    // --- Hide desktop widgets (script line ~220) ---
    exec_tolerant(
        &session,
        "defaults write com.apple.WindowManager StandardHideWidgets -bool true",
        "hide desktop widgets",
    )
    .await;

    // --- Xcode Command Line Tools (script lines ~222–239) — tolerate ---
    provision_xcode_clt(&session).await;

    // --- Homebrew (script lines ~241–250) — tolerate ---
    eprintln!("Installing Homebrew (this can take a while)...");
    exec_tolerant(
        &session,
        "NONINTERACTIVE=1 /bin/bash -c \"$(curl -fsSL \
         https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"",
        "Homebrew install",
    )
    .await;
    exec_tolerant(
        &session,
        "echo 'eval \"$(/opt/homebrew/bin/brew shellenv)\"' >> ~/.zprofile",
        "Homebrew shellenv",
    )
    .await;

    // --- Close Terminal + clear saved app state (script lines ~252–257) ---
    eprintln!("Closing Terminal and clearing saved app state...");
    exec_tolerant(&session, "killall Terminal 2>/dev/null || true", "close Terminal").await;
    exec_tolerant(
        &session,
        "rm -rf ~/Library/Saved\\ Application\\ State/* 2>/dev/null || true",
        "clear saved app state",
    )
    .await;

    // --- Install the agent binary + LaunchAgent plist (script lines ~261–328) ---
    provision_agent(&session, agent_bin, setup_id, paths).await?;

    eprintln!(
        "Boot-1 provisioning complete. Setup VM '{setup_id}' is running at {ip} \
         (pid {pid}), ready for the recovery/SIP/TCC cycle (golden name '{}').",
        opts.name
    );
    Ok(SetupVm {
        id: setup_id.to_string(),
        pid,
        ip,
        key_path: private_key.to_path_buf(),
    })
}

/// Wallpaper step (script lines ~199–215): generate the PNG on the guest,
/// compile the embedded helper on the host, upload + run. Wholly tolerant —
/// a missing `swiftc` or any failure warns and skips.
async fn provision_wallpaper(session: &SshSession, setup_id: &str, paths: &VmPaths) {
    eprintln!("Setting wallpaper to solid gray...");
    let Some(swiftc) = crate::qemu_profile::which("swiftc") else {
        eprintln!("  warning: swiftc not found — skipping wallpaper");
        return;
    };
    let scratch = paths.vms_dir().join(format!("golden-{setup_id}"));
    let src = match write_scratch(&scratch, "set-wallpaper.swift", SET_WALLPAPER_SWIFT.as_bytes()) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  warning: could not stage wallpaper helper — skipping ({e})");
            return;
        }
    };
    let bin = scratch.join("set-wallpaper");
    let compiled = std::process::Command::new(swiftc)
        .arg("-o")
        .arg(&bin)
        .arg(&src)
        .output();
    match compiled {
        Ok(out) if out.status.success() => {}
        _ => {
            eprintln!("  warning: could not compile set-wallpaper helper — skipping wallpaper");
            return;
        }
    }
    exec_tolerant(session, &wallpaper_png_cmd(), "generate wallpaper PNG").await;
    if session.upload(&bin, "/tmp/set-wallpaper").await.is_err() {
        eprintln!("  warning: could not upload wallpaper helper — skipping");
        return;
    }
    exec_tolerant(
        session,
        &format!(
            "chmod +x /tmp/set-wallpaper && \
             /tmp/set-wallpaper /Users/{VANILLA_USER}/Pictures/solid_gray.png && \
             rm /tmp/set-wallpaper"
        ),
        "set wallpaper",
    )
    .await;
    let _ = std::fs::remove_dir_all(&scratch);
}

/// Xcode CLT step (script lines ~222–239): discover the install label via
/// `softwareupdate -l`, install it. Tolerant throughout (warn + continue).
async fn provision_xcode_clt(session: &SshSession) {
    eprintln!("Installing Xcode Command Line Tools (this takes a few minutes)...");
    // The sentinel file makes the CLT package appear in `softwareupdate -l`.
    exec_tolerant(
        session,
        "touch /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress",
        "CLT install sentinel",
    )
    .await;
    let listing = session.exec("softwareupdate -l 2>&1").await;
    let label = listing.ok().and_then(|o| parse_clt_label(&o.stdout));
    match label {
        Some(label) => {
            eprintln!("  found: {label}");
            exec_tolerant(
                session,
                &format!("softwareupdate --install '{label}' --verbose 2>&1 | tail -1"),
                "CLT install",
            )
            .await;
        }
        None => eprintln!("  warning: no Command Line Tools label found — skipping"),
    }
    exec_tolerant(
        session,
        "rm -f /tmp/.com.apple.dt.CommandLineTools.installondemand.in-progress",
        "clear CLT sentinel",
    )
    .await;
}

/// Agent install (script lines ~261–328): upload the binary to
/// `/usr/local/bin/testanyware-agent` and the embedded plist to
/// `~/Library/LaunchAgents/`. Hard-fails if the binary does not install —
/// the golden is useless without it.
async fn provision_agent(
    session: &SshSession,
    agent_bin: &Path,
    setup_id: &str,
    paths: &VmPaths,
) -> Result<(), VmError> {
    eprintln!("Installing testanyware-agent to VM...");
    session.upload(agent_bin, "/tmp/testanyware-agent").await?;
    exec_checked(session, "sudo mkdir -p /usr/local/bin", "create /usr/local/bin").await?;
    exec_checked(
        session,
        "sudo mv /tmp/testanyware-agent /usr/local/bin/testanyware-agent",
        "move agent into /usr/local/bin",
    )
    .await?;
    exec_checked(
        session,
        "sudo chmod +x /usr/local/bin/testanyware-agent",
        "chmod agent",
    )
    .await?;
    exec_checked(session, "test -x /usr/local/bin/testanyware-agent", "verify agent installed")
        .await?;
    eprintln!("  testanyware-agent binary installed.");

    eprintln!("Installing launchd plist...");
    let scratch = paths.vms_dir().join(format!("golden-{setup_id}"));
    let plist = write_scratch(
        &scratch,
        "com.linkuistics.testanyware.agent.plist",
        AGENT_PLIST.as_bytes(),
    )?;
    session
        .upload(&plist, "/tmp/com.linkuistics.testanyware.agent.plist")
        .await?;
    exec_checked(session, "mkdir -p ~/Library/LaunchAgents", "create ~/Library/LaunchAgents").await?;
    exec_checked(
        session,
        "mv /tmp/com.linkuistics.testanyware.agent.plist ~/Library/LaunchAgents/",
        "install LaunchAgent plist",
    )
    .await?;
    let _ = std::fs::remove_dir_all(&scratch);
    eprintln!("  LaunchAgent plist installed.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_image_matches_cirrus_naming() {
        assert_eq!(
            vanilla_image("tahoe"),
            "ghcr.io/cirruslabs/macos-tahoe-vanilla:latest"
        );
        assert_eq!(
            vanilla_image("sequoia"),
            "ghcr.io/cirruslabs/macos-sequoia-vanilla:latest"
        );
    }

    #[test]
    fn ssh_pubkey_candidates_prefer_ed25519() {
        let c = ssh_pubkey_candidates(Path::new("/Users/alice"));
        assert_eq!(c, vec![
            PathBuf::from("/Users/alice/.ssh/id_ed25519.pub"),
            PathBuf::from("/Users/alice/.ssh/id_rsa.pub"),
        ]);
    }

    #[test]
    fn private_key_for_strips_pub_suffix() {
        assert_eq!(
            private_key_for(Path::new("/Users/alice/.ssh/id_ed25519.pub")),
            PathBuf::from("/Users/alice/.ssh/id_ed25519")
        );
        // Defensive: no `.pub` suffix is returned unchanged.
        assert_eq!(
            private_key_for(Path::new("/Users/alice/.ssh/id_ed25519")),
            PathBuf::from("/Users/alice/.ssh/id_ed25519")
        );
    }

    #[test]
    fn agent_bin_under_prefix_matches_brew_layout() {
        assert_eq!(
            agent_bin_under_prefix(Path::new("/opt/homebrew/opt/testanyware")),
            PathBuf::from(
                "/opt/homebrew/opt/testanyware/share/testanyware/agents/macos/testanyware-agent"
            )
        );
    }

    #[test]
    fn session_restore_defaults_disable_window_restore() {
        let cmds = session_restore_defaults();
        assert_eq!(cmds.len(), 4);
        assert!(cmds.iter().all(|c| c.starts_with("defaults write")));
        assert!(cmds[0].contains("NSQuitAlwaysKeepsWindows"));
        assert!(cmds.iter().any(|c| c.contains("LoginwindowLaunchesRelaunchApps")));
    }

    #[test]
    fn wallpaper_png_cmd_decodes_scales_and_installs() {
        let cmd = wallpaper_png_cmd();
        assert!(cmd.contains(SOLID_GRAY_PNG_BASE64));
        assert!(cmd.contains("base64 -d"));
        assert!(cmd.contains("sips -z 1080 1920"));
        assert!(cmd.contains("~/Pictures/solid_gray.png"));
    }

    #[test]
    fn parse_clt_label_extracts_label_above_the_title_line() {
        // Realistic `softwareupdate -l` shape: the `* Label:` line sits above
        // the `Title: … Command Line Tools …` detail line.
        let out = "Software Update Tool\n\
                   \n\
                   Finding available software\n\
                   Software Update found the following new or updated software:\n\
                   * Label: Command Line Tools for Xcode-16.0\n\
                   \tTitle: Command Line Tools for Xcode, Version: 16.0, Size: 731770KiB, Recommended: YES,\n";
        assert_eq!(
            parse_clt_label(out).as_deref(),
            Some("Command Line Tools for Xcode-16.0")
        );
    }

    #[test]
    fn parse_clt_label_matches_when_clt_named_on_the_label_line() {
        let out = "* Label: Command Line Tools beta 6 for Xcode-16.1\n";
        assert_eq!(
            parse_clt_label(out).as_deref(),
            Some("Command Line Tools beta 6 for Xcode-16.1")
        );
    }

    #[test]
    fn parse_clt_label_is_none_when_only_unrelated_updates_listed() {
        let out = "* Label: macOS Sequoia 15.1\n\
                   \tTitle: macOS Sequoia 15.1, Version: 15.1, Size: 1234KiB\n";
        assert_eq!(parse_clt_label(out), None);
    }

    #[test]
    fn parse_clt_label_is_none_for_empty_output() {
        assert_eq!(parse_clt_label(""), None);
    }

    #[test]
    fn embedded_helpers_are_present() {
        // The include_str! helpers must be non-empty — a moved/renamed
        // provisioner file would otherwise fail the build, but this guards
        // an accidental emptying.
        assert!(AGENT_PLIST.contains("com.linkuistics.testanyware.agent"));
        assert!(SET_WALLPAPER_SWIFT.contains("setDesktopImageURL"));
    }
}
