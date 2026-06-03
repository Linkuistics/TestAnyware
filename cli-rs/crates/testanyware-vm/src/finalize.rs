//! Top-level macOS golden creation: the SIP/TCC cycle + finalize + clone.
//!
//! [`create_golden`] is the command-layer entry point for
//! `vm create-golden --platform macos`. It wires the whole pipeline together
//! on top of the two lower layers built by the earlier grove leaves:
//!
//!   1. **Boot 1** — [`crate::golden::provision_boot1`] (leaf `020`): clone the
//!      vanilla image into a throwaway *setup VM* and provision it over SSH.
//!   2. **The SIP/TCC cycle** (this module, ports the script's lines 677–723):
//!      disable SIP via a Recovery boot, grant the agent's TCC permissions
//!      (which require SIP off), re-enable SIP via a second Recovery boot. The
//!      two Recovery boots are [`crate::recovery::recovery_boot_csrutil`] (leaf
//!      `010`).
//!   3. **Finalize** (this module, script lines 745–818): the final agent-health
//!      gate, a clean desktop + clean shutdown, and `tart clone` to the golden.
//!
//! Ports the top-level orchestration of
//! `provisioner/scripts/vm-create-golden-macos.sh` — the **macro sequence is at
//! parity** (the 5-boot order, the disable→grant→enable→verify ordering, the
//! health gate, the System-Events clean shutdown). This is the finisher of grove
//! node `110`; on completion the source script is deleted.
//!
//! ## Fatal vs. tolerant (parity with the script)
//!
//! The script treats steps unevenly, and that asymmetry is load-bearing:
//!   - **SIP status checks are warnings, not failures.** A failed SIP-disable is
//!     caught downstream — the `sudo sqlite3` TCC write fails when SIP is on, and
//!     *that* verification is fatal. The authoritative SIP gate is the over-SSH
//!     `csrutil status` check here, but the script (and ADR-0008) deliberately
//!     keep it non-aborting.
//!   - **TCC-row verification and the agent-health gate are fatal.** A golden
//!     without working TCC grants or a responding agent is useless.
//!
//! **macOS-host only** (`tart` wraps Virtualization.framework), so this module is
//! `#[cfg(target_os = "macos")]`-gated at the crate root, like `golden.rs`.
//!
//! ## Progress narration
//!
//! Like boot-1, this multi-minute flow emits its running narration to **stderr**
//! via `eprintln!`, keeping `--json` stdout clean. The pure SQL/parse helpers are
//! unit-tested; the live `tart`/SSH orchestration is verified by actually
//! creating a golden on the Mac (cheap — `vm-costs`), not by unit tests.

use std::time::Duration;

use crate::error::VmError;
use crate::golden::{cleanup_setup_vm, provision_boot1, GoldenOptions, SetupVm, VANILLA_USER};
use crate::paths::VmPaths;
use crate::recovery::{recovery_boot_csrutil, wait_for_pid_exit};
use crate::ssh::SshSession;

// ---- constants ----------------------------------------------------------

/// Where boot-1 installs the agent — the `client` column of the TCC grants and
/// the target of the csreq blob. Ports the script's hard-coded path.
const AGENT_INSTALL_PATH: &str = "/usr/local/bin/testanyware-agent";

/// The system-level TCC database. SIP must be disabled before `sudo sqlite3`
/// can write to it (the reason the disable-SIP recovery boot comes first).
const TCC_DB: &str = "/Library/Application Support/com.apple.TCC/TCC.db";

/// `kTCCServiceAccessibility` — AXUIElement traversal, the agent's primary
/// GUI-introspection mechanism.
const SERVICE_ACCESSIBILITY: &str = "kTCCServiceAccessibility";
/// `kTCCServiceSystemPolicyAllFiles` (Full Disk Access) — so `exec`'d test
/// commands reading protected folders don't trip a TCC privacy dialog.
const SERVICE_FDA: &str = "kTCCServiceSystemPolicyAllFiles";

/// Agent-health gate budget: 30 × 2s = 60s of `curl` polling (script parity,
/// lines 748–754).
const AGENT_HEALTH_ATTEMPTS: u32 = 30;
const AGENT_HEALTH_INTERVAL: Duration = Duration::from_secs(2);

/// Final-shutdown pid-exit wait before forcing a `tart stop`. The script waits
/// 60 × 2s = 120s; we bound it tighter (30 × 2s = 60s) because the `010` live
/// runs showed the System-Events shutdown never takes effect headless — it
/// always falls through to the force-stop, so a long wait is pure dead time.
/// Boot-1 already disabled app-relaunch (`LoginwindowLaunchesRelaunchApps`) and
/// this step pre-cleans the desktop, so a force-stop still yields a clean golden.
const FINAL_SHUTDOWN_ATTEMPTS: u32 = 30;
const FINAL_SHUTDOWN_INTERVAL: Duration = Duration::from_secs(2);

// ---- pure helpers (unit-tested) -----------------------------------------

/// The `INSERT OR REPLACE` statement granting `service` to the agent, sharing
/// the `$CSREQ_HEX` shell variable computed once per [`grant_tcc_command`].
/// `client_type=1` (path), `auth_value=2` (allowed). Ports the script's two
/// near-identical inserts (lines 638–650) — only the service name differs.
fn tcc_insert_sql(service: &str) -> String {
    format!(
        "INSERT OR REPLACE INTO access \
         (service, client, client_type, auth_value, auth_reason, auth_version, csreq, \
          indirect_object_identifier_type, indirect_object_identifier, flags, last_modified) \
         VALUES \
         ('{service}', '{AGENT_INSTALL_PATH}', 1, 2, 0, 1, X'$CSREQ_HEX', 0, 'UNUSED', 0, \
          CAST(strftime('%s','now') AS INTEGER));"
    )
}

/// The single guest shell command that computes the shared csreq blob from the
/// agent's designated code-signing requirement and inserts both TCC grants.
/// Ports the script's `vm_ssh '...'` block (lines 632–650), but with only one
/// level of shell quoting (the guest's) since `russh` `exec` does not re-parse.
/// `$CSREQ_HEX` expands inside the double-quoted SQL argument; the single quotes
/// around the SQL string literals stay literal there. macOS Tahoe requires the
/// csreq field for TCC to accept the entry.
fn grant_tcc_command() -> String {
    format!(
        "CSREQ_HEX=$(codesign -dr- {AGENT_INSTALL_PATH} 2>&1 | sed -n 's/.*=> //p' | \
         csreq -r- -b /dev/stdout | xxd -p | tr -d '\\n') && \
         sudo sqlite3 \"{TCC_DB}\" \"{ax}\" && \
         sudo sqlite3 \"{TCC_DB}\" \"{fda}\"",
        ax = tcc_insert_sql(SERVICE_ACCESSIBILITY),
        fda = tcc_insert_sql(SERVICE_FDA),
    )
}

/// The verification `SELECT` for one grant: `auth_value|length(csreq)`. Ports
/// the script's verify queries (lines 700–705 / 728–733).
fn tcc_verify_sql(service: &str) -> String {
    format!(
        "SELECT auth_value, length(csreq) FROM access \
         WHERE service='{service}' AND client='{AGENT_INSTALL_PATH}';"
    )
}

/// Parse a `sudo sqlite3 … SELECT auth_value, length(csreq)` row — sqlite's
/// default `|`-separated output (e.g. `2|123`). Returns `(auth_value, csreq_len)`
/// from the first line, or `None` if the row is missing/unparsable (no grant).
fn parse_tcc_row(out: &str) -> Option<(i64, i64)> {
    let line = out.lines().next()?.trim();
    let (auth, len) = line.split_once('|')?;
    Some((auth.trim().parse().ok()?, len.trim().parse().ok()?))
}

/// A grant is good iff it is allowed (`auth_value == 2`) with a non-empty csreq
/// blob. Ports the script's `grep -q "^2|"` plus the `len > 0` intent.
fn grant_ok(row: Option<(i64, i64)>) -> bool {
    matches!(row, Some((2, len)) if len > 0)
}

/// Whether `csrutil status` output reports the `expected` state
/// (`"enabled"`/`"disabled"`), case-insensitively. Ports the script's
/// `grep -q "disabled"` / `grep -q "enabled"`. No false positive: `"disabled"`
/// does not contain the substring `"enabled"`.
fn sip_status_matches(out: &str, expected: &str) -> bool {
    out.to_lowercase().contains(expected)
}

// ---- live orchestration (verified on the Mac, not unit-tested) -----------

/// The command entry point: produce a golden image from a vanilla one. Runs
/// boot-1 provisioning, the SIP/TCC cycle, the health gate, the clean shutdown,
/// and the clone to golden. Returns the golden's name on success.
///
/// On any failure after boot-1, the setup VM is torn down (extending boot-1's
/// own cleanup-on-failure guard across the whole pipeline) so a botched run
/// never strands a setup VM. On success the setup VM is consumed by the final
/// `tart clone` + `tart delete`.
pub async fn create_golden(opts: &GoldenOptions, paths: &VmPaths) -> Result<String, VmError> {
    // Boot 1 cleans itself up on failure; from its Ok the setup VM is running.
    let setup = provision_boot1(opts, paths).await?;

    match run_sip_tcc_finalize(opts, setup, paths).await {
        Ok(name) => Ok(name),
        Err((setup, err)) => {
            eprintln!("Golden creation failed — tearing down setup VM '{}'.", setup.id);
            cleanup_setup_vm(&setup.id, setup.pid);
            Err(err)
        }
    }
}

/// The pipeline body after boot-1. Takes ownership of the [`SetupVm`] and, on
/// failure, hands the *latest* one back alongside the error so the caller tears
/// down the right pid (each recovery boot mints a new pid; the id is stable).
async fn run_sip_tcc_finalize(
    opts: &GoldenOptions,
    mut setup: SetupVm,
    paths: &VmPaths,
) -> Result<String, (SetupVm, VmError)> {
    // 1. Disable SIP (Recovery boot 1). `recovery_boot_csrutil` reboots normally
    //    back to an SSH-reachable state and returns the refreshed SetupVm.
    setup = match recovery_boot_csrutil(&setup, "csrutil disable", paths).await {
        Ok(s) => s,
        Err(e) => return Err((setup, e)),
    };
    // 2. Verify SIP disabled over SSH — non-fatal (the TCC write below is the
    //    real gate: it fails if SIP is still on).
    report_sip_status(&setup, "disabled").await;

    // 3. Grant TCC (requires SIP off).
    if let Err(e) = grant_tcc_permissions(&setup).await {
        return Err((setup, e));
    }
    // 4. Verify both TCC rows — fatal.
    if let Err(e) = verify_tcc_grants(&setup, "after grant").await {
        return Err((setup, e));
    }

    // 5. Re-enable SIP (Recovery boot 2).
    setup = match recovery_boot_csrutil(&setup, "csrutil enable", paths).await {
        Ok(s) => s,
        Err(e) => return Err((setup, e)),
    };
    report_sip_status(&setup, "enabled").await;

    // 6. Verify both TCC rows survive SIP re-enable — fatal.
    if let Err(e) = verify_tcc_grants(&setup, "after SIP re-enable").await {
        return Err((setup, e));
    }

    // Final agent-health gate — fatal.
    if let Err(e) = verify_agent_health(&setup).await {
        return Err((setup, e));
    }

    // Finalize: clean desktop, disable Remote Login, clean shutdown.
    finalize_and_shutdown(&setup).await;

    // Clone to golden (consumes the setup VM).
    if let Err(e) = clone_to_golden(&setup.id, &opts.name) {
        return Err((setup, e));
    }
    Ok(opts.name.clone())
}

/// Open a key-auth SSH session to the setup VM (pubkey installed in boot-1,
/// persists across the recovery cycle).
async fn ssh(setup: &SetupVm) -> Result<SshSession, VmError> {
    SshSession::connect_key(&setup.ip, 22, VANILLA_USER, &setup.key_path).await
}

/// Report `csrutil status` over SSH against the `expected` state. Non-fatal: a
/// mismatch warns but does not abort (script parity — the downstream gates catch
/// a genuinely wrong state).
async fn report_sip_status(setup: &SetupVm, expected: &str) {
    let status = match ssh(setup).await {
        Ok(s) => s.exec("csrutil status").await.map(|o| o.stdout).unwrap_or_default(),
        Err(_) => String::new(),
    };
    let line = status.trim();
    if sip_status_matches(&status, expected) {
        eprintln!("  SIP status: {line} (successfully {expected}).");
    } else {
        eprintln!("  WARNING: SIP may not be {expected} — csrutil status: {line:?}");
    }
}

/// Grant the agent's two TCC permissions (script `grant_tcc_permissions`, lines
/// 618–671). SIP must already be disabled. tccd is stopped before the write (it
/// locks `TCC.db`) and restarted after (it caches decisions) — launchd respawns
/// it either way, so the `killall`s are tolerant.
async fn grant_tcc_permissions(setup: &SetupVm) -> Result<(), VmError> {
    eprintln!("Granting TCC permissions to testanyware-agent...");
    let session = ssh(setup).await?;

    eprintln!("  Stopping tccd to release the database lock...");
    let _ = session.exec("sudo killall tccd").await; // tolerant: may not be running
    tokio::time::sleep(Duration::from_secs(2)).await;

    eprintln!("  Generating csreq blob and inserting TCC grants...");
    let out = session.exec(&grant_tcc_command()).await?;
    if out.exit_code != 0 {
        return Err(VmError::GoldenCreateFailed {
            detail: format!(
                "TCC grant insert failed (exit {}) — is SIP disabled? {}",
                out.exit_code,
                out.stderr.trim()
            ),
        });
    }

    eprintln!("  Restarting tccd to flush the TCC cache...");
    let _ = session.exec("sudo killall tccd").await; // tolerant
    tokio::time::sleep(Duration::from_secs(3)).await;
    Ok(())
}

/// Verify both TCC rows are present and allowed (`auth_value=2`, non-empty
/// csreq). Fatal on a missing/denied grant (script lines 699–713 / 727–743).
/// `when` labels the call site ("after grant" / "after SIP re-enable").
async fn verify_tcc_grants(setup: &SetupVm, when: &str) -> Result<(), VmError> {
    eprintln!("Verifying TCC database entries ({when})...");
    let session = ssh(setup).await?;
    let ax = tcc_row(&session, SERVICE_ACCESSIBILITY).await;
    let fda = tcc_row(&session, SERVICE_FDA).await;
    eprintln!("  Accessibility:    {}", describe_grant(ax));
    eprintln!("  Full Disk Access: {}", describe_grant(fda));
    if grant_ok(ax) && grant_ok(fda) {
        eprintln!("  Both TCC grants verified (auth_value=2, non-empty csreq).");
        Ok(())
    } else {
        Err(VmError::GoldenCreateFailed {
            detail: format!(
                "TCC verification failed ({when}): Accessibility={}, Full Disk Access={}",
                describe_grant(ax),
                describe_grant(fda)
            ),
        })
    }
}

/// Run one verification `SELECT` and parse its row.
async fn tcc_row(session: &SshSession, service: &str) -> Option<(i64, i64)> {
    let cmd = format!("sudo sqlite3 \"{TCC_DB}\" \"{}\"", tcc_verify_sql(service));
    let out = session.exec(&cmd).await.ok()?;
    parse_tcc_row(&out.stdout)
}

/// Human-readable grant state for the narration / error detail.
fn describe_grant(row: Option<(i64, i64)>) -> String {
    match row {
        Some((auth, len)) => format!("auth_value={auth}, csreq={len} bytes"),
        None => "missing".to_string(),
    }
}

/// Final health gate: the agent (launched by launchd) must respond on
/// `localhost:8648`. The agent binds guest-localhost, so this `curl`s **inside**
/// the guest over SSH rather than reaching it from the host (script lines
/// 746–760). Fatal on timeout.
async fn verify_agent_health(setup: &SetupVm) -> Result<(), VmError> {
    eprintln!("Checking agent health on port 8648...");
    let session = ssh(setup).await?;
    for attempt in 0..AGENT_HEALTH_ATTEMPTS {
        if let Ok(out) = session.exec("curl -sf http://localhost:8648/health").await {
            if out.exit_code == 0 {
                eprintln!("  Agent is running and healthy on port 8648.");
                return Ok(());
            }
        }
        if attempt + 1 < AGENT_HEALTH_ATTEMPTS {
            tokio::time::sleep(AGENT_HEALTH_INTERVAL).await;
        }
    }
    Err(VmError::GoldenCreateFailed {
        detail: "agent did not respond on http://localhost:8648/health within the health window \
                 — check launchd and the agent log on the setup VM"
            .into(),
    })
}

/// Clean the desktop and shut the setup VM down cleanly before the clone (script
/// lines 763–803). Best-effort throughout — a stop that won't complete cleanly
/// falls through to a force `tart stop`.
async fn finalize_and_shutdown(setup: &SetupVm) {
    eprintln!("Cleaning desktop state...");
    if let Ok(session) = ssh(setup).await {
        // The vanilla image boots with Terminal open; kill it and clear saved
        // state so the golden boots to a clean desktop.
        let _ = session.exec("killall Terminal 2>/dev/null || true").await;
        tokio::time::sleep(Duration::from_secs(2)).await;
        let _ = session
            .exec("rm -rf ~/Library/Saved\\ Application\\ State/* 2>/dev/null || true")
            .await;

        // Disable Remote Login + clean shutdown in one shell. `setremotelogin
        // off` unloads sshd and may kill this very session, so the shutdown is
        // queued after it in the same command — a dropped session is expected,
        // hence the tolerant `let _`. System Events shutdown (not `shutdown -h`)
        // so loginwindow records that no apps are open (no relaunch on the
        // golden's first boot).
        eprintln!("Disabling Remote Login and shutting down (clean, via System Events)...");
        let _ = session
            .exec(
                "sudo systemsetup -f -setremotelogin off >/dev/null 2>&1; \
                 osascript -e 'tell application \"System Events\" to shut down'",
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

/// `tart clone <setup> <golden>` then `tart delete <setup>` (script lines
/// 807–809). The clone is the one fatal step here — a failed clone means no
/// golden. A failed delete only leaks the (stopped) setup VM, so it warns.
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
    fn tcc_insert_sql_carries_the_grant_shape() {
        let sql = tcc_insert_sql(SERVICE_ACCESSIBILITY);
        assert!(sql.contains("INSERT OR REPLACE INTO access"));
        assert!(sql.contains("'kTCCServiceAccessibility'"));
        assert!(sql.contains("'/usr/local/bin/testanyware-agent'"));
        // client_type=1, auth_value=2 (the allow values), shared csreq var.
        assert!(sql.contains("1, 2, 0, 1, X'$CSREQ_HEX'"));
        assert!(sql.contains("CAST(strftime('%s','now') AS INTEGER)"));
    }

    #[test]
    fn tcc_insert_sql_only_the_service_differs() {
        let ax = tcc_insert_sql(SERVICE_ACCESSIBILITY);
        let fda = tcc_insert_sql(SERVICE_FDA);
        assert!(ax.contains("'kTCCServiceAccessibility'"));
        assert!(fda.contains("'kTCCServiceSystemPolicyAllFiles'"));
        // Same client path, same column list, same allow values.
        assert_eq!(
            ax.replace("kTCCServiceAccessibility", "X"),
            fda.replace("kTCCServiceSystemPolicyAllFiles", "X")
        );
    }

    #[test]
    fn grant_tcc_command_computes_csreq_then_inserts_both() {
        let cmd = grant_tcc_command();
        // csreq blob pipeline.
        assert!(cmd.contains("codesign -dr- /usr/local/bin/testanyware-agent"));
        assert!(cmd.contains("csreq -r- -b /dev/stdout"));
        assert!(cmd.contains("xxd -p"));
        assert!(cmd.contains("CSREQ_HEX=$("));
        // Both grants inserted, chained on success of the csreq computation.
        assert!(cmd.contains("'kTCCServiceAccessibility'"));
        assert!(cmd.contains("'kTCCServiceSystemPolicyAllFiles'"));
        assert_eq!(cmd.matches("sudo sqlite3").count(), 2);
        assert!(cmd.contains("&&"));
    }

    #[test]
    fn tcc_verify_sql_selects_auth_value_and_csreq_length() {
        let sql = tcc_verify_sql(SERVICE_FDA);
        assert!(sql.contains("SELECT auth_value, length(csreq) FROM access"));
        assert!(sql.contains("service='kTCCServiceSystemPolicyAllFiles'"));
        assert!(sql.contains("client='/usr/local/bin/testanyware-agent'"));
    }

    #[test]
    fn parse_tcc_row_reads_pipe_separated_values() {
        assert_eq!(parse_tcc_row("2|123\n"), Some((2, 123)));
        assert_eq!(parse_tcc_row("  0|0 "), Some((0, 0)));
        // Only the first row is consulted.
        assert_eq!(parse_tcc_row("2|45\n2|45\n"), Some((2, 45)));
    }

    #[test]
    fn parse_tcc_row_is_none_for_missing_or_garbage() {
        assert_eq!(parse_tcc_row(""), None); // no grant row
        assert_eq!(parse_tcc_row("\n"), None);
        assert_eq!(parse_tcc_row("denied"), None); // no `|`
        assert_eq!(parse_tcc_row("x|y"), None); // unparsable numbers
    }

    #[test]
    fn grant_ok_requires_allowed_and_nonempty_csreq() {
        assert!(grant_ok(Some((2, 200)))); // allowed, has csreq
        assert!(!grant_ok(Some((2, 0)))); // allowed but empty csreq
        assert!(!grant_ok(Some((0, 200)))); // denied
        assert!(!grant_ok(Some((1, 200)))); // not the allow value
        assert!(!grant_ok(None)); // missing
    }

    #[test]
    fn sip_status_matches_is_case_insensitive_and_unambiguous() {
        let disabled = "System Integrity Protection status: disabled.";
        let enabled = "System Integrity Protection status: enabled.";
        assert!(sip_status_matches(disabled, "disabled"));
        assert!(sip_status_matches(enabled, "enabled"));
        // No false positives across the two states.
        assert!(!sip_status_matches(disabled, "enabled"));
        assert!(!sip_status_matches(enabled, "disabled"));
        // Case-insensitive.
        assert!(sip_status_matches("STATUS: DISABLED", "disabled"));
    }
}
