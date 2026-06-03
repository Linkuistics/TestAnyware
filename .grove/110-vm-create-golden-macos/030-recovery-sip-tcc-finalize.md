# 030-recovery-sip-tcc-finalize

**Kind:** planning

## Goal

Design and build the **re-engineered recovery automation** (node decision Q3),
then the SIP disable/enable cycle, the TCC sqlite grants, the final agent-health
gate, and the clean shutdown + `tart clone` to the golden image. Finishes the
node and deletes the source script.

## Why this is a planning leaf

The node grilling chose to **re-engineer the recovery flow** rather than
transliterate the script's blind fixed sleeps. So this leaf **opens with a
design pass** (grill the state machine), then implements it. It may decompose
further if the state machine is large (e.g. split "recovery driver" from
"TCC/finalize").

## The re-engineering brief (what to design)

Replace the script's `_recovery_boot_csrutil` (lines 398–594) — a sequence of
`testanyware` *subprocess* calls separated by `sleep 15/10` — with an
**in-process observe/act/verify state machine** over `testanyware-rfb` +
`testanyware-ocr-client`:

- **Macro orchestration stays at parity** (do NOT change): boot `--recovery
  --no-graphics --vnc-experimental`; navigate the startup picker
  (Right→Right→Enter); reach recovery desktop; open Terminal via the Utilities
  menu (OCR-located, press-hold-drag to bypass the modal); run the csrutil
  command; answer y / username / password; `halt`; reboot normally; wait SSH.
  The 5-boot sequence and the disable-SIP → grant-TCC → enable-SIP order are
  fixed (script lines 677–723).
- **Micro mechanism is re-engineered:** each step becomes
  observe (poll RFB framebuffer → OCR) → assert expected screen → act (RFB
  input) → verify the transition occurred → retry-with-backoff on miss, with a
  bounded overall deadline. Replaces blind sleeps with signal-driven waits where
  a signal exists; where none exists (csrutil's y/user/pass prompts echo
  nothing reliable), design the most robust available proxy (e.g. screen-diff
  settle detection) rather than a fixed sleep — this is the open design question
  to grill.
- In-process API: `RfbConnection::connect` to the recovery VNC endpoint (parsed
  from the `tart run` log as in `010`/`tart.rs::parse_vnc_url`), then
  `type_text`/`key_event`/`mouse_down`/`mouse_move`/`mouse_up`; `OcrEngine::
  detect` + `find_text` for "Options"/"Utilities"/"Terminal" location. Note
  VNC keysym quirks [[cmd-key-tahoe]] apply on this RFB path.

## Then implement (parity outcomes)

- `grant_tcc_permissions` (script 604–671): stop `tccd`, generate the shared
  csreq blob via `codesign -dr- | csreq | xxd`, `INSERT OR REPLACE` both grants
  (`kTCCServiceAccessibility`, `kTCCServiceSystemPolicyAllFiles`) into
  `/Library/Application Support/com.apple.TCC/TCC.db`, restart `tccd`. Verify
  both rows (`auth_value=2`, csreq length > 0) — over the `010` SSH layer.
- Verify SIP disabled / re-enabled via `csrutil status` (683–723).
- Final health gate: agent responding on `localhost:8648` (745–761) — the script
  curls inside the guest; the Rust port can reuse `health::wait_for_agent`
  against the guest, or curl-over-ssh for parity. Decide.
- Clean desktop, disable Remote Login (`systemsetup -setremotelogin off`), clean
  shutdown via System Events `shut down` (NOT `shutdown -h now` — saves session
  state) (763–803).
- `tart clone <setup> <golden>` + `tart delete <setup>` (805–812).

## Done when

- `vm create-golden --platform macos` end-to-end produces a golden functionally
  equivalent to the script: agent LaunchAgent healthy on 8648, both TCC grants
  present with `auth_value=2`, SIP re-enabled, SSH off — **verified by actually
  creating a golden on the Mac** ([[vm-costs]]).
- `--dry-run` describes the full 5-boot plan without mutating; contract codes +
  help template satisfied; `cli-contract.rs` passes.
- `provisioner/scripts/vm-create-golden-macos.sh` **deleted**; any release
  bundling reference updated. Update `CONTEXT.md` if a "golden creation" term
  warrants an entry.

## Notes

- If the recovery state machine turns out large, `leaf-decompose` this into
  `010-recovery-driver` + `020-tcc-and-finalize` — lazily, only if needed.
- The recovery flow is the highest-risk part of the whole node; budget live-VM
  iteration. A failed recovery automation leaves the setup VM in a known state
  (SSH-reachable post-reboot), so re-runs are debuggable.
