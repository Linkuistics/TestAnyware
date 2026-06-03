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
//
// The canonical surface lives in `testanyware_cli::surface` (the single
// source of truth consumed by `main.rs` and the §8 discoverability
// handlers). Adding a new command to `surface.rs` automatically updates
// this test's expectations — there is no parallel table to keep in sync.

use testanyware_cli::surface::{CANONICAL_COMMANDS, SYNONYM_ALIASES, VERB_FIRST_ALIASES};

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

/// Contract §7: every canonical subcommand's `--help` carries the required
/// sections (OUTPUT, EXIT CODES, EXAMPLES, SEE ALSO) and at least two
/// concrete example invocations. §11 acceptance criterion #1, enforced over
/// the whole surface (the per-command `*_help_follows_template` tests above
/// are subsumed here; they remain as finer-grained diagnostics).
#[test]
fn each_subcommand_help_follows_template() {
    let mut failures: Vec<String> = Vec::new();
    for spec in CANONICAL_COMMANDS {
        let mut argv: Vec<&str> = spec.path.to_vec();
        argv.push("--help");
        let out = run(&argv);
        let path = spec.path.join(" ");
        if !out.status.success() {
            failures.push(format!(
                "`{path}` --help exited non-zero: {}",
                String::from_utf8_lossy(&out.stderr),
            ));
            continue;
        }
        let help = String::from_utf8_lossy(&out.stdout);

        // §7 sections. Matched as substrings (not `"<SECTION>:"`) so the
        // `file exec` variant titled "EXIT CODES (text mode):" still counts,
        // and the `viewer` interactive carve-out — whose OUTPUT/EXIT CODES
        // document "None" / its window-lifecycle codes — is not special-cased.
        for section in ["OUTPUT", "EXIT CODES", "EXAMPLES", "SEE ALSO"] {
            if !help.contains(section) {
                failures.push(format!("`{path}` --help missing §7 section {section:?}"));
            }
        }

        // §7.7: ≥ 2 concrete example invocations. Heuristic: the EXAMPLES
        // block lists `testanyware <path> …` lines; count full-path
        // occurrences across the help body (SEE ALSO may add a few, which
        // only ever helps clear the ≥2 bar — never masks a real shortfall).
        let needle = format!("testanyware {path}");
        let examples = help.matches(&needle).count();
        if examples < 2 {
            failures.push(format!(
                "`{path}` --help needs ≥2 example invocations ({needle:?}), found {examples}",
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "§7 help-template failures:\n{}",
        failures.join("\n"),
    );
}

/// Contract §3.1: every data-producing command accepts `--json` and the
/// resulting stdout is a single JSON document (or one-per-line under
/// `--output jsonl`) carrying `schema_version`.
///
/// LIVE-VM GATE. The happy-path `--json` output of most data commands is
/// produced only against a running VM (capture a framebuffer, snapshot the
/// AX tree, OCR the screen, list real clones). Those `--json` emissions are
/// exercised in `tests/live-vm-gate.rs` (e.g. `screen find-text --json`,
/// `screen record --json`, `agent snapshot --json`). The offline surface —
/// that the flag exists and the envelope shape is well-formed — is covered
/// by the per-command tests in this file (`vm_list_json_emits_truncation_
/// envelope`, `doctor_json_emits_report_envelope`, `capabilities_lists_full_
/// surface`). The ports have landed; this stays gated because a *benign*
/// invocation of every data command needs a live target, not because any
/// command is unimplemented.
#[test]
#[ignore = "live-VM gate: §3.1 happy-path --json needs a running VM — see tests/live-vm-gate.rs"]
fn each_data_command_supports_json() {
    // No offline assertion is possible for the generic sweep: a benign
    // `--json` invocation of (e.g.) `screen capture` / `agent snapshot`
    // requires a connected VM. See the live-VM gate and the per-command
    // offline envelope tests named in the doc comment above.
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
///
/// LIVE-VM GATE for the *generic* per-command sweep. Most error paths
/// (`AUTH_REQUIRED`, `WINDOW_NOT_FOUND`, `ELEMENT_AMBIGUOUS`, agent-wire
/// codes) only arise against a running VM and are exercised in
/// `tests/live-vm-gate.rs`. The offline-reachable error envelopes are
/// already asserted by the per-command tests in this file:
/// `vm_commands_carry_stable_error_codes` (VM_NOT_FOUND/INVALID_PLATFORM/
/// GOLDEN_NOT_FOUND), `screen_find_text_connection_error_envelope`
/// (IO_ERROR), and the `schema`-miss path in
/// `schema_command_emits_json_schema_for_each_command` (SCHEMA_NOT_FOUND).
#[test]
#[ignore = "live-VM gate: §3.4 generic error sweep needs a running VM — see tests/live-vm-gate.rs"]
fn errors_carry_stable_code_and_correct_exit() {
    // Offline-reachable error envelopes are covered by the per-command
    // tests named in the doc comment; the generic cross-surface sweep needs
    // a live VM to drive the connection/agent error families.
}

/// Contract §6.1: every identifier in `--json` output round-trips as
/// input to a sibling command. The canonical pair is `vm start --json`
/// → take the returned `id` → `vm stop <id>`.
///
/// LIVE-VM GATE. Round-tripping requires producing real ids: `vm start
/// --json` boots a VM, `agent inspect --json` issues an AX element id.
/// Both ends need live infrastructure, so this is the integration variant
/// of the contract gate, owned by `tests/live-vm-gate.rs`. The ports on
/// both ends have landed; this stays gated on infrastructure, not work.
#[test]
#[ignore = "live-VM gate: §6.1 round-trip needs a running VM to mint ids — see tests/live-vm-gate.rs"]
fn identifiers_round_trip() {
    // No offline check: minting a real `vm_id` / element id requires a
    // running VM. The round-trip pairs (vm start→stop, vm list→delete,
    // agent inspect→press) are integration concerns of the live-VM gate.
}

/// Contract §9.3: every mutating command accepts `--dry-run`, emits the
/// planned action, and exits 0 without performing the mutation. The JSON
/// envelope sets `"dry_run": true`.
///
/// The connection-based mutating commands (input *, agent action/window,
/// screen record, file *) all short-circuit before any network I/O in
/// dry-run, so placeholder args drive them offline. The `vm` group is
/// excluded here and covered with fixtures by
/// `vm_mutating_commands_support_dry_run` (stop/delete) — vm dry-run
/// validates against *local backend state* (a running-VM meta sidecar, a
/// golden qcow2, the QEMU host preflight) that uniform placeholder args
/// cannot synthesize; vm create-golden's plan is macOS-gated.
#[test]
fn each_mutating_command_supports_dry_run() {
    // Minimal required args (after the command path) that let each
    // connection-based mutating command reach its dry-run short-circuit.
    fn recipe(path: &[&str]) -> Option<Vec<&'static str>> {
        Some(match path {
            ["agent", "press"] => vec!["--role", "button"],
            ["agent", "set-value"] => vec!["--role", "textfield", "--value", "x"],
            ["agent", "focus"] => vec!["--role", "button"],
            ["agent", "show-menu"] => vec!["--menu", "File"],
            ["agent", "window-focus"]
            | ["agent", "window-close"]
            | ["agent", "window-minimize"] => vec!["--window", "W"],
            ["agent", "window-resize"] => vec!["--window", "W", "--width", "1", "--height", "1"],
            ["agent", "window-move"] => vec!["--window", "W", "--x", "1", "--y", "1"],
            ["input", "key"] | ["input", "key-down"] | ["input", "key-up"] => vec!["a"],
            ["input", "type"] => vec!["hello"],
            ["input", "click"]
            | ["input", "mouse-down"]
            | ["input", "mouse-up"]
            | ["input", "move"]
            | ["input", "scroll"] => vec!["10", "20"],
            ["input", "drag"] => vec!["0", "0", "10", "10"],
            ["screen", "record"] => vec!["-o", "/tmp/testanyware-contract-dryrun.mp4"],
            ["file", "upload"] => vec!["/tmp/testanyware-a", "/tmp/testanyware-b"],
            ["file", "download"] => vec!["/tmp/testanyware-a", "/tmp/testanyware-b"],
            ["file", "exec"] => vec!["true"],
            _ => return None,
        })
    }

    let mut failures: Vec<String> = Vec::new();
    let mut unrecognized: Vec<String> = Vec::new();
    for spec in CANONICAL_COMMANDS {
        if !spec.mutating {
            continue;
        }
        // vm group: covered with local-state fixtures elsewhere (see doc).
        if spec.path.first() == Some(&"vm") {
            continue;
        }
        let Some(extra) = recipe(spec.path) else {
            unrecognized.push(spec.path.join(" "));
            continue;
        };
        let mut argv: Vec<&str> = spec.path.to_vec();
        argv.extend(extra);
        argv.push("--dry-run");
        argv.push("--json");
        let out = run(&argv);
        let shown = argv.join(" ");
        if out.status.code() != Some(0) {
            failures.push(format!(
                "`{shown}` exited {:?} (want 0); stderr: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr),
            ));
            continue;
        }
        let body: serde_json::Value = match serde_json::from_slice(&out.stdout) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!("`{shown}` dry-run stdout did not parse as JSON: {e}"));
                continue;
            }
        };
        if body.get("dry_run").and_then(|v| v.as_bool()) != Some(true) {
            failures.push(format!(
                "`{shown}` dry-run JSON missing `dry_run: true`; got: {body}",
            ));
        }
    }

    // Honesty guard: a new mutating command in surface.rs without a recipe
    // (and not in the vm group) must not silently skip the sweep.
    assert!(
        unrecognized.is_empty(),
        "mutating command(s) missing a dry-run recipe — add to recipe() or \
         cover with a fixture test:\n{}",
        unrecognized.join("\n"),
    );
    assert!(
        failures.is_empty(),
        "§9.3 dry-run sweep failures:\n{}",
        failures.join("\n"),
    );
}

/// Contract §9.4: `vm list`, `agent windows`, `agent snapshot` (flat
/// element list mode), and `screen find-text` (no-query mode) default to
/// `--limit 100` and signal truncation per §3.5.
///
/// LIVE-VM GATE for the truncation behaviour. `vm list`'s offline envelope
/// shape (`items`/`returned`/`total`/`truncated`) is already asserted by
/// `vm_list_json_emits_truncation_envelope`. Forcing actual truncation on
/// `agent windows` / `agent snapshot` / `screen find-text` requires a live
/// VM populated with enough windows/elements/on-screen text to exceed the
/// default limit, which is owned by `tests/live-vm-gate.rs`.
#[test]
#[ignore = "live-VM gate: §9.4 truncation needs a populated running VM — see tests/live-vm-gate.rs"]
fn list_commands_default_limit_and_truncate() {
    // Offline: `vm_list_json_emits_truncation_envelope` covers the envelope
    // shape. Forcing truncation on the agent/screen list commands needs a
    // live VM with >limit items — an integration concern of the live gate.
}

/// Contract §8.1: `capabilities --json` enumerates every public
/// subcommand from §1 and every error code from §4.
#[test]
fn capabilities_lists_full_surface() {
    let out = run(&["capabilities", "--json"]);
    assert!(
        out.status.success(),
        "`testanyware capabilities --json` exited non-zero (status: {:?}, stderr: {})",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );

    let body: serde_json::Value = serde_json::from_slice(&out.stdout)
        .expect("capabilities stdout must parse as JSON");

    let obj = body.as_object().expect("capabilities body is a JSON object");
    assert_eq!(
        obj.get("ok").and_then(|v| v.as_bool()),
        Some(true),
        "capabilities body missing ok:true; got: {body:#?}",
    );
    assert!(
        obj.get("schema_version").and_then(|v| v.as_str()).is_some(),
        "capabilities body missing schema_version; got: {body:#?}",
    );

    // §8.1: `subcommands` must include every top-level group reachable
    // from the canonical surface.
    let subcommands: Vec<&str> = obj
        .get("subcommands")
        .and_then(|v| v.as_array())
        .expect("capabilities.subcommands must be an array")
        .iter()
        .map(|v| v.as_str().expect("subcommands entries are strings"))
        .collect();

    let expected_groups: std::collections::BTreeSet<&str> = CANONICAL_COMMANDS
        .iter()
        .map(|spec| spec.path[0])
        .collect();
    for group in &expected_groups {
        assert!(
            subcommands.contains(group),
            "capabilities.subcommands missing canonical group {group:?}; got: {subcommands:?}",
        );
    }

    // §8.1 + §4.7: `error_codes` carries the catalogue. Spot-check codes
    // that span §4.1, §4.2, §4.5, §4.6, and §8.2.
    let error_codes: Vec<&str> = obj
        .get("error_codes")
        .and_then(|v| v.as_array())
        .expect("capabilities.error_codes must be an array")
        .iter()
        .map(|v| v.as_str().expect("error_codes entries are strings"))
        .collect();
    for required in [
        "AUTH_REQUIRED",
        "VM_NOT_FOUND",
        "ELEMENT_NOT_FOUND",
        "USAGE_ERROR",
        "SCHEMA_NOT_FOUND",
    ] {
        assert!(
            error_codes.contains(&required),
            "capabilities.error_codes missing {required:?}; got: {error_codes:?}",
        );
    }
}

/// Contract §8.2: `testanyware schema <command>` emits the JSON Schema
/// for `<command> --json` (or exits 3 with `SCHEMA_NOT_FOUND` on miss).
#[test]
fn schema_command_emits_json_schema_for_each_command() {
    let mut failures: Vec<String> = Vec::new();

    for spec in CANONICAL_COMMANDS {
        let Some(schema_id) = spec.schema_id else { continue };

        let mut argv: Vec<&str> = Vec::with_capacity(spec.path.len() + 1);
        argv.push("schema");
        for token in spec.path {
            argv.push(token);
        }
        let out = run(&argv);
        if !out.status.success() {
            failures.push(format!(
                "`testanyware {}` exited non-zero (status: {:?}): {}",
                argv.join(" "),
                out.status,
                String::from_utf8_lossy(&out.stderr),
            ));
            continue;
        }

        let actual: serde_json::Value = match serde_json::from_slice(&out.stdout) {
            Ok(v) => v,
            Err(e) => {
                failures.push(format!(
                    "`testanyware {}` stdout did not parse as JSON: {e}",
                    argv.join(" "),
                ));
                continue;
            }
        };
        let actual_obj = match actual.as_object() {
            Some(o) => o,
            None => {
                failures.push(format!(
                    "`testanyware {}` did not produce a JSON object",
                    argv.join(" "),
                ));
                continue;
            }
        };
        if !(actual_obj.contains_key("$schema") || actual_obj.contains_key("type")) {
            failures.push(format!(
                "`testanyware {}` output lacks $schema or type — not a JSON Schema",
                argv.join(" "),
            ));
            continue;
        }

        // Must equal the on-disk schema file byte-for-byte (modulo
        // semantic value comparison for whitespace tolerance).
        let path = schema_path(schema_id);
        let file_bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                failures.push(format!(
                    "could not read schema file {}: {e}",
                    path.display(),
                ));
                continue;
            }
        };
        let expected: serde_json::Value = serde_json::from_slice(&file_bytes)
            .expect("schema file is malformed");
        if expected != actual {
            failures.push(format!(
                "`testanyware {}` output differs from {}",
                argv.join(" "),
                path.display(),
            ));
        }
    }

    // Miss path: a path that is not a canonical command must exit 3 and
    // emit a SCHEMA_NOT_FOUND envelope on stdout.
    let miss = run(&["schema", "definitely", "not", "real"]);
    assert_eq!(
        miss.status.code(),
        Some(3),
        "schema-miss must exit 3 (got: {:?}, stderr: {})",
        miss.status,
        String::from_utf8_lossy(&miss.stderr),
    );
    let miss_body: serde_json::Value = serde_json::from_slice(&miss.stdout)
        .expect("schema-miss stdout must parse as JSON");
    assert_eq!(
        miss_body.get("code").and_then(|v| v.as_str()),
        Some("SCHEMA_NOT_FOUND"),
        "schema-miss code must be SCHEMA_NOT_FOUND; got: {miss_body:#?}",
    );

    assert!(failures.is_empty(), "schema command failures:\n{}", failures.join("\n---\n"));
}

/// Contract §8.3: `testanyware llm-instructions` emits the full LLM
/// usage guide — kept lean enough to prepend as LLM context (byte
/// ceiling asserted below).
#[test]
fn llm_instructions_command_emits_manual() {
    let out = run(&["llm-instructions"]);
    assert!(
        out.status.success(),
        "`testanyware llm-instructions` exited non-zero (status: {:?}, stderr: {})",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    let body = String::from_utf8_lossy(&out.stdout);
    assert!(
        !body.trim().is_empty(),
        "`testanyware llm-instructions` produced empty stdout",
    );
    // §8.3 keeps the guide lean enough to prepend as LLM context.
    // Assert a generous byte ceiling so it cannot bloat unbounded:
    // ~4 chars/token × 3000 tokens × 1.5x slack ≈ 18000 bytes.
    assert!(
        body.len() < 18_000,
        "llm-instructions output is {} bytes — too large to prepend as LLM context (§8.3)",
        body.len(),
    );
}

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

// ---------------------------------------------------------------------------
// doctor — port-task slice (020-port-doctor)
// ---------------------------------------------------------------------------

/// §7: `doctor --help` carries the required sections and ≥2 example
/// invocations.
#[test]
fn doctor_help_follows_template() {
    let out = run(&["doctor", "--help"]);
    assert!(out.status.success(), "`doctor --help` exited non-zero");
    let help = String::from_utf8_lossy(&out.stdout);
    for section in ["OUTPUT:", "EXIT CODES:", "EXAMPLES:", "SEE ALSO:"] {
        assert!(help.contains(section), "`doctor --help` missing {section:?}; got:\n{help}");
    }
    let examples = help.matches("testanyware doctor").count();
    assert!(examples >= 2, "`doctor --help` needs ≥2 example invocations, found {examples}");
}

/// §3.1 + §10.1: `doctor --json` emits a single report envelope with
/// `schema_version`, a boolean `ok`, and the five structured check groups.
/// doctor is read-only and safe to run; it exits 0 when healthy or 1 when a
/// blocking check fails — both are valid here, so we assert the shape, not the
/// host's health.
#[test]
fn doctor_json_emits_report_envelope() {
    let out = run(&["doctor", "--json"]);
    assert!(
        matches!(out.status.code(), Some(0) | Some(1)),
        "`doctor --json` must exit 0 (healthy) or 1 (unhealthy); got {:?}, stderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );
    let body: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("doctor --json must parse as JSON");
    assert!(body["schema_version"].is_string(), "missing schema_version; got: {body:#?}");
    assert!(body["ok"].is_boolean(), "ok must be a boolean; got: {body:#?}");
    let checks = body["checks"].as_object().expect("checks must be an object");
    for group in [
        "install_path",
        "bundled_agents",
        "bundled_scripts",
        "host_tools",
        "script_tool_floors",
    ] {
        assert!(checks.contains_key(group), "checks missing {group:?}; got: {body:#?}");
        assert!(
            checks[group]["status"].is_string(),
            "check {group:?} missing string status; got: {body:#?}",
        );
    }
}

/// §9.3: doctor is read-only and MUST NOT advertise `--dry-run`.
#[test]
fn doctor_does_not_advertise_dry_run() {
    let out = run(&["doctor", "--help"]);
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(!help.contains("--dry-run"), "doctor is read-only; must not advertise --dry-run");
    // And passing it is a usage error (clap exit 2).
    let bad = run(&["doctor", "--dry-run"]);
    assert_eq!(bad.status.code(), Some(2), "unknown --dry-run flag must be a usage error");
}

// ---------------------------------------------------------------------------
// screen find-text — port-task slice (030-screen-find-text)
//
// The `--json` happy path captures a framebuffer and runs OCR, so it needs a
// live VM + OCR daemon; that behaviour is covered by the live-VM gate, not
// here (mirrors the `#[ignore]`d generic §3.1 check). These checks verify the
// command's offline surface: the §7 help template, read-only status (§9.3),
// and the §3.4 error envelope on a connection-resolution failure.
// ---------------------------------------------------------------------------

/// §7: `screen find-text --help` carries the required sections and ≥2
/// example invocations; the verb-first alias points back to the canonical.
#[test]
fn screen_find_text_help_follows_template() {
    let out = run(&["screen", "find-text", "--help"]);
    assert!(out.status.success(), "`screen find-text --help` exited non-zero");
    let help = String::from_utf8_lossy(&out.stdout);
    for section in ["OUTPUT:", "EXIT CODES:", "EXAMPLES:", "SEE ALSO:"] {
        assert!(help.contains(section), "find-text help missing {section:?}; got:\n{help}");
    }
    let examples = help.matches("testanyware screen find-text").count();
    assert!(examples >= 2, "find-text help needs ≥2 example invocations, found {examples}");

    let alias = run(&["find-text", "--help"]);
    assert!(alias.status.success(), "`find-text --help` (alias) exited non-zero");
    assert!(
        String::from_utf8_lossy(&alias.stdout)
            .contains("Alias of `testanyware screen find-text`"),
        "alias help must point at the canonical command",
    );
}

/// §9.3: `screen find-text` is read-only and MUST NOT advertise `--dry-run`.
#[test]
fn screen_find_text_is_read_only() {
    let out = run(&["screen", "find-text", "--help"]);
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(!help.contains("--dry-run"), "find-text is read-only; must not advertise --dry-run");
    let bad = run(&["screen", "find-text", "x", "--dry-run"]);
    assert_eq!(bad.status.code(), Some(2), "unknown --dry-run flag must be a usage error");
}

/// §3.4 + §4 + §5: a connection-resolution failure emits a single JSON error
/// object with a stable `code` and the §5 exit code. `--connect` is the
/// highest-precedence target form, so pointing it at a missing file fails
/// deterministically (IO_ERROR, exit 1) regardless of any `TESTANYWARE_*`
/// env the test inherits — exercising the find-text handler's own error path
/// without a live VM.
#[test]
fn screen_find_text_connection_error_envelope() {
    let out = run(&["screen", "find-text", "x", "--connect", "/no/such/spec.json", "--json"]);
    assert_eq!(out.status.code(), Some(1), "missing --connect spec must exit 1 (IO_ERROR)");
    let body: serde_json::Value =
        serde_json::from_slice(&out.stdout).expect("error output must be JSON");
    assert_eq!(body["ok"], false);
    assert_eq!(body["code"], "IO_ERROR");
    assert!(body["schema_version"].is_string());
}
