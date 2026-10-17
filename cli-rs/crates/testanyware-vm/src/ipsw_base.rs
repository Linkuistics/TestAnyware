//! A macOS base VM built from Apple's IPSW — the stand-in for a Cirrus Labs
//! *vanilla* image when none is published for a macOS release (grove leaf
//! `ipsw-base-vm-k14`). **macOS-host only**, like `tart`/`golden`.
//!
//! Golden creation ([`crate::golden::provision_boot1`]) clones a base VM into a
//! throwaway setup VM and provisions it over SSH as `admin`/`admin`
//! ([`crate::golden::VANILLA_USER`] / [`crate::golden::VANILLA_PASS`]). Cirrus
//! publishes such a base for each release it supports, with the account made
//! and Remote Login on. For a release it does not (macOS 27 at the time of
//! writing) this module makes the equivalent itself:
//!
//! 1. resolve the IPSW for a macOS major from Apple's catalogue, download it
//!    into the cache (resumable);
//! 2. `tart create --from-ipsw` — Virtualization.framework installs macOS;
//! 3. first boot, drive Setup Assistant over RFB + OCR (ADR-0008) to create
//!    the `admin` account;
//! 4. turn Remote Login on and prove it by an SSH login as `admin`/`admin`;
//! 5. leave the VM stopped under [`base_vm_name`].
//!
//! The base is built once and cloned from for every golden, so the install is
//! paid for once per macOS release.
//!
//! ## The `--version` selector
//!
//! `vm create-golden --platform macos --version <v>` names either a Cirrus
//! vanilla tag (`tahoe`, `sequoia`, `sonoma`) or a macOS **major number**
//! (`27`). [`base_source`] decides which by shape: all digits means "build it
//! from the IPSW", anything else is a Cirrus tag. The existing tags are never
//! numeric, so the tahoe/sequoia/sonoma path is unchanged.

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::VmError;
pub use crate::base_source::{base_source, base_vm_name, BaseSource};
use crate::paths::VmPaths;

/// One macOS restore image listed in Apple's catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpswRelease {
    /// The macOS version, e.g. `27.0.1`.
    pub version: String,
    /// The build, e.g. `26A434`.
    pub build: String,
    pub url: String,
}

impl IpswRelease {
    /// The macOS major version (`27` for `27.0.1`).
    pub fn major(&self) -> Option<u32> {
        self.version.split('.').next()?.parse().ok()
    }

    /// The file name this release is cached under.
    pub fn cache_file_name(&self) -> String {
        format!("macos-{}-{}.ipsw", self.version, self.build)
    }
}

/// Apple's macOS IPSW catalogue — the source `tart create --from-ipsw latest`
/// reads.
pub const IPSW_CATALOGUE_URL: &str =
    "https://mesu.apple.com/assets/macos/com_apple_macOSIPSW/com_apple_macOSIPSW.xml";

/// Every Mac restore image in the catalogue text. The catalogue is an XML plist
/// whose restore images are `<string>` URLs ending
/// `UniversalMac_<version>_<build>_Restore.ipsw`; those file names are read
/// directly rather than walking the plist, which has changed shape before
/// while the file name convention has not.
pub fn parse_ipsw_catalogue(text: &str) -> Vec<IpswRelease> {
    let mut releases: Vec<IpswRelease> = Vec::new();
    for (end, _) in text.match_indices("_Restore.ipsw") {
        let end = end + "_Restore.ipsw".len();
        let start = text[..end]
            .rfind(|c: char| c == '<' || c == '>' || c.is_whitespace() || c == '"')
            .map_or(0, |i| i + 1);
        let url = &text[start..end];
        let Some(file) = url.rsplit('/').next() else { continue };
        let Some(stem) = file.strip_prefix("UniversalMac_").and_then(|f| f.strip_suffix("_Restore.ipsw"))
        else {
            continue;
        };
        let Some((version, build)) = stem.split_once('_') else { continue };
        let version_ok = !version.is_empty()
            && version.split('.').all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()));
        if !url.starts_with("https://") || !version_ok || build.is_empty() {
            continue;
        }
        let release = IpswRelease {
            version: version.to_string(),
            build: build.to_string(),
            url: url.to_string(),
        };
        if !releases.contains(&release) {
            releases.push(release);
        }
    }
    releases
}

/// The version's numeric parts, for ordering `27.0.10` after `27.0.2`.
fn version_key(version: &str) -> Vec<u32> {
    version.split('.').filter_map(|p| p.parse().ok()).collect()
}

/// The newest release of macOS `major` in `releases`, or `None` when the
/// catalogue lists none (Apple only lists the releases it still signs).
pub fn select_ipsw(releases: &[IpswRelease], major: u32) -> Option<&IpswRelease> {
    releases
        .iter()
        .filter(|r| r.major() == Some(major))
        .max_by_key(|r| version_key(&r.version))
}

/// Where a release's IPSW is cached (`<cache_dir>/macos-27.0.1-26A434.ipsw`).
pub fn cached_ipsw_path(cache_dir: &Path, release: &IpswRelease) -> PathBuf {
    cache_dir.join(release.cache_file_name())
}

// ---- base configuration (pure, unit-tested) -----------------------------

/// The XOR key macOS uses to obscure the auto-login password in
/// `/etc/kcpassword`. It is obfuscation, not encryption, and has been stable
/// since Mac OS X 10.x.
const KCPASSWORD_KEY: [u8; 11] = [0x7D, 0x89, 0x52, 0x23, 0xD2, 0xBC, 0xDD, 0xEA, 0xA3, 0xB9, 0x1F];

/// The contents of `/etc/kcpassword` for an auto-login `password`: the bytes
/// XORed with [`KCPASSWORD_KEY`], zero-padded to a multiple of 12 *before* the
/// XOR (so the padding is key bytes), with always at least one pad byte.
pub fn kcpassword_bytes(password: &str) -> Vec<u8> {
    let raw = password.as_bytes();
    let padded_len = (raw.len() / 12 + 1) * 12;
    (0..padded_len)
        .map(|i| raw.get(i).copied().unwrap_or(0) ^ KCPASSWORD_KEY[i % KCPASSWORD_KEY.len()])
        .collect()
}

/// `kcpassword_bytes` as a `printf` argument (`\x1c\xed…`).
fn printf_escapes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("\\x{b:02x}")).collect()
}

/// Single-quote `s` for a POSIX shell.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// What a Cirrus vanilla image gives golden creation beyond a login: the
/// commands (label, shell) that make a fresh macOS install equivalent, run over
/// SSH as `user` with password `pass` (so the first `sudo` takes the password
/// on stdin). In order:
///
/// 1. passwordless `sudo` — boot-1 runs `sudo` with no terminal;
/// 2. auto-login — the agent is a per-user LaunchAgent, so it needs a GUI
///    session after every boot;
/// 3. no display/system sleep and no screen saver — an idle clone's
///    framebuffer must not go black and lock.
///
/// Gatekeeper is deliberately left on. Cirrus's image has assessments off, but
/// nothing in golden creation needs that, and on macOS 27 `spctl
/// --global-disable` refuses ("needs to be confirmed in System Settings"), so
/// it would take another driven GUI screen for no stated need.
pub fn base_config_commands(user: &str, pass: &str) -> Vec<(&'static str, String)> {
    let sudoers = format!("/etc/sudoers.d/{user}-nopasswd");
    let kc = printf_escapes(&kcpassword_bytes(pass));
    vec![
        (
            "passwordless sudo",
            format!(
                "printf '%s\\n' {pw} | sudo -S -p '' sh -c {inner} && sudo -n true",
                pw = sh_quote(pass),
                inner = sh_quote(&format!(
                    "echo '{user} ALL=(ALL) NOPASSWD: ALL' > {sudoers} && chmod 440 {sudoers}"
                )),
            ),
        ),
        (
            "auto-login",
            format!(
                "sudo defaults write /Library/Preferences/com.apple.loginwindow autoLoginUser {user} && \
                 printf '{kc}' | sudo tee /etc/kcpassword >/dev/null && \
                 sudo chmod 600 /etc/kcpassword && sudo chown root:wheel /etc/kcpassword"
            ),
        ),
        (
            "no sleep or screen saver",
            "sudo pmset -a displaysleep 0 sleep 0 disksleep 0 && \
             defaults -currentHost write com.apple.screensaver idleTime 0 && \
             sudo defaults write /Library/Preferences/com.apple.screensaver loginWindowIdleTime 0"
                .to_string(),
        ),
    ]
}

// ---- live orchestration (verified on the Mac, not unit-tested) -----------

/// Look up the newest IPSW for `major` in Apple's catalogue.
pub fn resolve_release(major: u32) -> Result<IpswRelease, VmError> {
    let out = std::process::Command::new(crate::qemu_profile::which("curl").unwrap_or_else(|| "curl".into()))
        .args(["-fsSL", IPSW_CATALOGUE_URL])
        .output()
        .map_err(|e| VmError::GoldenCreateFailed { detail: format!("fetch IPSW catalogue: {e}") })?;
    if !out.status.success() {
        return Err(VmError::GoldenCreateFailed {
            detail: format!("fetch IPSW catalogue {IPSW_CATALOGUE_URL}: curl exited {}", out.status),
        });
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let releases = parse_ipsw_catalogue(&text);
    select_ipsw(&releases, major).cloned().ok_or_else(|| VmError::GoldenCreateFailed {
        detail: format!(
            "Apple's IPSW catalogue lists no macOS {major} restore image (it lists: {}). \
             Use `--version tahoe` (a Cirrus Labs vanilla image) or another macOS major.",
            releases.iter().map(|r| r.version.as_str()).collect::<Vec<_>>().join(", ")
        ),
    })
}

/// Ensure `release`'s IPSW is in the cache, downloading it if not. The download
/// is resumable (`curl -C -` into a `.part` file, renamed on completion), so an
/// interrupted 25 GB fetch continues rather than restarting.
pub fn ensure_ipsw(release: &IpswRelease, cache_dir: &Path) -> Result<PathBuf, VmError> {
    let dest = cached_ipsw_path(cache_dir, release);
    if dest.is_file() {
        return Ok(dest);
    }
    std::fs::create_dir_all(cache_dir)
        .map_err(|e| VmError::Io(format!("create {}: {e}", cache_dir.display())))?;
    let part = dest.with_extension("ipsw.part");
    eprintln!(
        "Downloading macOS {} ({}) IPSW into {} (about 25 GB; resumable)...",
        release.version,
        release.build,
        cache_dir.display()
    );
    let curl = crate::qemu_profile::which("curl").unwrap_or_else(|| "curl".into());
    let status = std::process::Command::new(curl)
        .args(["-fL", "-C", "-", "--retry", "5", "--progress-bar", "-o"])
        .arg(&part)
        .arg(&release.url)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::inherit())
        .status()
        .map_err(|e| VmError::GoldenCreateFailed { detail: format!("download IPSW: {e}") })?;
    if !status.success() {
        return Err(VmError::GoldenCreateFailed {
            detail: format!("download {}: curl exited {status}", release.url),
        });
    }
    std::fs::rename(&part, &dest)
        .map_err(|e| VmError::Io(format!("rename {} → {}: {e}", part.display(), dest.display())))?;
    Ok(dest)
}

/// Disk size of a base VM in GB — the same 50 GB the Cirrus vanilla images
/// have, which is what `tart create` defaults to as well.
const BASE_DISK_GB: u32 = 50;

/// How long to wait for the base VM's `tart` process to exit after
/// `shutdown -h now`, before forcing a `tart stop`.
const SHUTDOWN_WAIT_ATTEMPTS: u32 = 60;
const SHUTDOWN_WAIT_INTERVAL: Duration = Duration::from_secs(2);

/// The base VM for `major`: the existing one if it is already built, otherwise
/// build it (download the IPSW, install macOS, walk Setup Assistant, enable
/// SSH, configure — see the module docs). Returns the tart VM name to clone.
///
/// The VM is built under a `-building` name and renamed into place only when
/// complete, so a failed or interrupted build never leaves a half-made base
/// that a later run would mistake for a good one.
pub async fn ensure_base_vm(major: u32, paths: &VmPaths) -> Result<String, VmError> {
    let name = base_vm_name(major);
    if crate::tart::vm_exists(&name) {
        eprintln!("Using existing base VM '{name}'.");
        return Ok(name);
    }
    let release = resolve_release(major)?;
    eprintln!("No base VM '{name}'; building one from macOS {} ({}).", release.version, release.build);
    let ipsw = ensure_ipsw(&release, &paths.cache_dir())?;

    let building = format!("{name}-building");
    crate::tart::remove_existing(&building);
    eprintln!("Installing macOS into '{building}' (a few minutes)...");
    crate::tart::create_from_ipsw(&building, &ipsw, BASE_DISK_GB)?;

    match configure_fresh_vm(&building, paths).await {
        Ok(()) => {
            crate::tart::rename(&building, &name)?;
            eprintln!("Base VM '{name}' is ready.");
            Ok(name)
        }
        Err(e) => {
            eprintln!("Base VM build failed — removing '{building}'.");
            crate::tart::remove_existing(&building);
            Err(e)
        }
    }
}

/// Take a freshly installed (Setup Assistant not yet run) stopped VM to the
/// base state: `admin`/`admin`, Remote Login on, [`base_config_commands`]
/// applied, left shut down. Public so the live test can iterate on a clone of
/// an already-installed VM instead of reinstalling macOS each time.
pub async fn configure_fresh_vm(vm: &str, paths: &VmPaths) -> Result<(), VmError> {
    use crate::golden::{VANILLA_PASS, VANILLA_USER};
    use crate::recovery::RecoverySession;
    use crate::ssh::SshSession;

    eprintln!("Booting '{vm}' to Setup Assistant...");
    let (pid, log_path) = crate::tart::run_detached(vm, &paths.vms_dir())?;
    let result: Result<(), VmError> = async {
        let Some(vnc) = crate::tart::poll_vnc_url(&log_path, 60, Duration::from_secs(1)) else {
            return Err(VmError::boot_timeout(vm, crate::error::BootStage::Vnc));
        };
        let mut session = RecoverySession::connect(&vnc.host, vnc.port, vnc.password.as_deref()).await?;
        let driven = async {
            crate::setup_assistant::run_setup_assistant(&mut session, VANILLA_USER, VANILLA_PASS).await?;
            crate::setup_assistant::enable_remote_login(&mut session, VANILLA_PASS).await
        }
        .await;
        session.shutdown().await;
        driven?;

        eprint!("Waiting for guest IP...");
        let Some(ip) = crate::tart::poll_ip(vm, 60, Duration::from_secs(3)) else {
            eprintln!(" timed out.");
            return Err(VmError::boot_timeout(vm, crate::error::BootStage::GuestIp));
        };
        eprintln!(" {ip}");
        eprint!("Waiting for SSH...");
        let ssh = SshSession::wait_for_password(&ip, 22, VANILLA_USER, VANILLA_PASS, 60, Duration::from_secs(3))
            .await
            .inspect_err(|_| eprintln!(" not reachable."))?;
        eprintln!(" ready.");

        for (what, command) in base_config_commands(VANILLA_USER, VANILLA_PASS) {
            eprintln!("Configuring base: {what}...");
            crate::golden::exec_checked(&ssh, &command, what).await?;
        }
        // The connection drops as the guest goes down; that is the success path.
        eprintln!("Shutting the base VM down...");
        let _ = ssh.exec("sudo shutdown -h now").await;
        Ok(())
    }
    .await;

    match result {
        Ok(()) => {
            if !crate::recovery::wait_for_pid_exit(pid, SHUTDOWN_WAIT_ATTEMPTS, SHUTDOWN_WAIT_INTERVAL) {
                eprintln!("  shutdown did not finish — forcing stop.");
                crate::tart::stop(vm);
                crate::process::terminate(pid, Duration::from_millis(200), 10);
            }
            Ok(())
        }
        Err(e) => {
            crate::tart::stop(vm);
            crate::process::terminate(pid, Duration::from_millis(200), 10);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CATALOGUE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0"><dict>
  <key>Restore</key>
  <string>https://updates.cdn-apple.com/2026FallFCS/59241290-5d51/UniversalMac_27.0.1_26A434_Restore.ipsw</string>
  <key>Old</key>
  <string>https://updates.cdn-apple.com/2025FallFCS/abc/UniversalMac_26.0.1_25A362_Restore.ipsw</string>
  <string>https://updates.cdn-apple.com/2025FallFCS/def/UniversalMac_26.1_25B78_Restore.ipsw</string>
  <string>https://updates.cdn-apple.com/2026FallFCS/59241290-5d51/UniversalMac_27.0.1_26A434_Restore.ipsw</string>
  <string>https://updates.cdn-apple.com/x/UniversalMac_27.0.10_26A999_Restore.ipsw</string>
  <string>https://updates.cdn-apple.com/x/UniversalMac_27.0.2_26A500_Restore.ipsw</string>
  <string>https://example.com/not-a-mac/iPhone_27.0_Restore.ipsw</string>
  <string>http://insecure.example/UniversalMac_27.9_26Z1_Restore.ipsw</string>
</dict></plist>"#;

    #[test]
    fn catalogue_parse_reads_mac_restore_urls_only_and_dedupes() {
        let releases = parse_ipsw_catalogue(CATALOGUE);
        let versions: Vec<&str> = releases.iter().map(|r| r.version.as_str()).collect();
        assert_eq!(versions, ["27.0.1", "26.0.1", "26.1", "27.0.10", "27.0.2"]);
        let first = &releases[0];
        assert_eq!(first.build, "26A434");
        assert_eq!(
            first.url,
            "https://updates.cdn-apple.com/2026FallFCS/59241290-5d51/UniversalMac_27.0.1_26A434_Restore.ipsw"
        );
    }

    #[test]
    fn select_picks_the_newest_of_the_major_by_numeric_version() {
        let releases = parse_ipsw_catalogue(CATALOGUE);
        // 27.0.10 is newer than 27.0.2 numerically, though not lexically.
        assert_eq!(select_ipsw(&releases, 27).map(|r| r.version.as_str()), Some("27.0.10"));
        assert_eq!(select_ipsw(&releases, 26).map(|r| r.version.as_str()), Some("26.1"));
        assert_eq!(select_ipsw(&releases, 28), None);
    }

    #[test]
    fn empty_or_unrelated_text_lists_nothing() {
        assert!(parse_ipsw_catalogue("").is_empty());
        assert!(parse_ipsw_catalogue("<plist><dict/></plist>").is_empty());
    }

    #[test]
    fn cache_path_names_version_and_build() {
        let r = IpswRelease {
            version: "27.0.1".into(),
            build: "26A434".into(),
            url: "https://x/UniversalMac_27.0.1_26A434_Restore.ipsw".into(),
        };
        assert_eq!(r.major(), Some(27));
        assert_eq!(
            cached_ipsw_path(Path::new("/c"), &r),
            PathBuf::from("/c/macos-27.0.1-26A434.ipsw")
        );
    }

    #[test]
    fn kcpassword_matches_a_known_vector() {
        // Observed live: the file macOS accepted for password "admin".
        assert_eq!(
            kcpassword_bytes("admin"),
            [0x1c, 0xed, 0x3f, 0x4a, 0xbc, 0xbc, 0xdd, 0xea, 0xa3, 0xb9, 0x1f, 0x7d]
        );
    }

    #[test]
    fn kcpassword_pads_to_a_multiple_of_twelve_with_at_least_one_pad_byte() {
        assert_eq!(kcpassword_bytes("").len(), 12);
        assert_eq!(kcpassword_bytes("elevenchars").len(), 12);
        // Exactly 12 characters still gets a full block of padding.
        assert_eq!(kcpassword_bytes("twelve-chars").len(), 24);
        // XOR is its own inverse: decoding recovers the password then zeros.
        let enc = kcpassword_bytes("hunter2");
        let dec: Vec<u8> = enc.iter().enumerate().map(|(i, b)| b ^ KCPASSWORD_KEY[i % 11]).collect();
        assert_eq!(&dec[..7], b"hunter2");
        assert!(dec[7..].iter().all(|&b| b == 0));
    }

    #[test]
    fn base_config_covers_sudo_autologin_and_sleep_in_that_order() {
        let cmds = base_config_commands("admin", "admin");
        let labels: Vec<&str> = cmds.iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, ["passwordless sudo", "auto-login", "no sleep or screen saver"]);
        let (_, sudo) = &cmds[0];
        assert!(sudo.contains("NOPASSWD: ALL"));
        assert!(sudo.contains("/etc/sudoers.d/admin-nopasswd"));
        // The first sudo has no passwordless rule yet, so it reads the password.
        assert!(sudo.contains("sudo -S"));
        let (_, login) = &cmds[1];
        assert!(login.contains("autoLoginUser admin"));
        assert!(login.contains(r"\x1c\xed\x3f\x4a"), "kcpassword bytes missing: {login}");
        assert!(login.contains("/etc/kcpassword"));
        let (_, sleep) = &cmds[2];
        assert!(sleep.contains("displaysleep 0") && sleep.contains("idleTime 0"));
    }

    #[test]
    fn shell_quoting_survives_a_quote_in_the_password() {
        assert_eq!(sh_quote("it's"), r#"'it'\''s'"#);
        let cmds = base_config_commands("admin", "p'w");
        assert!(cmds[0].1.contains(r#"'p'\''w'"#));
    }
}
