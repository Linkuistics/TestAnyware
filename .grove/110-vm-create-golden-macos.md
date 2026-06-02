# 110-vm-create-golden-macos

**Kind:** work

## Goal

Add `testanyware vm create-golden --platform macos` as a **full Rust port** of
`provisioner/scripts/vm-create-golden-macos.sh` (819 lines), and **delete that
script**. First platform of the per-platform golden command (linux/win are
Tier 2).

## Context

- Decision (070, Q3): **full Rust port**, not a façade over the scripts (user
  override; the external scripts go away). Memory [[golden-creation-in-cli]].
- Builds on existing `testanyware-vm` infra: `tart.rs` (tart runner from the
  retired `050-live-vm-gate`), `qemu.rs`, `lifecycle.rs`, `health.rs`,
  `process.rs`, `paths.rs`.
- **Net-new work** (no equivalent in `cli-rs` yet): ssh provisioning
  orchestration, boot-wait / recovery-mode sequencing, and the macOS specifics —
  the SIP disable/enable **recovery-boot cycle** and **TCC `sqlite` grants**
  (`kTCCServiceAccessibility`, `kTCCServiceSystemPolicyAllFiles`). The script is
  a 5-boot sequence (3 normal + 2 recovery); preserve that orchestration.
- **Decide the ssh strategy** here: an ssh crate (e.g. `ssh2`/`russh`) vs
  `Command::new("ssh")`. Note memory [[vm-ssh-from-harness]] gotchas
  (backgrounded ssh + askpass; `COPYFILE_DISABLE` for tar).
- Surface: add a `vm-create-golden` entry to `surface.rs::CANONICAL_COMMANDS`
  (`mutating: true`) and a schema — **not present today**. `--platform <p>`,
  `--version`, `--name` (match the script's flags).

## Done when

- `vm create-golden --platform macos` produces a golden image functionally
  equivalent to the script's output (agent LaunchAgent on 8648, TCC granted,
  SSH off, verified healthy), verified by actually creating one on the Mac
  (cheap — memory [[vm-costs]]).
- Satisfies the **CLI design contract**: `--json`, `--dry-run` (dry-run must
  describe the boot plan without mutating), stable error codes, help template.
- `provisioner/scripts/vm-create-golden-macos.sh` deleted (or staged for the
  `130` cli/-delete sweep); release bundling updated if it referenced the script.

## Notes

- Almost certainly a **node** (`leaf-decompose`): ssh-provisioning layer,
  boot/recovery orchestration, and TCC/SIP specifics are plausibly separate
  sessions. Decompose lazily once into it.
- Cmd-key/VNC keysym quirks (memory [[cmd-key-tahoe]]) are not in this path
  (ssh-driven), but the agent-health check at the end uses RFB.
