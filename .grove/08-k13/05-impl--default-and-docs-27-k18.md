# default-and-docs-27-k18


## Goal

Ask the human whether macOS 27 becomes the default macOS golden. Then make the
code and docs match the answer.

## Context

- **What the checks found.** `checks-27-k16` ran every check on a clone of
  `testanyware-golden-macos-27`. All passed on this 1x host, with two limits
  below.
  - Display: the 1x resolution switch works (1920x1080, mode 10). The guest
    advertises 12 modes, all at scale 1.0, the same list as Tahoe.
  - Accessibility: the agent reports `accessible: true`. `/windows` lists real
    app windows. `/snapshot` returns the Calculator tree. `/press` on `7` then `8`
    gave a display of `78`. `/focus` and `/set-value` work on TextEdit.
  - Keymap: Command+A, Option+A (typed `å`) and Command+Z all work over VNC.
  - Widgets: `StandardHideWidgets = 1` works on 27. The screenshot has no widgets.
    With the setting off, the same screenshot contains them (positive control).
    `/windows` still lists the three Notification Center widget windows while
    they are hidden. Nobody changed that.
- **Limit 1: HiDPI `@2x` was not run live.** This host's display is a 1x 5120x2160
  ultrawide. A Retina mode needs the host at its 2.00x mode (ADR-0016). The
  scale-2 selector exits 2 with a clear reason and changes nothing. The Rust unit
  tests for the scale-aware surface pass. To run it live, the human must switch
  the host display for a few minutes. Ask whether they want that. Never switch the
  host display without them.
- **Limit 2: a VNC crash.** tart 2.40.1 aborts when a VNC client connects after
  an idle gap of about two minutes. See
  `docs/research/tart-vnc-reconnect-crash-macos-27.md` and leaf
  `vnc-reconnect-crash-27-k17`. Tell the human what that leaf found.
- **An agent fix is not in the golden.** `role: "button"` in `/inspect`,
  `/snapshot` and the action endpoints never matched, because the agent read the
  value as a platform name. `checks-27-k16` fixed the source
  (`RoleMapper.queryRole`, with tests). The 27 golden still carries the old
  agent. A fix reaches it only on a golden rebuild (about 20 minutes in the
  human's terminal). Ask whether to rebuild now or at the next golden change.
  The Tahoe golden has the same gap.
- **The default is in one place.** `Platform::default_base` in
  `cli-rs/crates/testanyware-vm/src/lifecycle.rs` returns
  `testanyware-golden-macos-tahoe`, and a unit test in the same file pins it.
- **Docs that name Tahoe or macOS 26.** `README.md` (around line 466),
  `docs/user/golden-images.md`, `docs/components/provisioner.md`,
  `docs/architecture/vm-lifecycle.md` and
  `docs/reference/cli-schemas/vm-create-golden.json`. The website copies are in
  the repo's website sources. Search for them. Do not trust this list.
- **Running a clone from this harness.** `vm start` works here. It waits about 4
  minutes, warns that the agent did not reach health, and exits 0 with a healthy
  clone. Drive the agent with `curl` on `http://<ip>:8648`.

## Done when

- The human has answered whether 27 becomes the default. If no human is
  reachable, stop and leave this leaf live. Do not change the default on an
  assumed answer.
- If yes, `Platform::default_base`, its test, and every doc that names macOS 26
  or Tahoe as the macOS golden are updated. If no, the docs say how to select
  the 27 golden (`--base testanyware-golden-macos-27`).
- `testanyware-golden-macos-tahoe` is still present.
- The human has answered both questions in Context: the live HiDPI run, and the
  golden rebuild for the agent fix. A rebuild, if wanted, runs in their terminal.
- `task cli:test` passes.
- No clone or setup VM is left running (`vm list`).

## Notes

- Guests are driven only through the In-VM agent and VNC.
- Never replace or delete `testanyware-golden-macos-tahoe` without asking.
- Never run `set-display-mode.swift` on the host.
