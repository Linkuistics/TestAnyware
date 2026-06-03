//! `testanyware doctor` — host preflight diagnostics.
//!
//! Ports `cli/Sources/testanyware/DoctorCommand.swift` and the checks under
//! `cli/Sources/TestAnywareDriver/Diagnostics/`. The contract (§10.1, the
//! `doctor` gap row) requires the Rust port to *add* a `--json` envelope the
//! Swift command never had, while keeping a readable text mode that renders
//! the same data.
//!
//! ## Pure core vs. runtime probes
//!
//! Each check splits into a **pure classifier** (platform-independent, fully
//! unit-tested — mirrors the Swift `classify(...)` statics) and a **runtime
//! probe layer** (`which`/`where`, `brew --prefix`, `<tool> --version`,
//! filesystem) that is `#[cfg]`-gated. Only the probe layer differs per host
//! OS; the classifiers compile and test identically everywhere. The dev host
//! is macOS, where the full check set runs for real; the Linux/Windows
//! branches are `#[cfg]`-gated and unit-covered (per-platform-facilities
//! direction). A host without Homebrew (e.g. a Windows host) yields the
//! benign `NoHomebrew` "skip" verdict for the bundle checks — the
//! host-conditional set falls out of the existing logic rather than a parallel
//! table.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use serde_json::{json, Value};

use crate::output::{OutputMode, SCHEMA_VERSION};

// =========================================================================
// Version parsing & comparison (ports ToolAvailabilityCheck.parseVersion +
// Swift's `compare(_:options:.numeric)`).
// =========================================================================

/// Pull the first `MAJOR.MINOR[.PATCH]` dotted-numeric token out of a
/// `--version` blob. Tolerant of varied formats: `2.32.1`,
/// `QEMU emulator version 11.0.0`, `swtpm version 0.10.0` all yield the
/// right dotted token. A standalone year like `2026` (no dot) is skipped.
pub fn parse_version(raw: &str) -> Option<String> {
    let mut current = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
        } else {
            if let Some(v) = take_dotted(&current) {
                return Some(v);
            }
            current.clear();
        }
    }
    take_dotted(&current)
}

fn take_dotted(s: &str) -> Option<String> {
    if s.contains('.') && s.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        Some(s.trim_end_matches('.').to_string())
    } else {
        None
    }
}

/// Numeric dotted-version comparison mirroring Swift's `.numeric` option:
/// each `.`-separated component compares as a number, and a missing trailing
/// component counts as `0` (so `2.0` == `2.0.0`). Non-numeric components
/// degrade to `0` — unparseable floors are surfaced via their own verdict
/// regardless of how they order here.
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let pa: Vec<u64> = a.split('.').map(|t| t.parse().unwrap_or(0)).collect();
    let pb: Vec<u64> = b.split('.').map(|t| t.parse().unwrap_or(0)).collect();
    let n = pa.len().max(pb.len());
    for i in 0..n {
        let x = pa.get(i).copied().unwrap_or(0);
        let y = pb.get(i).copied().unwrap_or(0);
        match x.cmp(&y) {
            Ordering::Equal => continue,
            other => return other,
        }
    }
    Ordering::Equal
}

// =========================================================================
// 1. Install-path check (ports InstallPathCheck).
// =========================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallVerdict {
    /// `which testanyware` resolves under the Homebrew prefix.
    HomebrewInstall { path: String, brew_prefix: String },
    /// On PATH outside the Homebrew prefix while a prefix exists — the
    /// dev-symlink-shadows-brew hazard the doctor is built to catch.
    Shadowed { path: String, brew_prefix: String },
    /// Homebrew is not installed; nothing to compare against.
    NoHomebrew { path: String },
    /// Nothing on PATH resolves to `testanyware`.
    NotOnPath { brew_prefix: Option<String> },
}

impl InstallVerdict {
    /// `true` for verdicts that should not block tooling. `false` for the
    /// dev-symlink-shadow case and the "binary disappeared" case.
    pub fn is_ok(&self) -> bool {
        matches!(
            self,
            InstallVerdict::HomebrewInstall { .. } | InstallVerdict::NoHomebrew { .. }
        )
    }
}

/// Pure classifier. `path_binary` is what `which testanyware` printed (the
/// on-PATH symlink itself, not its target); `brew_prefix` is the resolved
/// Homebrew prefix. A trailing-slash guard stops `/opt/homebrew` from
/// falsely matching `/opt/homebrew-evil`.
pub fn classify_install_path(
    path_binary: Option<&str>,
    brew_prefix: Option<&str>,
) -> InstallVerdict {
    match (path_binary, brew_prefix) {
        (Some(path), Some(prefix)) => {
            let normalized = if prefix.ends_with('/') {
                prefix.to_string()
            } else {
                format!("{prefix}/")
            };
            if path.starts_with(&normalized) {
                InstallVerdict::HomebrewInstall {
                    path: path.to_string(),
                    brew_prefix: prefix.to_string(),
                }
            } else {
                InstallVerdict::Shadowed {
                    path: path.to_string(),
                    brew_prefix: prefix.to_string(),
                }
            }
        }
        (Some(path), None) => InstallVerdict::NoHomebrew {
            path: path.to_string(),
        },
        (None, prefix) => InstallVerdict::NotOnPath {
            brew_prefix: prefix.map(str::to_string),
        },
    }
}

// =========================================================================
// Shared path-issue type for the bundle checks.
// =========================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathIssue {
    Missing { path: String },
    NotExecutable { path: String },
}

impl PathIssue {
    fn kind(&self) -> &'static str {
        match self {
            PathIssue::Missing { .. } => "missing",
            PathIssue::NotExecutable { .. } => "not_executable",
        }
    }
    fn path(&self) -> &str {
        match self {
            PathIssue::Missing { path } | PathIssue::NotExecutable { path } => path,
        }
    }
}

// =========================================================================
// 2. Bundled-agents check (ports BundledAgentsCheck).
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentSlot {
    Macos,
    Windows,
    Linux,
}

impl AgentSlot {
    pub fn as_str(self) -> &'static str {
        match self {
            AgentSlot::Macos => "macos",
            AgentSlot::Windows => "windows",
            AgentSlot::Linux => "linux",
        }
    }
}

pub struct SlotProbe {
    pub slot: AgentSlot,
    pub expected_path: String,
    pub exists: bool,
    pub is_executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentsVerdict {
    AllPresent { brew_prefix: String },
    Missing {
        brew_prefix: String,
        issues: Vec<(AgentSlot, PathIssue)>,
    },
    NoHomebrew,
}

impl AgentsVerdict {
    pub fn is_ok(&self) -> bool {
        !matches!(self, AgentsVerdict::Missing { .. })
    }
}

/// Where each slot's payload is expected under a brew prefix.
pub fn agent_expected_path(brew_prefix: &str, slot: AgentSlot) -> String {
    let base = format!("{brew_prefix}/share/testanyware/agents");
    match slot {
        AgentSlot::Macos => format!("{base}/macos/testanyware-agent"),
        AgentSlot::Windows => format!("{base}/windows/testanyware-agent.exe"),
        AgentSlot::Linux => format!("{base}/linux/testanyware_agent/__main__.py"),
    }
}

/// Pure classifier. Only the macOS agent's executable bit is asserted — the
/// `.exe` is a Windows binary and the Linux Python entry point isn't
/// executable by file mode either way.
pub fn classify_bundled_agents(brew_prefix: Option<&str>, probes: &[SlotProbe]) -> AgentsVerdict {
    let Some(prefix) = brew_prefix else {
        return AgentsVerdict::NoHomebrew;
    };
    let mut issues = Vec::new();
    for slot in [AgentSlot::Macos, AgentSlot::Windows, AgentSlot::Linux] {
        match probes.iter().find(|p| p.slot == slot) {
            None => issues.push((slot, PathIssue::Missing { path: "(unprobed)".into() })),
            Some(p) if !p.exists => {
                issues.push((slot, PathIssue::Missing { path: p.expected_path.clone() }))
            }
            Some(p) => {
                if slot == AgentSlot::Macos && !p.is_executable {
                    issues.push((slot, PathIssue::NotExecutable { path: p.expected_path.clone() }));
                }
            }
        }
    }
    if issues.is_empty() {
        AgentsVerdict::AllPresent { brew_prefix: prefix.to_string() }
    } else {
        AgentsVerdict::Missing { brew_prefix: prefix.to_string(), issues }
    }
}

// =========================================================================
// 3. Bundled-scripts check (ports BundledScriptsCheck).
// =========================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptSlot {
    Scripts,
    Helpers,
}

impl ScriptSlot {
    fn as_str(self) -> &'static str {
        match self {
            ScriptSlot::Scripts => "scripts",
            ScriptSlot::Helpers => "helpers",
        }
    }
}

/// Filenames staged into `share/testanyware/scripts/`. All require the
/// executable bit.
pub const SCRIPT_FILENAMES: &[&str] = &[
    "_testanyware-paths.sh",
    // Only the not-yet-ported Tier-2 golden scripts ship. The macOS golden is
    // the in-process `vm create-golden --platform macos` command (grove `110`,
    // ADR-0007/0008), and `vm start/stop/list/delete` are ported into the
    // binary — those five scripts are no longer bundled (grove `120`).
    "vm-create-golden-linux.sh",
    "vm-create-golden-windows.sh",
];

/// Filenames staged into `share/testanyware/helpers/`. Presence-only —
/// modes vary by file type.
pub const HELPER_FILENAMES: &[&str] = &[
    "SetupComplete.cmd",
    "autounattend.xml",
    "com.linkuistics.testanyware.agent.plist",
    "desktop-setup.ps1",
    "set-wallpaper.ps1",
    "set-wallpaper.swift",
];

pub struct FileProbe {
    pub slot: ScriptSlot,
    pub path: String,
    pub exists: bool,
    pub is_executable: bool,
    pub requires_executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScriptsVerdict {
    AllPresent { brew_prefix: String },
    Missing {
        brew_prefix: String,
        issues: Vec<(ScriptSlot, PathIssue)>,
    },
    NoHomebrew,
}

impl ScriptsVerdict {
    pub fn is_ok(&self) -> bool {
        !matches!(self, ScriptsVerdict::Missing { .. })
    }
}

pub fn expected_script_paths(brew_prefix: &str) -> Vec<String> {
    let base = format!("{brew_prefix}/share/testanyware/scripts");
    SCRIPT_FILENAMES.iter().map(|f| format!("{base}/{f}")).collect()
}

pub fn expected_helper_paths(brew_prefix: &str) -> Vec<String> {
    let base = format!("{brew_prefix}/share/testanyware/helpers");
    HELPER_FILENAMES.iter().map(|f| format!("{base}/{f}")).collect()
}

/// Pure classifier. Each probe carries its own `requires_executable` flag so
/// the executable-bit invariant is a per-file property, not a slot-wide one.
pub fn classify_bundled_scripts(brew_prefix: Option<&str>, probes: &[FileProbe]) -> ScriptsVerdict {
    let Some(prefix) = brew_prefix else {
        return ScriptsVerdict::NoHomebrew;
    };
    let mut issues = Vec::new();
    for p in probes {
        if !p.exists {
            issues.push((p.slot, PathIssue::Missing { path: p.path.clone() }));
        } else if p.requires_executable && !p.is_executable {
            issues.push((p.slot, PathIssue::NotExecutable { path: p.path.clone() }));
        }
    }
    if issues.is_empty() {
        ScriptsVerdict::AllPresent { brew_prefix: prefix.to_string() }
    } else {
        ScriptsVerdict::Missing { brew_prefix: prefix.to_string(), issues }
    }
}

// =========================================================================
// 4. Host-tool availability (ports ToolAvailabilityCheck).
// =========================================================================

pub struct Tool {
    pub name: &'static str,
    pub purpose: &'static str,
    pub install_hint: &'static str,
    pub minimum_version: Option<&'static str>,
}

/// The three tools the doctor reports on, in display order. Floors are the
/// oldest version known to support the features the provisioner scripts rely
/// on — newer is always fine, and all three are advisory (missing tools do
/// not fail the doctor).
pub const KNOWN_TOOLS: &[Tool] = &[
    Tool {
        name: "tart",
        purpose: "macOS and Linux VMs",
        install_hint: "brew install cirruslabs/cli/tart",
        minimum_version: Some("2.0.0"),
    },
    Tool {
        name: "qemu-system-aarch64",
        purpose: "Windows VMs",
        install_hint: "brew install qemu",
        minimum_version: Some("8.0.0"),
    },
    Tool {
        name: "swtpm",
        purpose: "TPM 2.0 emulation for Windows 11 VMs",
        install_hint: "brew install swtpm",
        minimum_version: None,
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionVerdict {
    Ok { detected: Option<String> },
    BelowFloor { detected: String, minimum: String },
    Unparseable { raw: String, minimum: String },
    ProbeFailed { minimum: String },
}

/// Compare a tool's raw `--version` output against an optional floor.
pub fn compare_tool_version(raw: Option<&str>, minimum: Option<&str>) -> VersionVerdict {
    let Some(min) = minimum else {
        return VersionVerdict::Ok {
            detected: parse_version(raw.unwrap_or("")),
        };
    };
    let Some(raw) = raw.filter(|s| !s.is_empty()) else {
        return VersionVerdict::ProbeFailed { minimum: min.to_string() };
    };
    match parse_version(raw) {
        None => VersionVerdict::Unparseable {
            raw: raw.trim().to_string(),
            minimum: min.to_string(),
        },
        Some(detected) => {
            if compare_versions(&detected, min) == Ordering::Less {
                VersionVerdict::BelowFloor { detected, minimum: min.to_string() }
            } else {
                VersionVerdict::Ok { detected: Some(detected) }
            }
        }
    }
}

pub struct ToolStatus {
    pub name: &'static str,
    pub purpose: &'static str,
    pub install_hint: &'static str,
    pub path: Option<String>,
    pub version: VersionVerdict,
}

impl ToolStatus {
    /// A tool entry is "clean" (no warning) when it resolved on PATH and its
    /// version is at/above the floor. Advisory only — never flips overall ok.
    fn is_clean(&self) -> bool {
        self.path.is_some() && matches!(self.version, VersionVerdict::Ok { .. })
    }
}

// =========================================================================
// 5. Provisioner-script version sentinels (ports
//    ProvisionerScriptsVersionCheck).
// =========================================================================

/// Sentinel recognised inside scanned scripts:
/// `# testanyware-min-tool: <name> <version>`.
pub const SENTINEL_PREFIX: &str = "# testanyware-min-tool:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclaredFloor {
    pub tool: String,
    pub minimum_version: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolVerdict {
    Ok { tool: String, host_version: String, declared_minimum: String, source: String },
    BelowFloor { tool: String, host_version: String, declared_minimum: String, source: String },
    HostVersionUnknown { tool: String, declared_minimum: String, source: String },
    Unparseable { tool: String, raw_value: String, source: String },
}

pub struct ProvisionerResult {
    pub per_tool: Vec<ToolVerdict>,
    pub skipped: bool,
}

/// Parse all sentinel lines out of `content`, attaching `source`. Lines that
/// match the prefix but don't tokenise as `<tool> <version>` produce a floor
/// with the malformed value preserved (so the user sees the typo).
pub fn parse_sentinels(content: &str, source: &str) -> Vec<DeclaredFloor> {
    let mut floors = Vec::new();
    for raw_line in content.split('\n') {
        let line = raw_line.trim();
        let Some(payload) = line.strip_prefix(SENTINEL_PREFIX) else {
            continue;
        };
        let payload = payload.trim();
        let tokens: Vec<&str> = payload.split_whitespace().collect();
        if tokens.len() >= 2 {
            floors.push(DeclaredFloor {
                tool: tokens[0].to_string(),
                minimum_version: tokens[1].to_string(),
                source: source.to_string(),
            });
        } else {
            floors.push(DeclaredFloor {
                tool: tokens.first().copied().unwrap_or("(unknown)").to_string(),
                minimum_version: tokens.iter().skip(1).copied().collect::<Vec<_>>().join(" "),
                source: source.to_string(),
            });
        }
    }
    floors
}

/// Aggregate floors by tool name, keeping the highest declared version
/// (numeric ordering). Result is sorted by tool name; the `source` is the
/// script that declared the winning version.
pub fn aggregate_floors(floors: Vec<DeclaredFloor>) -> Vec<DeclaredFloor> {
    let mut by_tool: BTreeMap<String, DeclaredFloor> = BTreeMap::new();
    for floor in floors {
        match by_tool.get(&floor.tool) {
            Some(existing)
                if compare_versions(&floor.minimum_version, &existing.minimum_version)
                    != Ordering::Greater => {}
            _ => {
                by_tool.insert(floor.tool.clone(), floor);
            }
        }
    }
    by_tool.into_values().collect()
}

/// True if `value` is a dotted-numeric version like `2.0.0`. Pure-number
/// tokens (no dot) are rejected so `latest` and `2026` don't sneak through.
pub fn is_dotted_version(value: &str) -> bool {
    value.contains('.')
        && value.chars().all(|c| c.is_ascii_digit() || c == '.')
        && value.chars().next().is_some_and(|c| c.is_ascii_digit())
}

/// Pure classifier. Aggregates the highest declared floor per tool, then
/// compares each to the corresponding host version (`None` = host did not
/// resolve a version).
pub fn classify_provisioner(
    floors: Vec<DeclaredFloor>,
    host_versions: &BTreeMap<String, Option<String>>,
) -> Vec<ToolVerdict> {
    let mut verdicts = Vec::new();
    for declared in aggregate_floors(floors) {
        if !is_dotted_version(&declared.minimum_version) {
            verdicts.push(ToolVerdict::Unparseable {
                tool: declared.tool,
                raw_value: declared.minimum_version,
                source: declared.source,
            });
            continue;
        }
        let host_version = host_versions.get(&declared.tool).cloned().flatten();
        let Some(host_version) = host_version else {
            verdicts.push(ToolVerdict::HostVersionUnknown {
                tool: declared.tool,
                declared_minimum: declared.minimum_version,
                source: declared.source,
            });
            continue;
        };
        if compare_versions(&host_version, &declared.minimum_version) == Ordering::Less {
            verdicts.push(ToolVerdict::BelowFloor {
                tool: declared.tool,
                host_version,
                declared_minimum: declared.minimum_version,
                source: declared.source,
            });
        } else {
            verdicts.push(ToolVerdict::Ok {
                tool: declared.tool,
                host_version,
                declared_minimum: declared.minimum_version,
                source: declared.source,
            });
        }
    }
    verdicts
}

/// Extracts `MAJOR.MINOR[.PATCH]` from `Apple Swift version X.Y[.Z]`.
fn parse_swift_version(raw: &str) -> Option<String> {
    let idx = raw.find("Apple Swift version ")?;
    parse_version(&raw[idx + "Apple Swift version ".len()..])
}

// =========================================================================
// Runtime probe layer — #[cfg]-gated. The dev host (macOS) runs the full
// set; Linux shares the Homebrew model; a host without Homebrew yields
// benign skips.
// =========================================================================

#[cfg(target_os = "macos")]
fn brew_candidates() -> &'static [&'static str] {
    &["/opt/homebrew/bin/brew", "/usr/local/bin/brew"]
}

#[cfg(target_os = "linux")]
fn brew_candidates() -> &'static [&'static str] {
    &["/home/linuxbrew/.linuxbrew/bin/brew", "/usr/local/bin/brew"]
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
fn brew_candidates() -> &'static [&'static str] {
    // Windows (and other) hosts have no Homebrew; the bundle checks skip
    // benignly via the `NoHomebrew` verdict.
    &[]
}

#[cfg(unix)]
fn is_executable_file(path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|m| m.is_file() && (m.permissions().mode() & 0o111 != 0))
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &str) -> bool {
    std::path::Path::new(path).is_file()
}

fn path_exists(path: &str) -> bool {
    std::path::Path::new(path).exists()
}

fn resolve_brew_prefix() -> Option<String> {
    let brew = brew_candidates()
        .iter()
        .copied()
        .find(|p| is_executable_file(p))?;
    let out = std::process::Command::new(brew).arg("--prefix").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let prefix = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if prefix.is_empty() {
        None
    } else {
        Some(prefix)
    }
}

#[cfg(unix)]
fn which_binary(name: &str) -> Option<String> {
    let out = std::process::Command::new("/usr/bin/which").arg(name).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let path = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

#[cfg(not(unix))]
fn which_binary(name: &str) -> Option<String> {
    // `where` prints one match per line; take the first.
    let out = std::process::Command::new("where").arg(name).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let path = text.lines().next().unwrap_or("").trim().to_string();
    if path.is_empty() {
        None
    } else {
        Some(path)
    }
}

/// Run `<tool> --version` and return trimmed stdout (PATH-resolved by the OS).
fn probe_version(name: &str) -> Option<String> {
    let out = std::process::Command::new(name).arg("--version").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

fn probe_agent_slots(brew_prefix: Option<&str>) -> Vec<SlotProbe> {
    let Some(prefix) = brew_prefix else {
        return Vec::new();
    };
    [AgentSlot::Macos, AgentSlot::Windows, AgentSlot::Linux]
        .into_iter()
        .map(|slot| {
            let path = agent_expected_path(prefix, slot);
            let exists = path_exists(&path);
            SlotProbe {
                slot,
                is_executable: exists && is_executable_file(&path),
                expected_path: path,
                exists,
            }
        })
        .collect()
}

fn probe_script_files(brew_prefix: Option<&str>) -> Vec<FileProbe> {
    let Some(prefix) = brew_prefix else {
        return Vec::new();
    };
    let mut probes = Vec::new();
    for path in expected_script_paths(prefix) {
        let exists = path_exists(&path);
        probes.push(FileProbe {
            slot: ScriptSlot::Scripts,
            is_executable: exists && is_executable_file(&path),
            requires_executable: true,
            exists,
            path,
        });
    }
    for path in expected_helper_paths(prefix) {
        let exists = path_exists(&path);
        probes.push(FileProbe {
            slot: ScriptSlot::Helpers,
            is_executable: exists && is_executable_file(&path),
            requires_executable: false,
            exists,
            path,
        });
    }
    probes
}

fn gather_tool_statuses() -> Vec<ToolStatus> {
    KNOWN_TOOLS
        .iter()
        .map(|tool| {
            let path = which_binary(tool.name);
            let version = if path.is_none() {
                VersionVerdict::Ok { detected: None }
            } else {
                compare_tool_version(probe_version(tool.name).as_deref(), tool.minimum_version)
            };
            ToolStatus {
                name: tool.name,
                purpose: tool.purpose,
                install_hint: tool.install_hint,
                path,
                version,
            }
        })
        .collect()
}

/// Walk up from `binary_path` looking for a parent containing
/// `scripts/release-build.sh`. Dev builds resolve to the repo's copy; brew
/// installs return `None`.
fn locate_release_build_script(binary_path: &std::path::Path) -> Option<String> {
    let mut dir = binary_path
        .canonicalize()
        .unwrap_or_else(|_| binary_path.to_path_buf());
    dir.pop();
    loop {
        let candidate = dir.join("scripts").join("release-build.sh");
        if candidate.is_file() {
            return Some(candidate.display().to_string());
        }
        if !dir.pop() {
            return None;
        }
    }
}

fn list_shell_scripts(dir: &str) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "sh"))
        .map(|p| p.display().to_string())
        .collect();
    out.sort();
    out
}

/// Collect host versions for every tool referenced by `floors`. Tools covered
/// by the host-tool check reuse its result; extras are probed directly (with
/// the `swift` banner special-case mirrored from the Swift check).
fn collect_host_versions(
    floors: &[DeclaredFloor],
    tools: &[ToolStatus],
) -> BTreeMap<String, Option<String>> {
    let mut host_versions: BTreeMap<String, Option<String>> = BTreeMap::new();
    for status in tools {
        let detected = match &status.version {
            VersionVerdict::Ok { detected } => detected.clone(),
            VersionVerdict::BelowFloor { detected, .. } => Some(detected.clone()),
            VersionVerdict::Unparseable { .. } | VersionVerdict::ProbeFailed { .. } => None,
        };
        host_versions.insert(status.name.to_string(), detected);
    }
    let covered: std::collections::BTreeSet<&str> = KNOWN_TOOLS.iter().map(|t| t.name).collect();
    for floor in floors {
        if covered.contains(floor.tool.as_str()) || host_versions.contains_key(&floor.tool) {
            continue;
        }
        let raw = probe_version(&floor.tool);
        let parsed = raw.as_deref().and_then(|r| {
            if floor.tool == "swift" {
                parse_swift_version(r)
            } else {
                parse_version(r)
            }
        });
        host_versions.insert(floor.tool.clone(), parsed);
    }
    host_versions
}

fn gather_provisioner(brew_prefix: Option<&str>, tools: &[ToolStatus]) -> ProvisionerResult {
    let mut scripts: Vec<String> = Vec::new();
    if let Some(prefix) = brew_prefix {
        scripts.extend(list_shell_scripts(&format!("{prefix}/share/testanyware/scripts")));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(release_build) = locate_release_build_script(&exe) {
            scripts.push(release_build);
        }
    }
    if scripts.is_empty() {
        return ProvisionerResult { per_tool: Vec::new(), skipped: true };
    }
    let mut floors = Vec::new();
    for path in &scripts {
        if let Ok(content) = std::fs::read_to_string(path) {
            floors.extend(parse_sentinels(&content, path));
        }
    }
    let host_versions = collect_host_versions(&floors, tools);
    ProvisionerResult {
        per_tool: classify_provisioner(floors, &host_versions),
        skipped: false,
    }
}

// =========================================================================
// Report assembly, JSON + text rendering.
// =========================================================================

struct DoctorReport {
    install: InstallVerdict,
    running_binary: String,
    agents: AgentsVerdict,
    scripts: ScriptsVerdict,
    tools: Vec<ToolStatus>,
    provisioner: ProvisionerResult,
}

impl DoctorReport {
    fn gather() -> Self {
        let brew = resolve_brew_prefix();
        let tools = gather_tool_statuses();
        DoctorReport {
            install: classify_install_path(which_binary("testanyware").as_deref(), brew.as_deref()),
            running_binary: std::env::current_exe()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|_| "(unknown)".to_string()),
            agents: classify_bundled_agents(brew.as_deref(), &probe_agent_slots(brew.as_deref())),
            scripts: classify_bundled_scripts(brew.as_deref(), &probe_script_files(brew.as_deref())),
            provisioner: gather_provisioner(brew.as_deref(), &tools),
            tools,
        }
    }

    /// Overall health. Tool availability and script floors are advisory and
    /// do not flip this (matching the Swift `isOK` semantics).
    fn is_ok(&self) -> bool {
        self.install.is_ok() && self.agents.is_ok() && self.scripts.is_ok()
    }

    fn to_json(&self) -> Value {
        json!({
            "schema_version": SCHEMA_VERSION,
            "ok": self.is_ok(),
            "checks": {
                "install_path": self.install_json(),
                "bundled_agents": self.agents_json(),
                "bundled_scripts": self.scripts_json(),
                "host_tools": self.tools_json(),
                "script_tool_floors": self.provisioner_json(),
            },
        })
    }

    fn install_json(&self) -> Value {
        let status = if self.install.is_ok() { "pass" } else { "fail" };
        match &self.install {
            InstallVerdict::HomebrewInstall { path, brew_prefix } => json!({
                "status": status, "verdict": "homebrew_install",
                "running_binary": self.running_binary, "path": path, "brew_prefix": brew_prefix,
            }),
            InstallVerdict::Shadowed { path, brew_prefix } => json!({
                "status": status, "verdict": "shadowed",
                "running_binary": self.running_binary, "path": path, "brew_prefix": brew_prefix,
            }),
            InstallVerdict::NoHomebrew { path } => json!({
                "status": status, "verdict": "no_homebrew",
                "running_binary": self.running_binary, "path": path, "brew_prefix": Value::Null,
            }),
            InstallVerdict::NotOnPath { brew_prefix } => json!({
                "status": status, "verdict": "not_on_path",
                "running_binary": self.running_binary, "path": Value::Null, "brew_prefix": brew_prefix,
            }),
        }
    }

    fn agents_json(&self) -> Value {
        match &self.agents {
            AgentsVerdict::AllPresent { brew_prefix } => json!({
                "status": "pass", "brew_prefix": brew_prefix,
            }),
            AgentsVerdict::NoHomebrew => json!({
                "status": "skip", "reason": "homebrew_not_installed",
            }),
            AgentsVerdict::Missing { brew_prefix, issues } => json!({
                "status": "fail",
                "brew_prefix": brew_prefix,
                "issues": issues.iter().map(|(slot, issue)| json!({
                    "slot": slot.as_str(), "issue": issue.kind(), "path": issue.path(),
                })).collect::<Vec<_>>(),
            }),
        }
    }

    fn scripts_json(&self) -> Value {
        match &self.scripts {
            ScriptsVerdict::AllPresent { brew_prefix } => json!({
                "status": "pass", "brew_prefix": brew_prefix,
            }),
            ScriptsVerdict::NoHomebrew => json!({
                "status": "skip", "reason": "homebrew_not_installed",
            }),
            ScriptsVerdict::Missing { brew_prefix, issues } => json!({
                "status": "fail",
                "brew_prefix": brew_prefix,
                "issues": issues.iter().map(|(slot, issue)| json!({
                    "slot": slot.as_str(), "issue": issue.kind(), "path": issue.path(),
                })).collect::<Vec<_>>(),
            }),
        }
    }

    fn tools_json(&self) -> Value {
        let all_clean = self.tools.iter().all(ToolStatus::is_clean);
        json!({
            "status": if all_clean { "pass" } else { "warn" },
            "tools": self.tools.iter().map(|t| {
                let version = match &t.version {
                    VersionVerdict::Ok { detected } => json!({ "verdict": "ok", "detected": detected }),
                    VersionVerdict::BelowFloor { detected, minimum } =>
                        json!({ "verdict": "below_floor", "detected": detected, "minimum": minimum }),
                    VersionVerdict::Unparseable { raw, minimum } =>
                        json!({ "verdict": "unparseable", "raw": raw, "minimum": minimum }),
                    VersionVerdict::ProbeFailed { minimum } =>
                        json!({ "verdict": "probe_failed", "minimum": minimum }),
                };
                json!({
                    "name": t.name, "purpose": t.purpose, "install_hint": t.install_hint,
                    "path": t.path, "version": version,
                })
            }).collect::<Vec<_>>(),
        })
    }

    fn provisioner_json(&self) -> Value {
        if self.provisioner.skipped {
            return json!({ "status": "skip", "reason": "no_scripts_scanned", "floors": [] });
        }
        let clean = self
            .provisioner
            .per_tool
            .iter()
            .all(|v| matches!(v, ToolVerdict::Ok { .. }));
        json!({
            "status": if clean { "pass" } else { "warn" },
            "floors": self.provisioner.per_tool.iter().map(provisioner_verdict_json).collect::<Vec<_>>(),
        })
    }

    fn render_text(&self) -> String {
        let mut lines: Vec<String> = Vec::new();
        lines.push("testanyware doctor".to_string());
        lines.push(String::new());

        lines.push("Install path".to_string());
        lines.push(format!("  running binary:  {}", self.running_binary));
        self.render_install(&mut lines);
        lines.push(String::new());

        lines.push("Bundled agents".to_string());
        self.render_agents(&mut lines);
        lines.push(String::new());

        lines.push("Bundled scripts and helpers".to_string());
        self.render_scripts(&mut lines);
        lines.push(String::new());

        lines.push("Host tools".to_string());
        self.render_tools(&mut lines);
        lines.push(String::new());

        lines.push("Bundled-script tool floors".to_string());
        self.render_provisioner(&mut lines);

        let mut text = lines.join("\n");
        text.push('\n');
        text
    }

    fn render_install(&self, lines: &mut Vec<String>) {
        match &self.install {
            InstallVerdict::HomebrewInstall { path, brew_prefix } => {
                lines.push(format!("  on PATH:         {path}"));
                lines.push(format!("  Homebrew prefix: {brew_prefix}"));
                lines.push("  ✓ install path is under Homebrew prefix".to_string());
            }
            InstallVerdict::Shadowed { path, brew_prefix } => {
                lines.push(format!("  on PATH:         {path}"));
                lines.push(format!("  Homebrew prefix: {brew_prefix}"));
                lines.push(format!(
                    "  ✗ {path} shadows the Homebrew install at {brew_prefix}/bin/testanyware"
                ));
                lines.push(format!("    remediation: sudo rm {path}"));
                lines.push("                 (created during local dev; no longer needed)".to_string());
            }
            InstallVerdict::NoHomebrew { path } => {
                lines.push(format!("  on PATH:         {path}"));
                lines.push("  Homebrew prefix: not found".to_string());
                lines.push("  ! Homebrew is not installed; cannot verify install layout".to_string());
            }
            InstallVerdict::NotOnPath { brew_prefix } => {
                lines.push("  on PATH:         (not found)".to_string());
                match brew_prefix {
                    Some(prefix) => {
                        lines.push(format!("  Homebrew prefix: {prefix}"));
                        lines.push(format!(
                            "  ✗ testanyware is not on PATH; expected {prefix}/bin/testanyware"
                        ));
                        lines.push("    remediation: brew install Linkuistics/taps/testanyware".to_string());
                    }
                    None => {
                        lines.push("  Homebrew prefix: not found".to_string());
                        lines.push("  ✗ testanyware is not on PATH and Homebrew is not installed".to_string());
                    }
                }
            }
        }
    }

    fn render_agents(&self, lines: &mut Vec<String>) {
        match &self.agents {
            AgentsVerdict::AllPresent { brew_prefix } => {
                lines.push(format!("  bundle root:     {brew_prefix}/share/testanyware/agents"));
                lines.push("  ✓ macOS, Windows, and Linux agents all present".to_string());
            }
            AgentsVerdict::NoHomebrew => {
                lines.push("  bundle root:     (skipped — Homebrew not installed)".to_string());
            }
            AgentsVerdict::Missing { brew_prefix, issues } => {
                lines.push(format!("  bundle root:     {brew_prefix}/share/testanyware/agents"));
                for (slot, issue) in issues {
                    match issue {
                        PathIssue::Missing { path } => {
                            lines.push(format!("  ✗ {} agent missing: {path}", slot.as_str()))
                        }
                        PathIssue::NotExecutable { path } => {
                            lines.push(format!("  ✗ {} agent not executable: {path}", slot.as_str()))
                        }
                    }
                }
                lines.push("    remediation: brew reinstall Linkuistics/taps/testanyware".to_string());
            }
        }
    }

    fn render_scripts(&self, lines: &mut Vec<String>) {
        match &self.scripts {
            ScriptsVerdict::AllPresent { brew_prefix } => {
                lines.push(format!("  scripts root:    {brew_prefix}/share/testanyware/scripts"));
                lines.push(format!("  helpers root:    {brew_prefix}/share/testanyware/helpers"));
                lines.push("  ✓ all 8 provisioner scripts and 6 helpers present".to_string());
            }
            ScriptsVerdict::NoHomebrew => {
                lines.push("  scripts root:    (skipped — Homebrew not installed)".to_string());
            }
            ScriptsVerdict::Missing { brew_prefix, issues } => {
                lines.push(format!("  scripts root:    {brew_prefix}/share/testanyware/scripts"));
                lines.push(format!("  helpers root:    {brew_prefix}/share/testanyware/helpers"));
                for (slot, issue) in issues {
                    match issue {
                        PathIssue::Missing { path } => {
                            lines.push(format!("  ✗ {} file missing: {path}", slot.as_str()))
                        }
                        PathIssue::NotExecutable { path } => {
                            lines.push(format!("  ✗ {} file not executable: {path}", slot.as_str()))
                        }
                    }
                }
                lines.push("    remediation: brew reinstall Linkuistics/taps/testanyware".to_string());
            }
        }
    }

    fn render_tools(&self, lines: &mut Vec<String>) {
        for status in &self.tools {
            let Some(path) = &status.path else {
                lines.push(format!("  ! {} — not found ({})", status.name, status.purpose));
                lines.push(format!("    install hint: {}", status.install_hint));
                continue;
            };
            match &status.version {
                VersionVerdict::Ok { detected } => match detected {
                    Some(v) => lines.push(format!("  ✓ {} {v} — {path}", status.name)),
                    None => lines.push(format!("  ✓ {} — {path}", status.name)),
                },
                VersionVerdict::BelowFloor { detected, minimum } => {
                    lines.push(format!("  ! {} {detected} — {path}", status.name));
                    lines.push(format!(
                        "    below supported floor ({minimum}); upgrade with: {}",
                        status.install_hint
                    ));
                }
                VersionVerdict::Unparseable { raw, minimum } => {
                    lines.push(format!("  ! {} — {path}", status.name));
                    lines.push(format!(
                        "    could not parse --version output (expected ≥ {minimum}); raw: {raw}"
                    ));
                }
                VersionVerdict::ProbeFailed { minimum } => {
                    lines.push(format!("  ! {} — {path}", status.name));
                    lines.push(format!(
                        "    --version probe produced no output; cannot verify ≥ {minimum}"
                    ));
                }
            }
        }
    }

    fn render_provisioner(&self, lines: &mut Vec<String>) {
        if self.provisioner.skipped {
            lines.push("  scan:            (skipped — Homebrew or scripts directory not present)".to_string());
            return;
        }
        if self.provisioner.per_tool.is_empty() {
            lines.push("  ✓ no version sentinels declared in bundled scripts".to_string());
            return;
        }
        for verdict in &self.provisioner.per_tool {
            match verdict {
                ToolVerdict::Ok { tool, host_version, declared_minimum, source } => lines.push(
                    format!("  ✓ {tool} {host_version} ≥ {declared_minimum} ({})", pretty_source(source)),
                ),
                ToolVerdict::BelowFloor { tool, host_version, declared_minimum, source } => {
                    lines.push(format!(
                        "  ! {tool} {host_version} < {declared_minimum} declared by {}",
                        pretty_source(source)
                    ));
                    lines.push(format!(
                        "    bundled scripts may rely on features absent in {host_version}"
                    ));
                }
                ToolVerdict::HostVersionUnknown { tool, declared_minimum, source } => lines.push(
                    format!("  ! {tool} version unknown; {} declares ≥ {declared_minimum}", pretty_source(source)),
                ),
                ToolVerdict::Unparseable { tool, raw_value, source } => lines.push(format!(
                    "  ! {tool} sentinel in {} has unparseable version: {raw_value}",
                    pretty_source(source)
                )),
            }
        }
    }
}

fn provisioner_verdict_json(verdict: &ToolVerdict) -> Value {
    match verdict {
        ToolVerdict::Ok { tool, host_version, declared_minimum, source } => json!({
            "verdict": "ok", "tool": tool, "host_version": host_version,
            "declared_minimum": declared_minimum, "source": pretty_source(source),
        }),
        ToolVerdict::BelowFloor { tool, host_version, declared_minimum, source } => json!({
            "verdict": "below_floor", "tool": tool, "host_version": host_version,
            "declared_minimum": declared_minimum, "source": pretty_source(source),
        }),
        ToolVerdict::HostVersionUnknown { tool, declared_minimum, source } => json!({
            "verdict": "host_version_unknown", "tool": tool,
            "declared_minimum": declared_minimum, "source": pretty_source(source),
        }),
        ToolVerdict::Unparseable { tool, raw_value, source } => json!({
            "verdict": "unparseable", "tool": tool,
            "raw_value": raw_value, "source": pretty_source(source),
        }),
    }
}

/// Trim the brew-prefix portion of a script path so doctor output stays
/// readable: `<prefix>/share/testanyware/scripts/foo.sh` → `scripts/foo.sh`.
fn pretty_source(source: &str) -> String {
    const MARKER: &str = "/share/testanyware/";
    if let Some(idx) = source.find(MARKER) {
        source[idx + MARKER.len()..].to_string()
    } else {
        std::path::Path::new(source)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| source.to_string())
    }
}

/// `testanyware doctor` entry point. Read-only: no `--dry-run` (contract
/// §9.3). Emits the full report in both modes; exit 0 when healthy, 1 when a
/// blocking check (install/agents/scripts) fails.
pub fn run_doctor(mode: OutputMode) -> ! {
    let report = DoctorReport::gather();
    let ok = report.is_ok();
    match mode {
        OutputMode::Json => {
            let body = report.to_json();
            println!("{}", serde_json::to_string_pretty(&body).expect("serialize doctor report"));
        }
        OutputMode::Text => {
            print!("{}", report.render_text());
        }
    }
    std::process::exit(if ok { 0 } else { 1 });
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- version parsing & comparison ----

    #[test]
    fn parse_version_extracts_dotted_token() {
        assert_eq!(parse_version("2.32.1").as_deref(), Some("2.32.1"));
        assert_eq!(parse_version("QEMU emulator version 11.0.0").as_deref(), Some("11.0.0"));
        assert_eq!(parse_version("swtpm version 0.10.0").as_deref(), Some("0.10.0"));
        // A standalone year token has no dot and is skipped.
        assert_eq!(parse_version("built 2026 edition").as_deref(), None);
        assert_eq!(parse_version("no version here").as_deref(), None);
    }

    #[test]
    fn compare_versions_is_numeric_not_lexicographic() {
        assert_eq!(compare_versions("2.32", "2.5"), Ordering::Greater);
        assert_eq!(compare_versions("2.0", "2.0.0"), Ordering::Equal);
        assert_eq!(compare_versions("7.9.9", "8.0.0"), Ordering::Less);
    }

    // ---- install path ----

    #[test]
    fn install_homebrew_vs_shadowed_uses_trailing_slash_guard() {
        assert!(matches!(
            classify_install_path(Some("/opt/homebrew/bin/testanyware"), Some("/opt/homebrew")),
            InstallVerdict::HomebrewInstall { .. }
        ));
        // The trailing-slash guard stops a sibling prefix from matching.
        assert!(matches!(
            classify_install_path(Some("/opt/homebrew-evil/bin/testanyware"), Some("/opt/homebrew")),
            InstallVerdict::Shadowed { .. }
        ));
        assert!(matches!(
            classify_install_path(Some("/usr/local/bin/testanyware"), Some("/opt/homebrew")),
            InstallVerdict::Shadowed { .. }
        ));
    }

    #[test]
    fn install_no_homebrew_and_not_on_path() {
        assert!(matches!(
            classify_install_path(Some("/usr/local/bin/testanyware"), None),
            InstallVerdict::NoHomebrew { .. }
        ));
        assert!(classify_install_path(Some("/usr/local/bin/testanyware"), None).is_ok());
        let v = classify_install_path(None, Some("/opt/homebrew"));
        assert!(matches!(v, InstallVerdict::NotOnPath { brew_prefix: Some(_) }));
        assert!(!v.is_ok());
        assert!(!classify_install_path(None, None).is_ok());
    }

    // ---- bundled agents ----

    #[test]
    fn agents_no_homebrew_is_a_benign_skip() {
        let v = classify_bundled_agents(None, &[]);
        assert_eq!(v, AgentsVerdict::NoHomebrew);
        assert!(v.is_ok());
    }

    #[test]
    fn agents_all_present_when_every_slot_ok() {
        let prefix = "/opt/homebrew";
        let probes: Vec<SlotProbe> = [AgentSlot::Macos, AgentSlot::Windows, AgentSlot::Linux]
            .into_iter()
            .map(|slot| SlotProbe {
                slot,
                expected_path: agent_expected_path(prefix, slot),
                exists: true,
                // Only macOS needs the executable bit.
                is_executable: slot == AgentSlot::Macos,
            })
            .collect();
        assert!(classify_bundled_agents(Some(prefix), &probes).is_ok());
    }

    #[test]
    fn agents_flag_missing_and_non_executable_macos() {
        let prefix = "/opt/homebrew";
        let probes = vec![
            SlotProbe {
                slot: AgentSlot::Macos,
                expected_path: agent_expected_path(prefix, AgentSlot::Macos),
                exists: true,
                is_executable: false, // present but not executable → issue
            },
            SlotProbe {
                slot: AgentSlot::Windows,
                expected_path: agent_expected_path(prefix, AgentSlot::Windows),
                exists: false, // missing → issue
                is_executable: false,
            },
            SlotProbe {
                slot: AgentSlot::Linux,
                expected_path: agent_expected_path(prefix, AgentSlot::Linux),
                exists: true,
                is_executable: false, // linux exec bit not required → no issue
            },
        ];
        let v = classify_bundled_agents(Some(prefix), &probes);
        match v {
            AgentsVerdict::Missing { issues, .. } => {
                assert_eq!(issues.len(), 2);
                assert!(matches!(issues[0], (AgentSlot::Macos, PathIssue::NotExecutable { .. })));
                assert!(matches!(issues[1], (AgentSlot::Windows, PathIssue::Missing { .. })));
            }
            other => panic!("expected Missing, got {other:?}"),
        }
    }

    // ---- bundled scripts ----

    #[test]
    fn scripts_require_executable_only_where_flagged() {
        let prefix = "/opt/homebrew";
        let mut probes: Vec<FileProbe> = expected_script_paths(prefix)
            .into_iter()
            .map(|path| FileProbe { slot: ScriptSlot::Scripts, path, exists: true, is_executable: true, requires_executable: true })
            .collect();
        probes.extend(expected_helper_paths(prefix).into_iter().map(|path| FileProbe {
            slot: ScriptSlot::Helpers,
            path,
            exists: true,
            is_executable: false, // helpers do not require the bit
            requires_executable: false,
        }));
        assert!(classify_bundled_scripts(Some(prefix), &probes).is_ok());
    }

    #[test]
    fn scripts_flag_non_executable_script() {
        let prefix = "/opt/homebrew";
        let probes = vec![FileProbe {
            slot: ScriptSlot::Scripts,
            path: format!("{prefix}/share/testanyware/scripts/vm-create-golden-linux.sh"),
            exists: true,
            is_executable: false,
            requires_executable: true,
        }];
        let v = classify_bundled_scripts(Some(prefix), &probes);
        assert!(matches!(v, ScriptsVerdict::Missing { .. }));
        assert!(!v.is_ok());
    }

    #[test]
    fn scripts_no_homebrew_skip() {
        assert_eq!(classify_bundled_scripts(None, &[]), ScriptsVerdict::NoHomebrew);
        assert!(classify_bundled_scripts(None, &[]).is_ok());
    }

    // ---- tool availability ----

    #[test]
    fn tool_version_floor_comparisons() {
        assert_eq!(
            compare_tool_version(Some("tart 2.1.0"), Some("2.0.0")),
            VersionVerdict::Ok { detected: Some("2.1.0".to_string()) }
        );
        assert_eq!(
            compare_tool_version(Some("tart 1.9.0"), Some("2.0.0")),
            VersionVerdict::BelowFloor { detected: "1.9.0".to_string(), minimum: "2.0.0".to_string() }
        );
        assert_eq!(
            compare_tool_version(Some("no digits here"), Some("2.0.0")),
            VersionVerdict::Unparseable { raw: "no digits here".to_string(), minimum: "2.0.0".to_string() }
        );
        assert_eq!(
            compare_tool_version(None, Some("8.0.0")),
            VersionVerdict::ProbeFailed { minimum: "8.0.0".to_string() }
        );
        // Presence-only tool (no floor) is always Ok.
        assert_eq!(
            compare_tool_version(Some("swtpm version 0.8.0"), None),
            VersionVerdict::Ok { detected: Some("0.8.0".to_string()) }
        );
    }

    // ---- provisioner sentinels ----

    #[test]
    fn parse_sentinels_reads_well_formed_and_malformed() {
        let content = "#!/bin/sh\n# testanyware-min-tool: tart 2.5.0\necho hi\n# testanyware-min-tool: swift latest\n# testanyware-min-tool: lonely\n";
        let floors = parse_sentinels(content, "scripts/vm-start.sh");
        assert_eq!(floors.len(), 3);
        assert_eq!(floors[0], DeclaredFloor { tool: "tart".into(), minimum_version: "2.5.0".into(), source: "scripts/vm-start.sh".into() });
        assert_eq!(floors[1].tool, "swift");
        assert_eq!(floors[1].minimum_version, "latest");
        assert_eq!(floors[2].tool, "lonely");
        assert_eq!(floors[2].minimum_version, "");
    }

    #[test]
    fn aggregate_floors_keeps_highest_per_tool() {
        let floors = vec![
            DeclaredFloor { tool: "tart".into(), minimum_version: "2.5.0".into(), source: "a".into() },
            DeclaredFloor { tool: "tart".into(), minimum_version: "2.32.0".into(), source: "b".into() },
        ];
        let agg = aggregate_floors(floors);
        assert_eq!(agg.len(), 1);
        assert_eq!(agg[0].minimum_version, "2.32.0");
        assert_eq!(agg[0].source, "b");
    }

    #[test]
    fn is_dotted_version_rejects_bare_numbers_and_words() {
        assert!(is_dotted_version("2.0.0"));
        assert!(is_dotted_version("2.5"));
        assert!(!is_dotted_version("2026"));
        assert!(!is_dotted_version("latest"));
    }

    #[test]
    fn classify_provisioner_covers_each_verdict() {
        let floors = vec![
            DeclaredFloor { tool: "tart".into(), minimum_version: "2.0.0".into(), source: "scripts/a.sh".into() },
            DeclaredFloor { tool: "qemu".into(), minimum_version: "9.0.0".into(), source: "scripts/b.sh".into() },
            DeclaredFloor { tool: "swtpm".into(), minimum_version: "0.8.0".into(), source: "scripts/c.sh".into() },
            DeclaredFloor { tool: "ghost".into(), minimum_version: "1.0.0".into(), source: "scripts/d.sh".into() },
            DeclaredFloor { tool: "typo".into(), minimum_version: "latest".into(), source: "scripts/e.sh".into() },
        ];
        let mut host: BTreeMap<String, Option<String>> = BTreeMap::new();
        host.insert("tart".into(), Some("2.5.0".into())); // ok
        host.insert("qemu".into(), Some("8.0.0".into())); // below floor
        host.insert("swtpm".into(), None); // unknown
        // "ghost" absent from map → unknown; "typo" → unparseable.
        let verdicts = classify_provisioner(floors, &host);
        // aggregate_floors sorts by tool name: ghost, qemu, swtpm, tart, typo.
        assert!(matches!(verdicts[0], ToolVerdict::HostVersionUnknown { .. }));
        assert!(matches!(verdicts[1], ToolVerdict::BelowFloor { .. }));
        assert!(matches!(verdicts[2], ToolVerdict::HostVersionUnknown { .. }));
        assert!(matches!(verdicts[3], ToolVerdict::Ok { .. }));
        assert!(matches!(verdicts[4], ToolVerdict::Unparseable { .. }));
    }

    // ---- pretty source ----

    #[test]
    fn pretty_source_trims_brew_prefix_or_falls_back_to_basename() {
        assert_eq!(
            pretty_source("/opt/homebrew/share/testanyware/scripts/vm-start.sh"),
            "scripts/vm-start.sh"
        );
        assert_eq!(pretty_source("/repo/scripts/release-build.sh"), "release-build.sh");
    }

    // ---- host check set ----

    #[test]
    fn brew_candidates_match_host_os() {
        let candidates = brew_candidates();
        #[cfg(target_os = "macos")]
        assert!(candidates.contains(&"/opt/homebrew/bin/brew"));
        #[cfg(target_os = "linux")]
        assert!(candidates.contains(&"/home/linuxbrew/.linuxbrew/bin/brew"));
        #[cfg(not(any(target_os = "macos", target_os = "linux")))]
        assert!(candidates.is_empty());
    }
}
