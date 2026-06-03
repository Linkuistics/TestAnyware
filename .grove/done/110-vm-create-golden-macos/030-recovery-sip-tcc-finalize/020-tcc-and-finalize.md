# 020-tcc-and-finalize

**Kind:** work

## Goal

Wire the full `vm create-golden --platform macos` pipeline end-to-end on top of
the `010` recovery driver: the SIP/TCC orchestration, the TCC sqlite grants, the
final agent-health gate, the clean shutdown, and `tart clone` to the golden.
Then **delete the source script** and confirm parity. Finishes node `110`.

## Context

Ports the script's top-level SIP/TCC cycle + finalize (`vm-create-golden-macos.sh`
lines 604–818) over the existing `russh` `SshSession`. Mechanical relative to
`010` — almost all of it is SSH commands and verification, no RFB/OCR.

Today `commands/vm.rs::run_vm_create_golden` stops after `provision_boot1` with a
"not yet wired" handoff message (it leaves the setup VM running and reports the
golden is *not* produced). This leaf removes that handoff and completes the flow.

## What to port (parity outcomes)

Top-level orchestration (script 677–723), order is fixed:
1. `recovery_boot_csrutil(setup, "csrutil disable")` (from `010`).
2. Verify SIP disabled: `csrutil status` over SSH contains "disabled".
3. `grant_tcc_permissions` (script 604–671): `sudo killall tccd`; generate the
   shared csreq blob (`codesign -dr- /usr/local/bin/testanyware-agent | sed …
   | csreq -r- -b /dev/stdout | xxd -p | tr -d '\n'`); `INSERT OR REPLACE` both
   grants (`kTCCServiceAccessibility`, `kTCCServiceSystemPolicyAllFiles`,
   `client_type=1`, `auth_value=2`, shared `X'<csreq>'`) into
   `/Library/Application Support/com.apple.TCC/TCC.db`; `sudo killall tccd`
   again to flush the cache. All over SSH (SIP must be disabled first).
4. Verify both TCC rows: `SELECT auth_value, length(csreq) …` → `2|<len>` with
   `len > 0` for both.
5. `recovery_boot_csrutil(setup, "csrutil enable")`; verify SIP re-enabled.
6. Verify both TCC rows survive SIP re-enable (`auth_value=2`).

Final health gate (745–761): agent responding on `localhost:8648`. Decide
between `health::wait_for_agent` against the guest vs curl-over-ssh for parity —
note the script curls *inside* the guest because the agent binds localhost; pick
whichever actually reaches it (likely curl-over-ssh, or an ssh port-forward).

Finalize (763–818):
- Clean desktop: `killall Terminal`; clear `~/Library/Saved Application State/*`.
- Disable Remote Login + clean shutdown in one SSH call: `sudo systemsetup -f
  -setremotelogin off; osascript -e 'tell application "System Events" to shut
  down'` (NOT `shutdown -h now` — would relaunch apps on next boot; script 776–788).
  The setremotelogin may kill the SSH session — that's expected, mask the error.
- Wait for the tart process to exit (force-stop fallback).
- `tart clone <setup> <golden>`; `tart delete <setup>`.

End-to-end wiring:
- Rewrite `run_vm_create_golden` to run `provision_boot1` → the SIP/TCC cycle →
  finalize → clone, removing the handoff stub and the "golden not produced"
  caveat. On success it reports the real golden name.
- Cleanup-on-failure: the setup VM is torn down on any hard failure (extend the
  `provision_boot1` cleanup guard to span the whole pipeline).

## Done when

- `vm create-golden --platform macos` runs end-to-end and produces a golden
  **verified by actually creating one on the Mac** ([[vm-costs]]): agent
  LaunchAgent healthy on 8648, both TCC grants `auth_value=2` with non-empty
  csreq, SIP re-enabled, Remote Login off. Record the run.
- `--dry-run` still describes the full 5-boot plan without mutating (already in
  `vm.rs::emit_golden_plan`); stable error codes + help template; `cli-contract.rs`
  passes for `vm-create-golden`.
- `provisioner/scripts/vm-create-golden-macos.sh` **deleted**; grep the repo for
  references (release/bundling scripts, docs) and update them. Helpers the script
  SCP'd (`set-wallpaper.swift`, the agent plist) are already embedded in
  `golden.rs` — confirm nothing else depends on the script path.
- `cargo test` green; `zig cc` cross-check unaffected.

## Notes

- This is the node `110` finisher. On completion, walk the retire cascade: this
  sub-node `030`, then node `110` (ask the user before retiring `110` — there may
  be a follow-up), promoting anything durable from the briefs upward.
- The script's own success banner points at `scripts/test-integration.sh --base
  <golden>` — sanity that the produced golden is consumable there is a nice extra
  check but not required for done.
