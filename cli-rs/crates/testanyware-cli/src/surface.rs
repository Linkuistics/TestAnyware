//! Canonical command surface, alias tables, and error-code catalogue.
//!
//! This module is the single source of truth for the data that the
//! `capabilities`, `schema`, and `llm-instructions` discoverability
//! commands emit, *and* for the `cli-contract.rs` integration test. The
//! contract document (`docs/architecture/cli-design-contract.md`) §1
//! defines the canonical surface; this module encodes it once for both
//! consumers.

/// One canonical subcommand from §1.
///
/// `path` is the noun-first canonical invocation (space-separated tokens).
/// `mutating` flags whether `--dry-run` is required by §9.3.
/// `data_producing` flags whether `--json` is required by §3.1.
/// `schema_id` names the schema file under
/// `docs/reference/cli-schemas/<schema-id>.json` (§3.3); `None` for
/// non-data commands.
pub struct CommandSpec {
    pub path: &'static [&'static str],
    pub mutating: bool,
    pub data_producing: bool,
    pub schema_id: Option<&'static str>,
}

/// Every canonical command listed in contract §1.
pub const CANONICAL_COMMANDS: &[CommandSpec] = &[
    // vm
    CommandSpec { path: &["vm", "start"],   mutating: true,  data_producing: true, schema_id: Some("vm-start") },
    CommandSpec { path: &["vm", "stop"],    mutating: true,  data_producing: true, schema_id: Some("vm-stop") },
    CommandSpec { path: &["vm", "list"],    mutating: false, data_producing: true, schema_id: Some("vm-list") },
    CommandSpec { path: &["vm", "delete"],  mutating: true,  data_producing: true, schema_id: Some("vm-delete") },

    // agent — query
    CommandSpec { path: &["agent", "health"],   mutating: false, data_producing: true, schema_id: Some("agent-health") },
    CommandSpec { path: &["agent", "snapshot"], mutating: false, data_producing: true, schema_id: Some("agent-snapshot") },
    CommandSpec { path: &["agent", "inspect"],  mutating: false, data_producing: true, schema_id: Some("agent-inspect") },
    CommandSpec { path: &["agent", "windows"],  mutating: false, data_producing: true, schema_id: Some("agent-windows") },
    CommandSpec { path: &["agent", "wait"],     mutating: false, data_producing: true, schema_id: Some("agent-action") },

    // agent — action
    CommandSpec { path: &["agent", "press"],     mutating: true, data_producing: true, schema_id: Some("agent-action") },
    CommandSpec { path: &["agent", "set-value"], mutating: true, data_producing: true, schema_id: Some("agent-action") },
    CommandSpec { path: &["agent", "focus"],     mutating: true, data_producing: true, schema_id: Some("agent-action") },
    CommandSpec { path: &["agent", "show-menu"], mutating: true, data_producing: true, schema_id: Some("agent-action") },

    // agent — window-*
    CommandSpec { path: &["agent", "window-focus"],    mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-resize"],   mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-move"],     mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-close"],    mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-minimize"], mutating: true, data_producing: true, schema_id: Some("agent-window-action") },

    // input
    CommandSpec { path: &["input", "key"],        mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "key-down"],   mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "key-up"],     mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "type"],       mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "click"],      mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "mouse-down"], mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "mouse-up"],   mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "move"],       mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "scroll"],     mutating: true, data_producing: true, schema_id: Some("input-action") },
    CommandSpec { path: &["input", "drag"],       mutating: true, data_producing: true, schema_id: Some("input-action") },

    // screen
    CommandSpec { path: &["screen", "capture"],   mutating: false, data_producing: true, schema_id: Some("screen-capture") },
    CommandSpec { path: &["screen", "record"],    mutating: true,  data_producing: true, schema_id: Some("screen-record") },
    CommandSpec { path: &["screen", "size"],      mutating: false, data_producing: true, schema_id: Some("screen-size") },
    CommandSpec { path: &["screen", "find-text"], mutating: false, data_producing: true, schema_id: Some("screen-find-text") },

    // file
    CommandSpec { path: &["file", "upload"],   mutating: true, data_producing: true, schema_id: Some("file-upload") },
    CommandSpec { path: &["file", "download"], mutating: true, data_producing: true, schema_id: Some("file-download") },
    CommandSpec { path: &["file", "exec"],     mutating: true, data_producing: true, schema_id: Some("file-exec") },

    // viewer (ADR-0005): long-lived + interactive embedded viewer. Neither
    // mutating nor data-producing — exempt from --json/--dry-run, no schema.
    CommandSpec { path: &["viewer"], mutating: false, data_producing: false, schema_id: None },

    // top-level utilities
    CommandSpec { path: &["doctor"],           mutating: false, data_producing: true, schema_id: Some("doctor") },
    CommandSpec { path: &["capabilities"],     mutating: false, data_producing: true, schema_id: Some("capabilities") },
    CommandSpec { path: &["schema"],           mutating: false, data_producing: true, schema_id: None },
    CommandSpec { path: &["llm-instructions"], mutating: false, data_producing: false, schema_id: None },
];

/// Top-level noun groups exposed in `--help` (the `subcommands` key in
/// `capabilities --json`). Order matches the contract §1 surface table.
pub const TOP_LEVEL_GROUPS: &[&str] = &[
    "vm",
    "agent",
    "input",
    "screen",
    "file",
    "viewer",
    "doctor",
    "capabilities",
    "schema",
    "llm-instructions",
];

/// Verb-first aliases retained for muscle memory (§1).
pub const VERB_FIRST_ALIASES: &[(&str, &[&str])] = &[
    ("screenshot",  &["screen", "capture"]),
    ("record",      &["screen", "record"]),
    ("screen-size", &["screen", "size"]),
    ("find-text",   &["screen", "find-text"]),
    ("upload",      &["file", "upload"]),
    ("download",    &["file", "download"]),
    ("exec",        &["file", "exec"]),
];

/// Synonym aliases required by §1.
pub const SYNONYM_ALIASES: &[(&[&str], &[&str])] = &[
    (&["vm", "list"],      &["vm", "ls"]),
    (&["vm", "delete"],    &["vm", "rm"]),
    (&["vm", "delete"],    &["vm", "remove"]),
    (&["agent", "inspect"], &["agent", "show"]),
];

/// Stable error-code strings catalogued in contract §4 plus the
/// `SCHEMA_NOT_FOUND` code introduced by §8.2 for the `schema` command.
///
/// Order: §4.1 (auth/connection), §4.2 (vm), §4.3 (vnc), §4.4 (record),
/// §4.5 (agent), §4.6 (generic), §8.2 (schema).
pub const ERROR_CODES: &[&str] = &[
    // §4.1
    "AUTH_REQUIRED",
    "CONNECTION_REFUSED",
    "CONNECTION_TIMEOUT",
    "CONNECTION_DROPPED",
    "INVALID_ENDPOINT",
    "NO_CONNECTION_SPECIFIED",
    "INVALID_PLATFORM",
    // §4.2
    "VM_NOT_FOUND",
    "VM_BOOT_TIMEOUT",
    "VM_STOP_FAILED",
    "VM_BACKEND_UNSUPPORTED",
    "GOLDEN_NOT_FOUND",
    "GOLDEN_IN_USE",
    "TART_FAILED",
    "QEMU_FAILED",
    "KVM_PERMISSION_DENIED",
    "SWTPM_MISSING",
    "UEFI_NOT_FOUND",
    "SPAWN_FAILED",
    // §4.3
    "VNC_NOT_CONFIGURED",
    "VNC_FRAMEBUFFER_NOT_READY",
    "VNC_CAPTURE_FAILED",
    "VNC_ENCODING_FAILED",
    "VNC_PIXEL_MISMATCH",
    "VNC_DIMENSIONS_ZERO",
    // §4.4
    "RECORD_ALREADY_ACTIVE",
    "RECORD_NOT_ACTIVE",
    "RECORD_BUFFER_UNAVAILABLE",
    "RECORD_BUFFER_CREATE_FAILED",
    // §4.5
    "ELEMENT_NOT_FOUND",
    "ELEMENT_AMBIGUOUS",
    "WINDOW_NOT_FOUND",
    "ACTION_UNSUPPORTED",
    "EXEC_FAILED",
    "UPLOAD_FAILED",
    "DOWNLOAD_FAILED",
    "AGENT_ERROR_UNKNOWN",
    // §4.6
    "USAGE_ERROR",
    "IO_ERROR",
    "OCR_UNAVAILABLE",
    "OCR_CHILD_CRASHED",
    "OCR_TIMEOUT",
    "UNKNOWN_KEY",
    "UNKNOWN_BUTTON",
    "INTERNAL",
    // §4.7 (discoverability / not-found)
    "TEXT_NOT_FOUND",
    // §8.2
    "SCHEMA_NOT_FOUND",
];
