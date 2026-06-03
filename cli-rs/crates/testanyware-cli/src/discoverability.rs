//! Implementations for the §8 discoverability commands: `capabilities`,
//! `schema`, and `llm-instructions`.
//!
//! Each schema file lives at `docs/reference/cli-schemas/<id>.json` and is
//! embedded at build time via `include_str!` per contract §8.2. The path
//! is relative to *this* source file: from `src/discoverability.rs`,
//! `../../../../docs/reference/cli-schemas/<id>.json` reaches the repo
//! root and then into the schema directory.

use serde_json::{json, Value};

use crate::surface::{
    CommandSpec, CANONICAL_COMMANDS, ERROR_CODES, SYNONYM_ALIASES, TOP_LEVEL_GROUPS,
    VERB_FIRST_ALIASES,
};

const SCHEMA_VERSION: &str = "1.0";

/// Files embedded from `docs/reference/cli-schemas/`. Lookup keyed by
/// schema id (matches §3.1's table).
const EMBEDDED_SCHEMAS: &[(&str, &str)] = &[
    ("agent-action",        include_str!("../../../../docs/reference/cli-schemas/agent-action.json")),
    ("agent-health",        include_str!("../../../../docs/reference/cli-schemas/agent-health.json")),
    ("agent-inspect",       include_str!("../../../../docs/reference/cli-schemas/agent-inspect.json")),
    ("agent-snapshot",      include_str!("../../../../docs/reference/cli-schemas/agent-snapshot.json")),
    ("agent-window-action", include_str!("../../../../docs/reference/cli-schemas/agent-window-action.json")),
    ("agent-windows",       include_str!("../../../../docs/reference/cli-schemas/agent-windows.json")),
    ("capabilities",        include_str!("../../../../docs/reference/cli-schemas/capabilities.json")),
    ("doctor",              include_str!("../../../../docs/reference/cli-schemas/doctor.json")),
    ("file-download",       include_str!("../../../../docs/reference/cli-schemas/file-download.json")),
    ("file-exec",           include_str!("../../../../docs/reference/cli-schemas/file-exec.json")),
    ("file-upload",         include_str!("../../../../docs/reference/cli-schemas/file-upload.json")),
    ("input-action",        include_str!("../../../../docs/reference/cli-schemas/input-action.json")),
    ("screen-capture",      include_str!("../../../../docs/reference/cli-schemas/screen-capture.json")),
    ("screen-find-text",    include_str!("../../../../docs/reference/cli-schemas/screen-find-text.json")),
    ("screen-record",       include_str!("../../../../docs/reference/cli-schemas/screen-record.json")),
    ("screen-size",         include_str!("../../../../docs/reference/cli-schemas/screen-size.json")),
    ("vm-create-golden",    include_str!("../../../../docs/reference/cli-schemas/vm-create-golden.json")),
    ("vm-delete",           include_str!("../../../../docs/reference/cli-schemas/vm-delete.json")),
    ("vm-list",             include_str!("../../../../docs/reference/cli-schemas/vm-list.json")),
    ("vm-start",            include_str!("../../../../docs/reference/cli-schemas/vm-start.json")),
    ("vm-stop",             include_str!("../../../../docs/reference/cli-schemas/vm-stop.json")),
];

/// Find the canonical command spec whose `path` matches `tokens`.
fn find_command(tokens: &[String]) -> Option<&'static CommandSpec> {
    CANONICAL_COMMANDS
        .iter()
        .find(|spec| spec.path.len() == tokens.len()
            && spec.path.iter().zip(tokens.iter()).all(|(a, b)| *a == b))
}

fn embedded_schema(id: &str) -> Option<&'static str> {
    EMBEDDED_SCHEMAS.iter().find_map(|(name, body)| (*name == id).then_some(*body))
}

// -------------------------------------------------------------------------
// capabilities
// -------------------------------------------------------------------------

/// Run `testanyware capabilities`. Always emits JSON on stdout; the
/// `--json` flag is accepted as a no-op (§8.1: this is a machine-only
/// command).
pub fn run_capabilities() -> ! {
    let aliases: Value = {
        let mut obj = serde_json::Map::new();
        for (alias, canonical) in VERB_FIRST_ALIASES {
            obj.insert((*alias).to_string(), Value::String(canonical.join(" ")));
        }
        for (canonical, alias) in SYNONYM_ALIASES {
            obj.insert(alias.join(" "), Value::String(canonical.join(" ")));
        }
        Value::Object(obj)
    };

    let body = json!({
        "schema_version": SCHEMA_VERSION,
        "ok": true,
        "version": env!("CARGO_PKG_VERSION"),
        // git_revision is populated by the release pipeline (mirrors the
        // Swift CLI's release-build.sh / generate-version.sh flow). At
        // dev-build time it is "unknown".
        "git_revision": option_env!("TESTANYWARE_GIT_REVISION").unwrap_or("unknown"),
        "subcommands": TOP_LEVEL_GROUPS,
        "aliases": aliases,
        "output_formats": ["text", "json", "jsonl"],
        "features": {
            "idempotency_keys": false,
            "streaming": true,
            "dry_run": true,
            "schema_command": true,
        },
        "platforms": {
            "host": ["macos", "linux"],
            "guest": ["macos", "linux", "windows"],
        },
        "error_codes": ERROR_CODES,
        // §9.5 (hidden state): behaviour-influencing env vars are surfaced
        // here and documented in docs/reference/env-vars.md. `internal:
        // true` marks diagnostic/test-only seams that are NOT part of the
        // stable contract surface.
        "env_vars": [
            { "name": "TESTANYWARE_VM_ID",  "internal": false,
              "description": "VM instance id; resolves the per-VM connection spec." },
            { "name": "TESTANYWARE_VNC",     "internal": false,
              "description": "VNC endpoint host[:port] (ad-hoc, no spec file)." },
            { "name": "TESTANYWARE_AGENT",   "internal": false,
              "description": "Agent HTTP endpoint host[:port]." },
            { "name": "TESTANYWARE_PLATFORM", "internal": false,
              "description": "Target platform: macos, linux, windows." },
            { "name": "TESTANYWARE_RFB_ENCODING", "internal": true,
              "description": "Force a single primary RFB encoding (zrle|tight|raw) so the live-VM gate can exercise each decoder; not part of the stable contract." },
        ],
    });

    println!("{}", serde_json::to_string_pretty(&body).expect("serialize capabilities"));
    std::process::exit(0);
}

// -------------------------------------------------------------------------
// schema
// -------------------------------------------------------------------------

/// Run `testanyware schema <command...>`. Emits the JSON Schema for the
/// command's `--json` output, or an error envelope on miss.
pub fn run_schema(tokens: &[String]) -> ! {
    if tokens.is_empty() {
        eprintln!("USAGE: testanyware schema <command>");
        eprintln!("       e.g. `testanyware schema vm list`");
        std::process::exit(2);
    }

    let Some(spec) = find_command(tokens) else {
        emit_schema_not_found(tokens, "no canonical command matches the path");
    };

    let Some(schema_id) = spec.schema_id else {
        emit_schema_not_found(tokens, "command has no declared schema");
    };

    let Some(body) = embedded_schema(schema_id) else {
        emit_schema_not_found(tokens, &format!("schema id `{schema_id}` not embedded in this binary"));
    };

    print!("{body}");
    if !body.ends_with('\n') {
        println!();
    }
    std::process::exit(0);
}

fn emit_schema_not_found(tokens: &[String], reason: &str) -> ! {
    let envelope = json!({
        "schema_version": SCHEMA_VERSION,
        "ok": false,
        "code": "SCHEMA_NOT_FOUND",
        "message": format!("No schema for `testanyware {}`", tokens.join(" ")),
        "details": {
            "command": tokens,
            "reason": reason,
        },
    });
    println!("{}", serde_json::to_string_pretty(&envelope).expect("serialize SCHEMA_NOT_FOUND"));
    std::process::exit(3);
}

// -------------------------------------------------------------------------
// llm-instructions
// -------------------------------------------------------------------------

/// The full LLM usage guide, embedded at build time from the repo-root
/// `LLM_INSTRUCTIONS.md` — the single source of truth (§8.3). The
/// `../../../../` path climbs src → testanyware-cli → crates → cli-rs →
/// repo root, matching the schema includes above.
const LLM_INSTRUCTIONS: &str = include_str!("../../../../LLM_INSTRUCTIONS.md");

/// Run `testanyware llm-instructions`. Plain text on stdout.
pub fn run_llm_instructions() -> ! {
    print!("{LLM_INSTRUCTIONS}");
    if !LLM_INSTRUCTIONS.ends_with('\n') {
        println!();
    }
    std::process::exit(0);
}
