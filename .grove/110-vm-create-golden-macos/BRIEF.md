# 110-vm-create-golden-macos — brief

## Goal

Add `testanyware vm create-golden --platform macos` as a **full Rust port** of
`provisioner/scripts/vm-create-golden-macos.sh` (819 lines), and **delete that
script**. First platform of the per-platform golden command (linux/win are
Tier 2).

## Done when (node)

- `vm create-golden --platform macos` produces a golden image functionally
  equivalent to the script's output (agent LaunchAgent on 8648, TCC granted —
  `kTCCServiceAccessibility` + `kTCCServiceSystemPolicyAllFiles`, SSH off,
  verified healthy on port 8648), verified by actually creating one on the Mac
  (cheap — memory [[vm-costs]]).
- Satisfies the **CLI design contract**: `--json`, `--dry-run` (describes the
  boot plan without mutating), stable error codes, help template.
- `provisioner/scripts/vm-create-golden-macos.sh` deleted (or staged for the
  `130` cli/-delete sweep); release bundling updated if it referenced the script.

## Decisions (running log)

Settled in the 110 decomposition grilling (2026-06-03):

- **Q1 — decomposition shape: 3-leaf node.** SSH provisioning, normal-boot
  provisioning, and recovery/SIP/TCC are separately-verifiable sessions. Leaves
  `010` (scaffold + SSH layer), `020` (normal-boot provisioning), `030`
  (recovery + SIP/TCC + finalize).
- **Q2 — SSH strategy: `russh` (pure-Rust, async).** Native password+pubkey auth
  removes the askpass fragility ([[vm-ssh-from-harness]]); no C dependency, so
  the grove's `zig cc` cross-compile path ([[linux-crosscheck-zig]],
  [[local-release-no-ci]]) and Tier-2 linux/win goldens reuse the same
  provisioning layer cleanly. Rejected `ssh2`/libssh2 (C dep fights cross-build,
  sync API in an async CLI) and `Command::new("ssh")` (askpass fragility, weak
  testability). **→ ADR-0007.**
- **Q3 — port fidelity: re-engineer the recovery flow** (user override of the
  recommended faithful-port). The **macro orchestration stays at parity** — the
  5-boot sequence (3 normal + 2 recovery), the disable-SIP → grant-TCC →
  enable-SIP order, the agent-health gate, the clean shutdown + clone. The
  **micro automation inside recovery is re-engineered** from blind fixed sleeps
  into a robust **observe/act/verify state machine** over the in-process RFB
  framebuffer + OCR (poll → assert expected screen → act → confirm transition →
  retry). This raises `030` to a **planning** leaf: it opens by designing that
  state machine before porting. Risk is bounded because the boot *sequence* and
  *outcomes* are unchanged — only the in-recovery *mechanism* changes.

## Decomposition

- `010-scaffold-and-ssh-strategy` (work) — surface entry + clap wiring + handler
  skeleton + `--dry-run` boot plan + the `russh` provisioning helper (exec +
  sftp upload, password-once → pubkey). The foundation every boot phase sits on.
- `020-normal-boot-provisioning` (work) — port boot 1 over the `010` SSH layer:
  pubkey install, macOS defaults, solid wallpaper, hide widgets, Xcode CLT,
  Homebrew, agent binary + LaunchAgent plist install. Net-new: clean boot/IP
  wait, graceful stop.
- `030-recovery-sip-tcc-finalize` (planning) — design + build the re-engineered
  recovery state machine (in-process RFB+OCR), the SIP disable/enable cycle, the
  TCC sqlite grants (shared csreq blob), final health gate, clean shutdown +
  `tart clone` to golden, and the script deletion. Decomposes further if the
  state machine warrants it.

## Pointers

- Source script: `provisioner/scripts/vm-create-golden-macos.sh` (819 lines) —
  the parity reference for the boot sequence and outcomes.
- Helpers the script SCPs: `provisioner/helpers/set-wallpaper.swift`,
  `provisioner/helpers/com.linkuistics.testanyware.agent.plist`.
- Existing infra to build on (`cli-rs/crates/testanyware-vm/`): `tart.rs`
  (`TartRunner`, `run_tart`, `parse_vnc_url`, `poll_vnc_url`, `poll_ip`),
  `process.rs` (`process_alive`, `terminate`, `pgrep_first`), `paths.rs`,
  `health.rs` (`wait_for_agent`). In-process recovery automation uses
  `testanyware-rfb` (`RfbConnection`: `connect`, `key_event`/`type_text`/
  `click`/`mouse_down`/`mouse_up`/`mouse_move`, `framebuffer`) and
  `testanyware-ocr-client` (`OcrEngine::detect`, `find_text`).
- Surface: add `vm-create-golden` to `surface.rs::CANONICAL_COMMANDS`
  (`mutating: true`, `data_producing: true`, `schema_id: "vm-create-golden"`) +
  a schema file — not present today. Flags: `--platform`, `--version`, `--name`.
- Contract: `docs/architecture/cli-design-contract.md`. Pattern to mirror:
  `commands/vm.rs::run_vm_start` (dry-run validate → emit plan → execute).
- Memory: [[golden-creation-in-cli]] (full-port decision), [[vm-costs]]
  (verification is cheap), [[tart-ip-lies]] (use `tart list` state, not
  `tart ip`), [[vm-ssh-from-harness]] (ssh/tar gotchas — largely retired by the
  russh choice), [[cmd-key-tahoe]] (VNC keysyms — only the recovery RFB path).

## Notes

- Cmd-key/VNC keysym quirks ([[cmd-key-tahoe]]) matter only for the `030`
  recovery RFB automation, not the ssh-driven boots.
- `vm create-golden` is **macOS-host-only** in this node (`#[cfg(target_os =
  "macos")]`, like `tart.rs`); linux/win goldens are Tier 2.
