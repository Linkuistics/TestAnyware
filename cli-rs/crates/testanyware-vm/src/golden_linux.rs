//! Linux golden-image creation: **normal-mode SSH provisioning** (macOS host).
//!
//! Full Rust port of `provisioner/scripts/vm-create-golden-linux.sh` (grove leaf
//! `230-vm-create-golden-linux`), the command body of
//! `vm create-golden --platform linux`. Mirrors the macOS port
//! ([`crate::golden`] + [`crate::finalize`]) but is **far simpler**: Linux has no
//! System Integrity Protection, so there is **no SIP/TCC/Recovery cycle**. The
//! whole flow is two *normal* boots over the [`SshSession`](crate::ssh) seam
//! (ADR-0007):
//!
//!   1. **Boot 1 — provision** (this module): `tart clone` a vanilla Cirrus Labs
//!      Ubuntu image into a throwaway *setup VM*, boot it, install the host
//!      pubkey, then drive the whole desktop+agent provisioning over SSH —
//!      Ubuntu Desktop (minimal), NetworkManager via netplan, Firefox, GDM
//!      autologin forced to X11, a solid-gray locked-down desktop, full system
//!      updates, silent boot, and the `testanyware_agent` Python package as a
//!      systemd **user** service with AT-SPI2 enabled.
//!   2. **Boot 2 — apply + finalize**: reboot so GDM autologins and the agent
//!      service starts ([`crate::recovery::reboot_and_wait_ssh`]), gate on the
//!      agent's `/health` reporting `accessible: true`, disable+mask sshd (clones
//!      need no SSH — the agent's HTTP surface is the only ingress), clean
//!      shutdown, and `tart clone` to the golden.
//!
//! **Reuse, not duplication.** ADR-0007 made the `russh` provisioning seam and
//! the [`crate::golden`] host-side helpers (`resolve_ssh_key`, `exec_checked`,
//! `exec_tolerant`, `brew_prefix`) backend-neutral for exactly this; the
//! apply-settings reboot reuses [`crate::recovery::reboot_and_wait_ssh`]. Only
//! the Linux-specific bits live here: the Ubuntu vanilla image ref, the
//! directory-shaped agent (a Python package tarred + SFTP'd, vs macOS's
//! single-file binary), and the provisioning sequence itself.
//!
//! **macOS-host only** (`tart` wraps Virtualization.framework — the Linux golden
//! is built *on this Mac* via tart, exactly like the macOS golden), so the module
//! is `#[cfg(target_os = "macos")]`-gated at the crate root.
//!
//! ## Fatal vs. tolerant (parity with the script)
//!
//! The script branches unevenly on `vm_ssh` exit, and that asymmetry is
//! load-bearing: package installs and config writes the golden cannot work
//! without are **fatal** (`exec_checked`); cosmetic / best-effort steps the
//! script guards with `|| true` are **tolerant** (`exec_tolerant`). The
//! authoritative gate remains the post-reboot agent-health check.
//!
//! ## Progress narration
//!
//! Like the macOS port, this multi-minute flow narrates to **stderr** via
//! `eprintln!`, keeping `--json` stdout clean. The pure helpers (image ref, agent
//! resolution, health parse, config bodies) are unit-tested; the live `tart`/SSH
//! orchestration is verified by actually creating a golden on the Mac (cheap —
//! [[vm-costs]]), not by unit tests — the same policy as `golden.rs`/`tart.rs`.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::VmError;
use crate::golden::{
    brew_prefix, cleanup_setup_vm, exec_checked, exec_tolerant, resolve_ssh_key, GoldenOptions,
    SetupVm, VANILLA_PASS, VANILLA_USER,
};
use crate::id::generate_id;
use crate::paths::VmPaths;
use crate::recovery::{reboot_and_wait_ssh, wait_for_pid_exit};
use crate::ssh::SshSession;

// ---- constants ----------------------------------------------------------

/// Where the agent's Python package lands in the guest. Ports the script's
/// `/opt/testanyware` install root.
const AGENT_INSTALL_ROOT: &str = "/opt/testanyware";

/// Agent-health gate budget: 60 × 2s = 120s of `/health` polling (script
/// parity, the post-reboot `seq 1 60` / `sleep 2` loop).
const AGENT_HEALTH_ATTEMPTS: u32 = 60;
const AGENT_HEALTH_INTERVAL: Duration = Duration::from_secs(2);

/// Post-autologin settle before the health gate — gives the desktop session a
/// moment to come up so the user service starts (script's `sleep 10`).
const DESKTOP_SETTLE: Duration = Duration::from_secs(10);

/// Final-shutdown pid-exit wait before forcing a `tart stop`. 60 × 2s = 120s,
/// matching the script's shutdown-wait loop.
const FINAL_SHUTDOWN_ATTEMPTS: u32 = 60;
const FINAL_SHUTDOWN_INTERVAL: Duration = Duration::from_secs(2);

// ---- pure helpers (unit-tested) -----------------------------------------

/// The vanilla Cirrus Labs Ubuntu image ref for a Linux `version`. Ports
/// `_VANILLA="ghcr.io/cirruslabs/ubuntu:$_VERSION"`.
pub fn vanilla_image_linux(version: &str) -> String {
    format!("ghcr.io/cirruslabs/ubuntu:{version}")
}

/// The brew-bundled Linux agent **directory** under a `brew --prefix
/// testanyware`. Unlike macOS (a single binary), the Linux agent is a Python
/// *package directory* (`testanyware_agent/`). Ports
/// `$_BREW_PREFIX/share/testanyware/agents/linux`.
pub fn linux_agent_dir_under_prefix(brew_prefix: &Path) -> PathBuf {
    brew_prefix.join("share/testanyware/agents/linux")
}

/// `/etc/netplan/01-network-manager-all.yaml` body: delegate all networking to
/// NetworkManager (the base image renders via systemd-networkd, which Ubuntu
/// Desktop's NM has nothing to manage). Ports the script's netplan heredoc.
fn netplan_nm_yaml() -> &'static str {
    "network:\n  version: 2\n  renderer: NetworkManager\n"
}

/// `/etc/gdm3/custom.conf` body: autologin `admin` and force X11
/// (`WaylandEnable=false`) — the agent's xdotool coordinate fix only works
/// under X11 (GTK4 AT-SPI returns (0,0) under Wayland). Ports the script's
/// GDM heredoc.
fn gdm_custom_conf() -> &'static str {
    "[daemon]\nAutomaticLoginEnable=True\nAutomaticLogin=admin\nWaylandEnable=false\n"
}

/// `/usr/share/glib-2.0/schemas/99-testanyware.gschema.override` body: solid
/// gray desktop, no screen lock/blank, no notification banners, and AT-SPI2
/// toolkit accessibility on. Folds the script's two override writes (the
/// desktop block + the later AT-SPI2 append) into one file — the compiled
/// result is identical.
fn gschema_override() -> &'static str {
    "[org.gnome.desktop.background]\n\
     picture-options='none'\n\
     primary-color='#808080'\n\
     \n\
     [org.gnome.desktop.screensaver]\n\
     lock-enabled=false\n\
     \n\
     [org.gnome.desktop.session]\n\
     idle-delay=uint32 0\n\
     \n\
     [org.gnome.desktop.notifications]\n\
     show-banners=false\n\
     \n\
     [org.gnome.desktop.interface]\n\
     toolkit-accessibility=true\n"
}

/// `/opt/testanyware/run-agent.sh` launcher. Ports the script's launcher
/// heredoc — runs the Python package from the install root.
fn run_agent_launcher() -> String {
    format!("#!/bin/bash\ncd {AGENT_INSTALL_ROOT}\nexec python3 -m testanyware_agent\n")
}

/// `~/.config/systemd/user/testanyware-agent.service` body: a **user** service
/// (not system) so it runs inside the graphical session and can reach AT-SPI2.
/// Ports the script's systemd unit heredoc.
fn systemd_user_unit() -> String {
    format!(
        "[Unit]\n\
         Description=TestAnyware Agent TCP Service\n\
         After=graphical-session.target\n\
         \n\
         [Service]\n\
         Type=simple\n\
         ExecStart={AGENT_INSTALL_ROOT}/run-agent.sh\n\
         Restart=always\n\
         RestartSec=5\n\
         Environment=DISPLAY=:0\n\
         \n\
         [Install]\n\
         WantedBy=default.target\n"
    )
}

/// Whether an agent `/health` body reports `accessible: true`. Ports the
/// script's `grep -q '"accessible": *true'`, but parses the JSON so spacing /
/// key order cannot fool it. A non-JSON or `accessible: false` body is `false`.
fn health_accessible(body: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .and_then(|v| v.get("accessible").and_then(|a| a.as_bool()))
        .unwrap_or(false)
}

/// Build a `sudo tee <path> > /dev/null` command fed a quoted heredoc, so a
/// multi-line config `body` is written verbatim (the `'EOF'` marker disables
/// expansion — `$` and backticks stay literal). The marker is unusual enough
/// not to collide with config bodies. `sudo` because the targets are
/// root-owned; callers that write under `$HOME` drop it via `tee_user`.
fn tee_root(path: &str, body: &str) -> String {
    format!("sudo tee {path} > /dev/null << 'TESTANYWARE_EOF'\n{body}TESTANYWARE_EOF\n")
}

/// Like [`tee_root`] but writes a user-owned file (no `sudo`) — for the
/// systemd **user** unit and launcher under `$HOME`/`/opt` already chowned.
fn tee_user(path: &str, body: &str) -> String {
    format!("tee {path} > /dev/null << 'TESTANYWARE_EOF'\n{body}TESTANYWARE_EOF\n")
}

// ---- live orchestration (verified on the Mac, not unit-tested) -----------

/// Resolve the Linux agent **directory** (containing the `testanyware_agent`
/// package) to install. Honors `TESTANYWARE_AGENT_BIN_OVERRIDE` (a contributor
/// build's `agents/linux` dir); otherwise the brew-bundled tree under `brew
/// --prefix testanyware`. Ports the script's agent-resolution block.
fn resolve_linux_agent_dir() -> Result<PathBuf, VmError> {
    let dir = if let Ok(over) = std::env::var("TESTANYWARE_AGENT_BIN_OVERRIDE") {
        if over.is_empty() {
            None
        } else {
            Some(PathBuf::from(over))
        }
    } else {
        None
    };
    let dir = match dir {
        Some(d) => d,
        None => {
            let prefix = brew_prefix("testanyware").ok_or_else(|| VmError::GoldenCreateFailed {
                detail: "`brew --prefix testanyware` failed — install with \
                         `brew install Linkuistics/taps/testanyware`, or set \
                         TESTANYWARE_AGENT_BIN_OVERRIDE to a contributor `agents/linux` dir"
                    .into(),
            })?;
            linux_agent_dir_under_prefix(&prefix)
        }
    };
    if !dir.join("testanyware_agent").is_dir() {
        return Err(VmError::GoldenCreateFailed {
            detail: format!(
                "Linux agent package not found at {}/testanyware_agent",
                dir.display()
            ),
        });
    }
    Ok(dir)
}

/// The command entry point: produce a Linux golden from a vanilla Ubuntu image.
/// Clones+boots a throwaway setup VM, provisions it over SSH, reboots to apply
/// settings, gates on agent health, disables sshd, shuts down cleanly, and
/// clones to the golden. Returns the golden's name on success.
///
/// On any failure the setup VM is torn down (mirroring the macOS port's
/// cleanup-on-failure guard) so a botched run never strands a setup VM; on
/// success it is consumed by the final `tart clone` + `tart delete`.
pub async fn create_golden_linux(opts: &GoldenOptions, paths: &VmPaths) -> Result<String, VmError> {
    // Fail fast on host prerequisites before booting anything.
    let (pubkey, private_key) = resolve_ssh_key()?;
    let agent_dir = resolve_linux_agent_dir()?;
    eprintln!("Using SSH key: {}", pubkey.display());
    eprintln!("Using Linux agent: {}", agent_dir.display());

    // Delete any existing golden of the same name first.
    if crate::tart::vm_exists(&opts.name) {
        eprintln!("Deleting existing golden image '{}'...", opts.name);
        crate::tart::remove_existing(&opts.name);
    }

    let setup_id = generate_id().replacen("testanyware-", "testanyware-setup-", 1);
    let vanilla = vanilla_image_linux(&opts.version);
    eprintln!("Cloning {vanilla} → {setup_id}...");
    eprintln!("(This may pull the image on first run — can take several minutes)");
    crate::tart::clone(&vanilla, &setup_id)?;

    // Boot detached (`tart run --no-graphics --vnc-experimental`); the
    // `--vnc-experimental` flag gives GDM a virtual display so it does not
    // crash-loop after reboot. The VNC line in the run log is the boot signal.
    eprintln!("Booting setup VM...");
    let (pid, log_path) = crate::tart::run_detached(&setup_id, &paths.vms_dir())?;
    if crate::tart::poll_vnc_url(&log_path, 60, Duration::from_secs(1)).is_none() {
        cleanup_setup_vm(&setup_id, pid);
        return Err(VmError::VmBootTimeout { id: setup_id });
    }

    // Carry the in-flight setup VM (pid changes across the reboot) so a failure
    // anywhere tears down the *current* pid.
    let setup = SetupVm {
        id: setup_id.clone(),
        pid,
        ip: String::new(),
        key_path: private_key.clone(),
    };
    match provision_and_finalize(opts, setup, &pubkey, &agent_dir, paths).await {
        Ok(name) => Ok(name),
        Err((setup, err)) => {
            eprintln!("Golden creation failed — tearing down setup VM '{}'.", setup.id);
            cleanup_setup_vm(&setup.id, setup.pid);
            Err(err)
        }
    }
}

/// The pipeline body after the setup VM is booted. Takes ownership of the
/// [`SetupVm`] and, on failure, hands the *latest* one back with the error so
/// the caller tears down the right pid (the apply-settings reboot mints a new
/// pid; the id is stable).
async fn provision_and_finalize(
    opts: &GoldenOptions,
    mut setup: SetupVm,
    pubkey: &Path,
    agent_dir: &Path,
    paths: &VmPaths,
) -> Result<String, (SetupVm, VmError)> {
    // Resolve a state-gated guest IP (`tart-ip-lies`: trust `running`, not a
    // bare `tart ip`).
    eprint!("Waiting for guest IP...");
    let Some(ip) = crate::tart::poll_ip(&setup.id, 60, Duration::from_secs(3)) else {
        eprintln!(" timed out.");
        return Err((setup.clone(), VmError::VmBootTimeout { id: setup.id }));
    };
    eprintln!(" {ip}");
    setup.ip = ip.clone();

    // First contact: password auth to the vanilla image, install the host
    // pubkey, switch to key auth for everything after.
    let session = match install_key_and_connect(&ip, pubkey, &setup.key_path).await {
        Ok(s) => s,
        Err(e) => return Err((setup, e)),
    };

    // Boot-1 provisioning (desktop + agent).
    if let Err(e) = provision_desktop_and_agent(&session, agent_dir, &setup.id, paths).await {
        return Err((setup, e));
    }
    drop(session);

    // --- Reboot to apply settings (script's shutdown → restart cycle) ---
    eprintln!("Shutting down VM for the apply-settings reboot...");
    shutdown_guest(&setup).await;
    eprint!("Waiting for shutdown...");
    if wait_for_pid_exit(setup.pid, 60, Duration::from_secs(2)) {
        eprintln!(" done.");
    } else {
        eprintln!(" forcing stop.");
        crate::tart::stop(&setup.id);
        crate::process::terminate(setup.pid, Duration::from_millis(200), 10);
    }
    setup = match reboot_and_wait_ssh(&setup, paths).await {
        Ok(s) => s,
        Err(e) => return Err((setup, e)),
    };
    // Give the desktop a moment to finish autologin so the user service starts.
    tokio::time::sleep(DESKTOP_SETTLE).await;

    // Agent-health gate — fatal.
    if let Err(e) = verify_agent_health(&setup).await {
        return Err((setup, e));
    }

    // Finalize: disable+mask sshd, clean shutdown.
    finalize_and_shutdown(&setup).await;

    // Clone to golden (consumes the setup VM).
    if let Err(e) = clone_to_golden(&setup.id, &opts.name) {
        return Err((setup, e));
    }
    Ok(opts.name.clone())
}

/// Open a password session to the vanilla image, install the host pubkey, then
/// return a verified **key-auth** session for all later provisioning. Mirrors
/// the macOS boot-1 pubkey handoff.
async fn install_key_and_connect(
    ip: &str,
    pubkey: &Path,
    private_key: &Path,
) -> Result<SshSession, VmError> {
    eprint!("Waiting for SSH...");
    let pw = SshSession::wait_for_password(ip, 22, VANILLA_USER, VANILLA_PASS, 60, Duration::from_secs(3))
        .await
        .inspect_err(|_| eprintln!(" not reachable."))?;
    eprintln!(" ready.");

    eprintln!("Installing SSH key...");
    exec_checked(&pw, "mkdir -p ~/.ssh && chmod 700 ~/.ssh", "create ~/.ssh").await?;
    pw.upload(pubkey, "/tmp/host_key.pub").await?;
    exec_checked(
        &pw,
        "cat /tmp/host_key.pub >> ~/.ssh/authorized_keys && \
         chmod 600 ~/.ssh/authorized_keys && rm /tmp/host_key.pub",
        "append authorized_keys",
    )
    .await?;
    drop(pw);

    let session = SshSession::connect_key(ip, 22, VANILLA_USER, private_key).await?;
    let verify = session.exec("echo ok").await?;
    if verify.exit_code != 0 || !verify.stdout.contains("ok") {
        return Err(VmError::GoldenCreateFailed {
            detail: "SSH key auth verification failed — password auth still required".into(),
        });
    }
    eprintln!("SSH key auth verified.");
    Ok(session)
}

/// The boot-1 provisioning sequence over a key-auth session: Ubuntu Desktop,
/// NetworkManager, Firefox, GDM autologin, desktop lockdown, system updates,
/// silent boot, and the agent as a systemd user service. Ports the bulk of the
/// script's `vm_ssh` calls in order.
async fn provision_desktop_and_agent(
    session: &SshSession,
    agent_dir: &Path,
    setup_id: &str,
    paths: &VmPaths,
) -> Result<(), VmError> {
    install_ubuntu_desktop(session).await?;
    configure_network_manager(session).await?;
    install_firefox(session).await;
    configure_autologin(session).await?;
    configure_desktop(session).await?;
    system_updates(session).await;
    configure_silent_boot(session).await;
    install_agent(session, agent_dir, setup_id, paths).await?;
    Ok(())
}

/// Install `ubuntu-desktop-minimal`, with the script's service-suppression
/// dance: block service auto-start during the install (policy-rc.d + a
/// `systemctl`→`/bin/true` divert), pin out the snap `firefox`, install, then
/// restore everything and quiet `unattended-upgrades` so it cannot grab the
/// dpkg lock. Ports script lines ~204–280.
async fn install_ubuntu_desktop(session: &SshSession) -> Result<(), VmError> {
    eprintln!("Installing Ubuntu Desktop (this takes several minutes)...");
    exec_checked(session, "sudo DEBIAN_FRONTEND=noninteractive apt-get update -q", "apt-get update").await?;
    exec_tolerant(
        session,
        "sudo DEBIAN_FRONTEND=noninteractive apt-get remove -y needrestart >/dev/null 2>&1 || true",
        "remove needrestart",
    )
    .await;

    // Block service auto-start during install (two mechanisms — invoke-rc.d and
    // direct systemctl calls).
    exec_checked(
        session,
        "printf '#!/bin/sh\\nexit 101\\n' | sudo tee /usr/sbin/policy-rc.d > /dev/null && \
         sudo chmod +x /usr/sbin/policy-rc.d",
        "install policy-rc.d",
    )
    .await?;
    exec_checked(
        session,
        "sudo dpkg-divert --local --rename --add /usr/bin/systemctl && \
         sudo ln -sf /bin/true /usr/bin/systemctl",
        "divert systemctl",
    )
    .await?;

    // Pin the snap firefox out of this apt run (it needs snapd, which can't
    // start with systemctl diverted); installed separately afterwards.
    exec_checked(
        session,
        "printf 'Package: firefox\\nPin: release *\\nPin-Priority: -1\\n' | \
         sudo tee /etc/apt/preferences.d/no-firefox > /dev/null",
        "pin out firefox",
    )
    .await?;

    exec_checked(
        session,
        "sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
         -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold' \
         ubuntu-desktop-minimal",
        "install ubuntu-desktop-minimal",
    )
    .await?;

    exec_tolerant(session, "sudo rm -f /etc/apt/preferences.d/no-firefox", "remove firefox pin").await;

    // Mask unattended-upgrades BEFORE restoring systemctl so it cannot start and
    // grab the dpkg lock.
    exec_tolerant(
        session,
        "sudo ln -sf /dev/null /etc/systemd/system/unattended-upgrades.service",
        "pre-mask unattended-upgrades",
    )
    .await;

    // Restore systemctl + policy-rc.d so services start normally on boot.
    exec_checked(
        session,
        "sudo rm -f /usr/bin/systemctl && sudo dpkg-divert --local --rename --remove /usr/bin/systemctl",
        "restore systemctl",
    )
    .await?;
    exec_tolerant(session, "sudo rm -f /usr/sbin/policy-rc.d", "remove policy-rc.d").await;

    // Reload systemd so it reads the mask, then fully quiet unattended-upgrades
    // and wait for the dpkg lock to release.
    exec_tolerant(session, "sudo systemctl daemon-reload", "daemon-reload").await;
    exec_tolerant(session, "sudo systemctl stop unattended-upgrades.service 2>/dev/null || true", "stop unattended-upgrades").await;
    exec_tolerant(session, "sudo systemctl mask unattended-upgrades.service 2>/dev/null || true", "mask unattended-upgrades").await;
    exec_tolerant(session, "sudo killall unattended-upgr 2>/dev/null || true", "kill unattended-upgr").await;
    exec_tolerant(
        session,
        "while sudo fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1; do sleep 1; done",
        "wait for dpkg lock",
    )
    .await;
    eprintln!("  Ubuntu Desktop installed.");
    Ok(())
}

/// Switch networking from systemd-networkd to NetworkManager via netplan (the
/// base image renders networkd; Ubuntu Desktop's NM needs the renderer flipped
/// or it manages nothing). Ports script lines ~283–305.
async fn configure_network_manager(session: &SshSession) -> Result<(), VmError> {
    eprintln!("Configuring NetworkManager via netplan...");
    exec_tolerant(session, "sudo rm -f /etc/netplan/*.yaml", "clear netplan configs").await;
    exec_checked(
        session,
        &tee_root("/etc/netplan/01-network-manager-all.yaml", netplan_nm_yaml()),
        "write netplan config",
    )
    .await?;
    exec_checked(session, "sudo chmod 600 /etc/netplan/01-network-manager-all.yaml", "chmod netplan config").await?;
    exec_tolerant(
        session,
        "sudo systemctl disable systemd-networkd.service systemd-networkd-wait-online.service 2>/dev/null || true",
        "disable systemd-networkd",
    )
    .await;
    exec_tolerant(session, "sudo systemctl enable NetworkManager.service 2>/dev/null || true", "enable NetworkManager").await;
    Ok(())
}

/// Install Firefox now that snapd can run (systemctl restored). Tolerant — a
/// missing browser must not abort the golden. Ports script lines ~308–315.
async fn install_firefox(session: &SshSession) {
    eprintln!("Installing Firefox (snap)...");
    exec_tolerant(session, "sudo apt-mark unhold firefox 2>/dev/null || true", "unhold firefox").await;
    exec_tolerant(
        session,
        "while sudo fuser /var/lib/dpkg/lock-frontend >/dev/null 2>&1; do sleep 1; done",
        "wait for dpkg lock",
    )
    .await;
    exec_tolerant(session, "sudo DEBIAN_FRONTEND=noninteractive apt-get install -y firefox", "install firefox").await;
    eprintln!("  Firefox installed.");
}

/// Configure GDM autologin forced to X11 and skip the GNOME initial-setup
/// wizard. Ports script lines ~317–333.
async fn configure_autologin(session: &SshSession) -> Result<(), VmError> {
    eprintln!("Configuring autologin and forcing X11 session...");
    exec_checked(session, &tee_root("/etc/gdm3/custom.conf", gdm_custom_conf()), "write GDM custom.conf").await?;
    exec_tolerant(
        session,
        "mkdir -p ~/.config && echo 'yes' > ~/.config/gnome-initial-setup-done",
        "skip initial-setup wizard",
    )
    .await;
    Ok(())
}

/// Solid-gray locked-down desktop + AT-SPI2 via a gschema override. Ports script
/// lines ~335–360 (the two override writes folded into one).
async fn configure_desktop(session: &SshSession) -> Result<(), VmError> {
    eprintln!("Configuring desktop settings...");
    exec_checked(
        session,
        &tee_root(
            "/usr/share/glib-2.0/schemas/99-testanyware.gschema.override",
            gschema_override(),
        ),
        "write gschema override",
    )
    .await?;
    exec_checked(session, "sudo glib-compile-schemas /usr/share/glib-2.0/schemas/", "compile gschemas").await?;
    Ok(())
}

/// Run all pending system updates so the golden ships fully patched, and disable
/// the update-notifier + apt-daily timers so nothing pops up during tests.
/// Tolerant throughout (cosmetic / long-running). Ports script lines ~362–372.
async fn system_updates(session: &SshSession) {
    eprintln!("Running system updates (this may take a few minutes)...");
    exec_tolerant(
        session,
        "sudo DEBIAN_FRONTEND=noninteractive apt-get upgrade -y \
         -o Dpkg::Options::='--force-confdef' -o Dpkg::Options::='--force-confold'",
        "system upgrade",
    )
    .await;
    exec_tolerant(session, "sudo apt-get remove -y update-notifier 2>/dev/null || true", "remove update-notifier").await;
    exec_tolerant(
        session,
        "sudo systemctl disable apt-daily.timer apt-daily-upgrade.timer 2>/dev/null || true",
        "disable apt-daily timers",
    )
    .await;
    eprintln!("  Updates complete.");
}

/// Configure GRUB for a silent, instant boot straight to the GUI (drop the
/// cloud-image console override, quiet+splash, zero timeout). Tolerant — boot
/// cosmetics must not abort the golden. Ports script lines ~374–386.
async fn configure_silent_boot(session: &SshSession) {
    eprintln!("Configuring silent boot...");
    exec_tolerant(session, "sudo rm -f /etc/default/grub.d/50-cloudimg-settings.cfg", "drop cloudimg grub override").await;
    exec_tolerant(
        session,
        "sudo sed -i 's/^GRUB_CMDLINE_LINUX_DEFAULT=.*/GRUB_CMDLINE_LINUX_DEFAULT=\"quiet splash loglevel=0 vt.global_cursor_default=0\"/' /etc/default/grub",
        "set GRUB_CMDLINE_LINUX_DEFAULT",
    )
    .await;
    exec_tolerant(session, "sudo sed -i 's/^GRUB_TIMEOUT=.*/GRUB_TIMEOUT=0/' /etc/default/grub", "set GRUB_TIMEOUT").await;
    exec_tolerant(
        session,
        "grep -q '^GRUB_TIMEOUT_STYLE=' /etc/default/grub && \
         sudo sed -i 's/^GRUB_TIMEOUT_STYLE=.*/GRUB_TIMEOUT_STYLE=hidden/' /etc/default/grub || \
         echo 'GRUB_TIMEOUT_STYLE=hidden' | sudo tee -a /etc/default/grub > /dev/null",
        "set GRUB_TIMEOUT_STYLE",
    )
    .await;
    exec_tolerant(session, "sudo update-grub", "update-grub").await;
}

/// Install the `testanyware_agent` Python package and its systemd **user**
/// service. The package is a directory, so it is tarred on the host and SFTP'd
/// (vs macOS's single-file binary), then unpacked into `/opt/testanyware`.
/// Ports script lines ~388–470. Hard-fails if the agent does not install — the
/// golden is useless without it.
async fn install_agent(
    session: &SshSession,
    agent_dir: &Path,
    setup_id: &str,
    paths: &VmPaths,
) -> Result<(), VmError> {
    eprintln!("Installing testanyware-agent...");
    // xdotool (window-management fallback) + python3-pyatspi (AT-SPI2 bindings;
    // not in ubuntu-desktop-minimal).
    exec_checked(
        session,
        "sudo DEBIAN_FRONTEND=noninteractive apt-get install -y xdotool python3-pyatspi",
        "install agent deps (xdotool, python3-pyatspi)",
    )
    .await?;

    // Tar the package on the host (COPYFILE_DISABLE so macOS adds no AppleDouble
    // `._*` members — [[vm-ssh-from-harness]]), upload, unpack.
    let scratch = paths.vms_dir().join(format!("golden-{setup_id}"));
    let tarball = stage_agent_tarball(agent_dir, &scratch)?;
    exec_checked(session, &format!("sudo mkdir -p {AGENT_INSTALL_ROOT}"), "create /opt/testanyware").await?;
    let upload = session.upload(&tarball, "/tmp/testanyware-agent.tar").await;
    let _ = std::fs::remove_dir_all(&scratch);
    upload?;
    exec_checked(
        session,
        &format!("sudo tar -xf /tmp/testanyware-agent.tar -C {AGENT_INSTALL_ROOT} && rm -f /tmp/testanyware-agent.tar"),
        "unpack agent package",
    )
    .await?;
    exec_checked(
        session,
        &format!("test -d {AGENT_INSTALL_ROOT}/testanyware_agent"),
        "verify agent package installed",
    )
    .await?;

    // Launcher script (root-owned under /opt/testanyware, then made executable).
    // The write and the chmod are *separate* exec calls: a heredoc is terminated
    // by its marker line, so a `&&` chained onto the following line is a shell
    // syntax error (`&&` needs a left operand on the same logical line).
    let launcher_path = format!("{AGENT_INSTALL_ROOT}/run-agent.sh");
    exec_checked(session, &tee_root(&launcher_path, &run_agent_launcher()), "write agent launcher").await?;
    exec_checked(session, &format!("sudo chmod +x {launcher_path}"), "chmod agent launcher").await?;

    // systemd user service, enabled via a direct symlink (works without an
    // active user session, as the script does).
    exec_checked(session, "mkdir -p ~/.config/systemd/user", "create user systemd dir").await?;
    exec_checked(
        session,
        &tee_user("~/.config/systemd/user/testanyware-agent.service", &systemd_user_unit()),
        "install agent user service",
    )
    .await?;
    exec_checked(session, "mkdir -p ~/.config/systemd/user/default.target.wants", "create user wants dir").await?;
    exec_checked(
        session,
        "ln -sf ~/.config/systemd/user/testanyware-agent.service \
         ~/.config/systemd/user/default.target.wants/testanyware-agent.service",
        "enable agent user service",
    )
    .await?;

    // Open the firewall for the agent port (ufw may or may not be active).
    exec_tolerant(session, "sudo ufw allow 8648/tcp 2>/dev/null || true", "open firewall port 8648").await;
    eprintln!("  Agent installed.");
    Ok(())
}

/// Tar `<agent_dir>/testanyware_agent` into `<scratch>/testanyware-agent.tar` on
/// the host, returning the tarball path. `COPYFILE_DISABLE=1` keeps macOS from
/// embedding AppleDouble `._*` members the guest would unpack as junk.
fn stage_agent_tarball(agent_dir: &Path, scratch: &Path) -> Result<PathBuf, VmError> {
    std::fs::create_dir_all(scratch)
        .map_err(|e| VmError::Io(format!("create {}: {e}", scratch.display())))?;
    let tarball = scratch.join("testanyware-agent.tar");
    let out = std::process::Command::new("tar")
        .env("COPYFILE_DISABLE", "1")
        .arg("-cf")
        .arg(&tarball)
        .arg("-C")
        .arg(agent_dir)
        .arg("testanyware_agent")
        .output()
        .map_err(|e| VmError::GoldenCreateFailed { detail: format!("tar agent package: {e}") })?;
    if !out.status.success() {
        return Err(VmError::GoldenCreateFailed {
            detail: format!(
                "tar agent package failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            ),
        });
    }
    Ok(tarball)
}

/// Final agent-health gate: after the apply-settings reboot, GDM autologins and
/// the user service starts the agent on `0.0.0.0:8648`. Poll `/health` over SSH
/// (inside the guest) until it reports `accessible: true`. Fatal on timeout,
/// dumping the service status + journal to aid diagnosis (script lines ~474–495).
async fn verify_agent_health(setup: &SetupVm) -> Result<(), VmError> {
    eprint!("Waiting for agent at {}:8648...", setup.ip);
    let session = SshSession::connect_key(&setup.ip, 22, VANILLA_USER, &setup.key_path).await?;
    for attempt in 0..AGENT_HEALTH_ATTEMPTS {
        if let Ok(out) = session.exec("curl -sf --connect-timeout 2 http://localhost:8648/health").await {
            if out.exit_code == 0 && health_accessible(&out.stdout) {
                eprintln!(" ready.");
                eprintln!("  Agent health verified: {}", out.stdout.trim());
                return Ok(());
            }
        }
        if attempt + 1 < AGENT_HEALTH_ATTEMPTS {
            tokio::time::sleep(AGENT_HEALTH_INTERVAL).await;
        }
    }
    eprintln!(" timed out.");
    eprintln!("Debug: checking systemd user service status...");
    if let Ok(s) = session.exec("systemctl --user status testanyware-agent.service").await {
        eprintln!("{}", s.stdout.trim());
    }
    if let Ok(s) = session.exec("journalctl --user -u testanyware-agent.service --no-pager -n 20").await {
        eprintln!("{}", s.stdout.trim());
    }
    Err(VmError::GoldenCreateFailed {
        detail: "agent did not report accessible on http://localhost:8648/health within the health \
                 window — check the systemd user service on the setup VM"
            .into(),
    })
}

/// Disable + mask sshd (clones need no SSH — the agent's HTTP surface is the
/// only ingress, matching the Windows golden), then shut down cleanly. The
/// `disable`/`mask` deliberately omit `--now` so the running sshd survives long
/// enough to queue the shutdown in the same command. Ports script lines
/// ~497–515.
async fn finalize_and_shutdown(setup: &SetupVm) {
    eprintln!("Disabling SSH service and shutting down VM...");
    if let Ok(session) = SshSession::connect_key(&setup.ip, 22, VANILLA_USER, &setup.key_path).await {
        // A dropped session is expected (sshd is being disabled), hence tolerant.
        let _ = session
            .exec(
                "sudo systemctl disable ssh.service 2>/dev/null || sudo systemctl disable ssh 2>/dev/null; \
                 sudo systemctl mask ssh.service 2>/dev/null || sudo systemctl mask ssh 2>/dev/null; \
                 sudo shutdown -h now",
            )
            .await;
    } else {
        eprintln!("  warning: could not open SSH for the clean shutdown — forcing stop.");
    }

    eprint!("Waiting for shutdown...");
    if wait_for_pid_exit(setup.pid, FINAL_SHUTDOWN_ATTEMPTS, FINAL_SHUTDOWN_INTERVAL) {
        eprintln!(" done.");
    } else {
        eprintln!(" forcing stop.");
        crate::tart::stop(&setup.id);
        crate::process::terminate(setup.pid, Duration::from_millis(200), 10);
    }
}

/// Queue a best-effort clean shutdown of the guest (the apply-settings reboot's
/// first half). A dropped session is expected as the guest powers off.
async fn shutdown_guest(setup: &SetupVm) {
    if let Ok(session) = SshSession::connect_key(&setup.ip, 22, VANILLA_USER, &setup.key_path).await {
        let _ = session.exec("sudo shutdown -h now").await;
    }
}

/// `tart clone <setup> <golden>` then `tart delete <setup>`. The clone is the
/// one fatal step (no clone, no golden); a failed delete only leaks the
/// (stopped) setup VM, so it warns. Ports script lines ~509–512.
fn clone_to_golden(setup_id: &str, name: &str) -> Result<(), VmError> {
    eprintln!("Creating golden image '{name}'...");
    crate::tart::clone(setup_id, name)?;
    if !crate::tart::delete_golden(setup_id) {
        eprintln!("  warning: could not delete setup VM '{setup_id}' after cloning (golden is created).");
    }
    eprintln!("Golden image '{name}' created successfully.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vanilla_image_linux_matches_cirrus_naming() {
        assert_eq!(vanilla_image_linux("24.04"), "ghcr.io/cirruslabs/ubuntu:24.04");
        assert_eq!(vanilla_image_linux("22.04"), "ghcr.io/cirruslabs/ubuntu:22.04");
    }

    #[test]
    fn linux_agent_dir_matches_brew_layout() {
        assert_eq!(
            linux_agent_dir_under_prefix(Path::new("/opt/homebrew/opt/testanyware")),
            PathBuf::from("/opt/homebrew/opt/testanyware/share/testanyware/agents/linux")
        );
    }

    #[test]
    fn health_accessible_only_true_for_accessible_true() {
        assert!(health_accessible(r#"{"accessible": true, "platform": "linux"}"#));
        // Tolerant to spacing / key order (the script's grep needs exact spacing).
        assert!(health_accessible(r#"{"platform":"linux","accessible":true}"#));
        assert!(!health_accessible(r#"{"accessible": false, "platform": "linux"}"#));
        assert!(!health_accessible(r#"{"platform": "linux"}"#));
        assert!(!health_accessible("not json"));
        assert!(!health_accessible(""));
    }

    #[test]
    fn netplan_delegates_to_network_manager() {
        let yaml = netplan_nm_yaml();
        assert!(yaml.contains("renderer: NetworkManager"));
        assert!(yaml.contains("version: 2"));
    }

    #[test]
    fn gdm_conf_autologins_admin_and_forces_x11() {
        let conf = gdm_custom_conf();
        assert!(conf.contains("AutomaticLoginEnable=True"));
        assert!(conf.contains("AutomaticLogin=admin"));
        // X11, not Wayland — the agent's xdotool coordinate fix needs it.
        assert!(conf.contains("WaylandEnable=false"));
    }

    #[test]
    fn gschema_locks_down_desktop_and_enables_atspi() {
        let s = gschema_override();
        assert!(s.contains("picture-options='none'"));
        assert!(s.contains("primary-color='#808080'"));
        assert!(s.contains("lock-enabled=false"));
        assert!(s.contains("idle-delay=uint32 0"));
        assert!(s.contains("show-banners=false"));
        // AT-SPI2 folded into the same override.
        assert!(s.contains("toolkit-accessibility=true"));
    }

    #[test]
    fn systemd_unit_is_a_user_service_with_display() {
        let unit = systemd_user_unit();
        assert!(unit.contains("ExecStart=/opt/testanyware/run-agent.sh"));
        assert!(unit.contains("WantedBy=default.target")); // user-service install target
        assert!(unit.contains("Environment=DISPLAY=:0")); // needs the X11 display
        assert!(unit.contains("Restart=always"));
    }

    #[test]
    fn launcher_runs_the_python_package_from_install_root() {
        let l = run_agent_launcher();
        assert!(l.contains("cd /opt/testanyware"));
        assert!(l.contains("python3 -m testanyware_agent"));
    }

    #[test]
    fn tee_root_wraps_body_in_a_quoted_heredoc() {
        let cmd = tee_root("/etc/foo.conf", "a=1\nb=2\n");
        assert!(cmd.starts_with("sudo tee /etc/foo.conf > /dev/null << 'TESTANYWARE_EOF'\n"));
        assert!(cmd.contains("a=1\nb=2\n"));
        assert!(cmd.trim_end().ends_with("TESTANYWARE_EOF"));
    }

    #[test]
    fn tee_user_omits_sudo() {
        let cmd = tee_user("~/.config/x.service", "[Unit]\n");
        assert!(cmd.starts_with("tee ~/.config/x.service"));
        assert!(!cmd.contains("sudo"));
    }
}
