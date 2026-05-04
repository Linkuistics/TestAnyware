//! cli-contract.rs — port-time CI gate for the Rust `testanyware` port.
//!
//! Walks every public subcommand and asserts the contract documented in
//! `docs/architecture/cli-design-contract.md`. This file is the gate the
//! contract document references in §11 ("Acceptance for downstream port
//! tasks"): each per-command port task fills in the per-command pieces of
//! the assertions defined here.
//!
//! ## Why a skeleton?
//!
//! Most contract clauses cannot be enforced until the corresponding
//! command is actually ported (today every subcommand exits with status 2
//! and the message `not yet implemented in the Rust port`). The skeleton
//! committed here:
//!
//!   1. Encodes the canonical command surface from §1 in one place so
//!      every port task validates against the same target shape.
//!   2. Provides the assertion helpers (help-section presence, JSON
//!      schema validation, dry-run acceptance, error-code mapping) so
//!      port tasks reuse them rather than re-inventing.
//!   3. Marks the per-clause checks `#[ignore]` with a `todo!()` body so
//!      `cargo test -- --ignored` panics with a clear pointer to the
//!      contract section that still needs an assertion. Each port task
//!      replaces the `todo!()` body with the real check for the command
//!      it adds.
//!
//! ## Section map
//!
//! Sections referenced below map to the contract document:
//!
//!   §1  command surface (noun-first canonical + verb-first aliases)
//!   §3  JSON schema policy
//!   §4  error codes
//!   §5  exit codes
//!   §6  identifier round-trip
//!   §7  help-text template
//!   §8  discoverability commands (`capabilities`, `schema`, `llm-instructions`)
//!   §9  behaviour invariants (TTY, dry-run, list limits)

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Path to the binary under test, set by Cargo for integration tests of a
/// crate that declares a `[[bin]]` target.
const BIN: &str = env!("CARGO_BIN_EXE_testanyware");

// ---------------------------------------------------------------------------
// Canonical command surface (§1)
// ---------------------------------------------------------------------------

/// One canonical subcommand from §1.
///
/// `path` is the noun-first canonical invocation (space-separated tokens).
/// `mutating` flags whether `--dry-run` is required by §9.3.
/// `data_producing` flags whether `--json` is required by §3.1.
/// `schema_id` names the schema file under
/// `docs/reference/cli-schemas/<schema-id>.json` (§3.3); `None` for
/// non-data commands.
#[allow(dead_code)] // fields are read by ignored tests once they are filled in
struct CommandSpec {
    path: &'static [&'static str],
    mutating: bool,
    data_producing: bool,
    schema_id: Option<&'static str>,
}

#[allow(dead_code)] // read by ignored tests once they are filled in
const CANONICAL_COMMANDS: &[CommandSpec] = &[
    // vm (§1)
    CommandSpec { path: &["vm", "start"],   mutating: true,  data_producing: true, schema_id: Some("vm-start") },
    CommandSpec { path: &["vm", "stop"],    mutating: true,  data_producing: true, schema_id: Some("vm-stop") },
    CommandSpec { path: &["vm", "list"],    mutating: false, data_producing: true, schema_id: Some("vm-list") },
    CommandSpec { path: &["vm", "delete"],  mutating: true,  data_producing: true, schema_id: Some("vm-delete") },

    // agent — query (§1)
    CommandSpec { path: &["agent", "health"],   mutating: false, data_producing: true, schema_id: Some("agent-health") },
    CommandSpec { path: &["agent", "snapshot"], mutating: false, data_producing: true, schema_id: Some("agent-snapshot") },
    CommandSpec { path: &["agent", "inspect"],  mutating: false, data_producing: true, schema_id: Some("agent-inspect") },
    CommandSpec { path: &["agent", "windows"],  mutating: false, data_producing: true, schema_id: Some("agent-windows") },
    CommandSpec { path: &["agent", "wait"],     mutating: false, data_producing: true, schema_id: Some("agent-action") },

    // agent — action (§1, §9.2)
    CommandSpec { path: &["agent", "press"],     mutating: true, data_producing: true, schema_id: Some("agent-action") },
    CommandSpec { path: &["agent", "set-value"], mutating: true, data_producing: true, schema_id: Some("agent-action") },
    CommandSpec { path: &["agent", "focus"],     mutating: true, data_producing: true, schema_id: Some("agent-action") },
    CommandSpec { path: &["agent", "show-menu"], mutating: true, data_producing: true, schema_id: Some("agent-action") },

    // agent — window-* (§1, §9.2)
    CommandSpec { path: &["agent", "window-focus"],    mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-resize"],   mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-move"],     mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-close"],    mutating: true, data_producing: true, schema_id: Some("agent-window-action") },
    CommandSpec { path: &["agent", "window-minimize"], mutating: true, data_producing: true, schema_id: Some("agent-window-action") },

    // input (§1, §9.2 — every input is mutating and not retry-safe)
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

    // screen (§1)
    CommandSpec { path: &["screen", "capture"],   mutating: false, data_producing: true, schema_id: Some("screen-capture") },
    CommandSpec { path: &["screen", "record"],    mutating: true,  data_producing: true, schema_id: Some("screen-record") },
    CommandSpec { path: &["screen", "size"],      mutating: false, data_producing: true, schema_id: Some("screen-size") },
    CommandSpec { path: &["screen", "find-text"], mutating: false, data_producing: true, schema_id: Some("screen-find-text") },

    // file (§1)
    CommandSpec { path: &["file", "upload"],   mutating: true, data_producing: true, schema_id: Some("file-upload") },
    CommandSpec { path: &["file", "download"], mutating: true, data_producing: true, schema_id: Some("file-download") },
    CommandSpec { path: &["file", "exec"],     mutating: true, data_producing: true, schema_id: Some("file-exec") },

    // top-level utilities (§8)
    CommandSpec { path: &["doctor"],           mutating: false, data_producing: true, schema_id: Some("doctor") },
    CommandSpec { path: &["capabilities"],     mutating: false, data_producing: true, schema_id: Some("capabilities") },
    CommandSpec { path: &["schema"],           mutating: false, data_producing: true, schema_id: None },
    CommandSpec { path: &["llm-instructions"], mutating: false, data_producing: false, schema_id: None },
];

/// Verb-first aliases retained for muscle memory (§1).
///
/// Each entry maps the alias name to its canonical noun-first form.
#[allow(dead_code)]
const VERB_FIRST_ALIASES: &[(&str, &[&str])] = &[
    ("screenshot",  &["screen", "capture"]),
    ("record",      &["screen", "record"]),
    ("screen-size", &["screen", "size"]),
    ("find-text",   &["screen", "find-text"]),
    ("upload",      &["file", "upload"]),
    ("download",    &["file", "download"]),
    ("exec",        &["file", "exec"]),
];

/// Synonym aliases required by §1.
#[allow(dead_code)]
const SYNONYM_ALIASES: &[(&[&str], &[&str])] = &[
    (&["vm", "list"],      &["vm", "ls"]),
    (&["vm", "delete"],    &["vm", "rm"]),
    (&["vm", "delete"],    &["vm", "remove"]),
    (&["agent", "inspect"], &["agent", "show"]),
];

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run(args: &[&str]) -> Output {
    Command::new(BIN)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to invoke `{BIN} {}`: {e}", args.join(" ")))
}

/// Path to `docs/reference/cli-schemas/`, resolved from this crate's
/// manifest dir.
#[allow(dead_code)]
fn schema_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR points at crates/testanyware-cli; schemas live
    // four levels up at <repo>/docs/reference/cli-schemas.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("docs")
        .join("reference")
        .join("cli-schemas")
}

#[allow(dead_code)]
fn schema_path(schema_id: &str) -> PathBuf {
    schema_dir().join(format!("{schema_id}.json"))
}

#[allow(dead_code)]
fn schema_file_exists(schema_id: &str) -> bool {
    Path::new(&schema_path(schema_id)).is_file()
}

// ---------------------------------------------------------------------------
// Active checks — what the skeleton verifies today
// ---------------------------------------------------------------------------

#[test]
fn binary_exists_and_top_level_help_succeeds() {
    let out = run(&["--help"]);
    assert!(
        out.status.success(),
        "`testanyware --help` exited non-zero (status: {:?}, stderr: {})",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("testanyware"),
        "top-level --help did not mention the binary name; got:\n{stdout}",
    );
}

#[test]
fn version_flag_succeeds() {
    let out = run(&["--version"]);
    assert!(
        out.status.success(),
        "`testanyware --version` exited non-zero (status: {:?})",
        out.status,
    );
}

/// Walk every top-level subcommand currently exposed by the binary and
/// confirm `<sub> --help` exits 0. This catches clap-level breakage in
/// any port task without asserting the §7 help-template (which is the
/// per-command port task's responsibility, see the ignored
/// `each_subcommand_help_follows_template` test below).
///
/// Subcommands not yet present in the binary are skipped silently — the
/// purpose of this test is "every command that exists has working help",
/// not "every contract command exists yet". The ignored
/// `every_canonical_command_is_present` test enforces the latter once
/// the surface migration to §1 is complete.
#[test]
fn each_present_subcommand_help_succeeds() {
    let top_level = top_level_subcommands_from_help();
    assert!(
        !top_level.is_empty(),
        "could not parse any subcommands from `testanyware --help`; \
         either the binary is broken or the help format changed and this \
         parser needs updating",
    );

    for sub in &top_level {
        let out = run(&[sub, "--help"]);
        assert!(
            out.status.success(),
            "`testanyware {sub} --help` exited non-zero (status: {:?}, stderr: {})",
            out.status,
            String::from_utf8_lossy(&out.stderr),
        );
    }
}

/// Parse the top-level subcommand names from `testanyware --help`.
///
/// clap's default help layout lists subcommands under a `Commands:`
/// header, one per line, indented, with the name as the first whitespace-
/// separated token. Anything before that header (description, usage,
/// options) is skipped. Anything after a blank line ends the section.
fn top_level_subcommands_from_help() -> Vec<String> {
    let stdout = String::from_utf8_lossy(&run(&["--help"]).stdout).into_owned();
    let mut in_commands = false;
    let mut names = Vec::new();
    for line in stdout.lines() {
        if line.starts_with("Commands:") {
            in_commands = true;
            continue;
        }
        if !in_commands {
            continue;
        }
        if line.trim().is_empty() {
            break;
        }
        if let Some(name) = line.split_whitespace().next() {
            // "help" is auto-added by clap; not a contract command.
            if name != "help" {
                names.push(name.to_string());
            }
        }
    }
    names
}

// ---------------------------------------------------------------------------
// Per-clause stubs — filled in by the port tasks named in each comment
// ---------------------------------------------------------------------------
//
// These tests are `#[ignore]`d so the suite stays green until each port
// task fills in its slice of the contract. Running `cargo test --
// --ignored` will panic at the `todo!()` call in any clause that has not
// yet been implemented, naming the contract section and the responsible
// port task.

/// Contract §1: every canonical command listed in `CANONICAL_COMMANDS`
/// is reachable via the binary's help tree.
#[test]
fn every_canonical_command_is_present() {
    let mut failures: Vec<(Vec<&'static str>, String)> = Vec::new();
    for spec in CANONICAL_COMMANDS {
        let mut argv: Vec<&str> = spec.path.to_vec();
        argv.push("--help");
        let out = run(&argv);
        if !out.status.success() {
            failures.push((
                spec.path.to_vec(),
                format!(
                    "exit {:?}, stderr: {}",
                    out.status,
                    String::from_utf8_lossy(&out.stderr),
                ),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "canonical commands missing from clap tree:\n{failures:#?}",
    );
}

/// Contract §1 + §7.2: every verb-first alias in `VERB_FIRST_ALIASES`
/// has working `--help`, and its help body announces itself as
/// "Alias of `testanyware <canonical>`" rather than re-documenting the
/// canonical command.
#[test]
fn verb_first_aliases_match_canonical() {
    let mut failures: Vec<String> = Vec::new();
    for (alias, canonical) in VERB_FIRST_ALIASES {
        let out = run(&[alias, "--help"]);
        if !out.status.success() {
            failures.push(format!(
                "`testanyware {alias} --help` exited non-zero: stderr: {}",
                String::from_utf8_lossy(&out.stderr),
            ));
            continue;
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let needle = format!("Alias of `testanyware {}`", canonical.join(" "));
        if !stdout.contains(&needle) {
            failures.push(format!(
                "`testanyware {alias} --help` missing announcement {needle:?}; got:\n{stdout}",
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "verb-first alias announcement failures:\n{}",
        failures.join("\n---\n"),
    );
}

/// Contract §1: synonym aliases (`ls`, `rm`, `remove`, `show`) produce
/// the same help output as their canonical (clap dispatches them through
/// to the canonical's help body).
#[test]
fn synonym_aliases_match_canonical() {
    let mut failures: Vec<String> = Vec::new();
    for (canonical, alias) in SYNONYM_ALIASES {
        let mut canonical_argv: Vec<&str> = canonical.to_vec();
        canonical_argv.push("--help");
        let canonical_out = run(&canonical_argv);
        let mut alias_argv: Vec<&str> = alias.to_vec();
        alias_argv.push("--help");
        let alias_out = run(&alias_argv);

        if !canonical_out.status.success() {
            failures.push(format!(
                "canonical {canonical:?} --help failed: {}",
                String::from_utf8_lossy(&canonical_out.stderr),
            ));
            continue;
        }
        if !alias_out.status.success() {
            failures.push(format!(
                "alias {alias:?} --help failed: {}",
                String::from_utf8_lossy(&alias_out.stderr),
            ));
            continue;
        }
        if canonical_out.stdout != alias_out.stdout {
            failures.push(format!(
                "alias {alias:?} produced different help than canonical {canonical:?}:\n\
                 --- canonical ---\n{}\n\
                 --- alias ---\n{}",
                String::from_utf8_lossy(&canonical_out.stdout),
                String::from_utf8_lossy(&alias_out.stdout),
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "synonym alias failures:\n{}",
        failures.join("\n---\n"),
    );
}

/// Contract §7: every subcommand's `--help` contains the required
/// sections in order — one-line summary, description, USAGE, OPTIONS,
/// OUTPUT, EXIT CODES, EXAMPLES (≥ 2), SEE ALSO.
#[test]
#[ignore = "contract §7: implemented per-command as ports land"]
fn each_subcommand_help_follows_template() {
    todo!(
        "walk CANONICAL_COMMANDS; for each, assert the help body contains \
         each of: 'USAGE', 'OUTPUT', 'EXIT CODES', 'EXAMPLES', 'SEE ALSO', \
         and at least two example invocations (heuristic: two lines \
         starting with `testanyware ` inside the EXAMPLES section). \
         §11 acceptance criterion #1 — filled in incrementally by each \
         per-command port task."
    );
}

/// Contract §3.1: every data-producing command accepts `--json` and the
/// resulting stdout is a single JSON document (or one-per-line under
/// `--output jsonl`) carrying `schema_version`.
#[test]
#[ignore = "contract §3.1: implemented per-command as ports land"]
fn each_data_command_supports_json() {
    todo!(
        "for each spec in CANONICAL_COMMANDS where data_producing == true, \
         run a benign invocation with `--json` and assert: (a) stdout \
         parses as serde_json::Value, (b) the value is an object \
         containing the key `schema_version`. §11 acceptance criterion \
         #2 — filled in by each per-command port task once the command \
         actually produces output."
    );
}

/// Contract §3.3: a schema file exists at
/// `docs/reference/cli-schemas/<schema-id>.json` for every distinct
/// `schema_id` in `CANONICAL_COMMANDS`. Stub schemas (with only
/// `schema_version` and `$comment: "TODO"`) are explicitly permitted by
/// §3.3 so this check can be enabled before any command emits real data.
#[test]
fn every_schema_id_has_a_schema_file() {
    let mut seen = std::collections::BTreeSet::new();
    let mut missing = Vec::new();
    let mut malformed = Vec::new();

    for spec in CANONICAL_COMMANDS {
        let Some(id) = spec.schema_id else { continue };
        if !seen.insert(id) {
            continue;
        }
        let path = schema_path(id);
        if !path.is_file() {
            missing.push((id, path));
            continue;
        }
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                malformed.push((id, path, format!("read failed: {e}")));
                continue;
            }
        };
        if let Err(e) = serde_json::from_slice::<serde_json::Value>(&bytes) {
            malformed.push((id, path, format!("invalid JSON: {e}")));
        }
    }

    assert!(
        missing.is_empty() && malformed.is_empty(),
        "schema files missing or malformed:\n\
         missing: {missing:#?}\n\
         malformed: {malformed:#?}",
    );
}

/// Contract §3.4: when `--json` is set and the command fails, stdout
/// carries exactly one JSON error object whose `code` is one of the
/// stable strings in §4 and whose exit code matches §5.
#[test]
#[ignore = "contract §3.4 + §4 + §5: implemented per-command as ports land"]
fn errors_carry_stable_code_and_correct_exit() {
    todo!(
        "design crafted invocations per command that should fail with a \
         specific code (e.g. `vm stop nonsense` → VM_NOT_FOUND, exit 3). \
         Assert stdout JSON parses, has `ok: false`, `code` from §4, \
         and process exit matches §5. §11 acceptance criterion #3 — \
         filled in by each per-command port task that introduces the \
         error path."
    );
}

/// Contract §6.1: every identifier in `--json` output round-trips as
/// input to a sibling command. The canonical pair is `vm start --json`
/// → take the returned `id` → `vm stop <id>`.
#[test]
#[ignore = "contract §6.1: implemented when both ends of the round-trip pair land"]
fn identifiers_round_trip() {
    todo!(
        "exercise: vm start --json (parse id) → vm stop <id>. Repeat for \
         each identifier-bearing command pair (golden_name from vm list \
         → vm delete; element id from agent inspect → agent press --id). \
         §11 acceptance criterion #5 — requires live VM infrastructure, \
         so this is the integration variant of the contract gate."
    );
}

/// Contract §9.3: every mutating command accepts `--dry-run`, validates
/// inputs, resolves the connection, emits the planned action, and exits
/// 0 without performing the mutation. JSON envelope sets
/// `"dry_run": true`.
#[test]
#[ignore = "contract §9.3: implemented per-command as mutating ports land"]
fn each_mutating_command_supports_dry_run() {
    todo!(
        "for each spec in CANONICAL_COMMANDS where mutating == true, run \
         `<path...> --dry-run --json` with placeholder args; assert exit \
         0 and JSON contains `dry_run: true`. §11 acceptance criterion \
         #4 — filled in by each per-command port task that introduces the \
         mutation."
    );
}

/// Contract §9.4: `vm list`, `agent windows`, `agent snapshot` (flat
/// element list mode), and `screen find-text` (no-query mode) default to
/// `--limit 100` and signal truncation per §3.5.
#[test]
#[ignore = "contract §9.4: enable when listing commands are ported"]
fn list_commands_default_limit_and_truncate() {
    todo!(
        "assert each list-mode command emits a JSON envelope with `items`, \
         `returned`, `total`, `truncated` per §3.5. Synthesise enough \
         items in the target (or stub) to force truncation. Filled in by \
         the listing-commands port tasks."
    );
}

/// Contract §8.1: `capabilities --json` enumerates every public
/// subcommand from §1 and every error code from §4.
#[test]
#[ignore = "contract §8.1: enable when `capabilities` is ported"]
fn capabilities_lists_full_surface() {
    todo!(
        "run `testanyware capabilities --json`; assert `subcommands` is \
         a superset of CANONICAL_COMMANDS' top-level groups, and \
         `error_codes` contains every code referenced in §4. Filled in by \
         the `capabilities` port task."
    );
}

/// Contract §8.2: `testanyware schema <command>` emits the JSON Schema
/// for `<command> --json` (or exits 3 with `SCHEMA_NOT_FOUND` on miss).
#[test]
#[ignore = "contract §8.2: enable when `schema` is ported"]
fn schema_command_emits_json_schema_for_each_command() {
    todo!(
        "for each spec in CANONICAL_COMMANDS with schema_id.is_some(), \
         run `testanyware schema <path...>`; assert stdout parses as \
         JSON Schema (object with $schema or type field) matching the \
         file at schema_path(spec.schema_id.unwrap()). Filled in by the \
         `schema` port task."
    );
}

/// Contract §8.3: `testanyware llm-instructions` emits a focused manual
/// (~3000 tokens cap, English-only).
#[test]
#[ignore = "contract §8.3: enable when `llm-instructions` is ported"]
fn llm_instructions_command_emits_manual() {
    todo!(
        "assert `testanyware llm-instructions` exits 0 and stdout is \
         non-empty text. §8.3 says ~3000-token cap; we don't enforce a \
         hard token count here (that lives in the port task's own \
         tests). Filled in by the `llm-instructions` port task."
    );
}
