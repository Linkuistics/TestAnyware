# checks-27-k16


## Goal

In the macOS 27 golden (`testanyware-golden-macos-27`), the two checks the human
named pass: the Display / HiDPI behaviour (ADR-0014, ADR-0015, ADR-0016: the
guest resolution switch and the HiDPI build) and the accessibility agent (the
macOS In-VM agent's permissions and its window and element queries). Fix what
breaks in the CLI or agent, or cut a leaf for what is not this leaf's. Confirm
the VNC keymap (Command = XK_Alt_L 0xffe9, Option = XK_Meta_L 0xffe7) as a side
effect of driving the guest, and fix it only if it blocks the checks.

## Context

- **The golden exists.** `testanyware-golden-macos-27` was built by
  `golden-from-27-base-k15` and a clone reaches a Healthy agent
  (`/health` = `{"accessible":true,"platform":"macos"}`). The base
  `testanyware-base-macos-27` and `testanyware-golden-macos-tahoe` are intact.
- **Reaching a guest from this harness.** Non-Apple binaries cannot reach a
  guest's `192.168.64.x` from the harness (Local Network privacy, not proven;
  memory `project-harness-local-network-blocked`). `curl`, `nc` and the system
  `ssh` do reach it, and VNC is on loopback and works. So `vm start` (its agent
  wait is russh) has to be run by the human in their terminal; after that the
  agent can be driven from here with `curl` against `http://<ip>:8648`
  (`/health`, `/windows`, `/snapshot`, `/inspect`, `/exec`; body for `/exec` is
  `{"command": "..."}`). Ask for the start, keep the clone running, stop it
  with `testanyware vm stop <id>` (it deletes the clone). Run the branch binary
  `cli-rs/target/debug/testanyware`, not the installed Homebrew one, which
  predates the IPSW work.
- **One macOS VM at a time.**
- **Already observed on 27, nothing more:**
  - `vm start` set the display to 1920x1080 pt @ 1x ("switched main display to
    1920x1080 pt @ 1x (1920x1080 px, modeID 10)"). That is the plain resolution
    switch only. The HiDPI build (ADR-0015, ADR-0016) is not exercised yet.
  - The agent's `/windows` query returned windows with titles and geometry.
    `/snapshot`, `/inspect`, `/press` and the rest are not exercised yet.
  - **Desktop widgets still draw.** Boot 1 writes `com.apple.WindowManager
    StandardHideWidgets = 1` (read back as `1`), but `/windows` lists
    Notification Center widget windows (Featured, Forecast, Month, 180 to 360 pt
    squares at the top left). On 27 the key does not hide them. It affects
    screenshots and OCR, so decide whether to find the 27 key (fix in
    `golden.rs`, then a golden rebuild) or note it; a rebuild is about 20
    minutes in the human's terminal.
- **Golden creation changed in k15.** `finalize.rs` no longer reads `TCC.db`
  with SIP on (macOS 27 refuses: `authorization denied`), and the health gate now
  requires `accessible: true`. The Tahoe golden has not been rebuilt since, so
  that stricter gate is unproven on the Tahoe path.

## Done when

- The Display / HiDPI check passes in a clone of the 27 golden (the resolution
  switch and the HiDPI build, as the three ADRs define them), or the break is
  fixed or cut as a leaf.
- The accessibility check passes: the agent reports `accessible: true`, a window
  query lists real windows, and an element query (`/snapshot` or `/inspect`)
  returns the tree of an app window. A press or focus action works on one
  element.
- The keymap is noted as confirmed or the failure is fixed.
- The widget question is decided and recorded.
- No clone or setup VM is left running.
- Last act: cut `default-and-docs-27` (impl), carrying what you learned. It asks
  the human whether 27 becomes the default.

## Notes

- Guests are driven only through the In-VM agent and VNC. Never run
  `set-display-mode.swift` on the host; compile on the host, execute in the guest.
- If no human is reachable to run `vm start`, stop and leave the leaf live.

<!-- tc: exempt because the sections above this line predate the session, and the finish cycle deletes this file -->

## Decisions (running log)

Settled in this session, 2026-10-18. A clone of `testanyware-golden-macos-27`
ran every check. The host is a 1x 5120x2160 display.

- **Display, plain resolution switch: passes on 27.** `set-display-mode 1920 1080 1`
  returned "switched main display to 1920x1080 pt @ 1x (1920x1080 px, modeID 10)".
  `screen size` then read 1920x1080. The guest advertises 12 modes, all at scale 1.0.
- **Display, HiDPI `@2x`: not run live, and not a break.** The 2x selector exits 2
  with "no 2x 1920x1080 (3840x2160 px) mode advertised" and changes nothing. That
  is the degrade path ADR-0015 and ADR-0016 specify for a 1x host. A live run needs
  the host switched to its 2.00x mode. The leaf did not switch the host display.
  `default-and-docs-27-k18` asks the human. The Rust workspace tests pass
  (595 passed, 0 failed, 12 ignored live gates).
- **Accessibility: passes on 27.** `/health` reports `accessible: true`. `/windows`
  lists TextEdit and Calculator with titles, focus and geometry. `/snapshot`
  returns the Calculator tree. `/press` on ids `Seven` and `Eight` made the display
  read `78`. `/focus` and `/set-value` work on the TextEdit text area.
- **Agent bug, fixed: a unified `role` never matched.** `{"role":"button"}` in
  `/inspect` returned `not_found`. `{"role":"AXButton"}` worked. The three handlers
  passed the wire role through the platform-name table, so `button` became
  `.unknown`. The protocol document says `role` is a unified name. The fix is
  `RoleMapper.queryRole`, which reads a unified name first and still accepts a
  platform name. It is not specific to macOS 27. Four new tests fail on the old
  code and pass on the fix. Not run live: the 27 golden carries the old agent, so
  the fix reaches it on the next golden rebuild. `default-and-docs-27-k18` asks.
- **Keymap: confirmed on 27.** Over VNC, Command+A then typing `Z` replaced the
  text, Option+A typed `å`, and Command+Z restored the text. No change needed.
- **Widgets: no change.** The leaf brief said the widgets still draw. They do not.
  A screenshot with `StandardHideWidgets = 1` has no widgets. With the setting
  off, the same screenshot has Calendar, Weather and Photos widgets at the
  geometry `/windows` reports (positive control). `/windows` lists those windows
  in both states, so it is not evidence of drawing. `docs/user/golden-images.md`
  already says widgets are disabled.
- **New fault, externalised: tart aborts on a VNC reconnect after an idle gap.**
  tart 2.40.1 on macOS 27.0.1 aborts in `_VZVNCServer` when a client connects
  after about two minutes with no client. Four of five clones crashed. It needs no
  display switch. The VM survives the gap, and the next connection kills it. The
  evidence and a draft upstream report are in
  `docs/research/tart-vnc-reconnect-crash-macos-27.md`. The cause is not isolated.
  The mitigation is a product question, so it is leaf `vnc-reconnect-crash-27-k17`.
  Filing upstream stays with the human.
- **Correction to the notes: `vm start` runs from this harness.** The Context said
  the human must run it. It works here. It waits its whole boot window (about 4
  minutes), warns that the agent did not reach health, and exits 0. The clone is
  healthy and `curl` reaches the agent. Only the Rust agent probe fails.
- **Taskfile: one recipe added.** `task agent:macos:test` builds the macOS agent and
  runs its Swift tests. It ran green, 47 tests.
- **No clone left running.** `vm list` reports none. The base, the 27 golden and the
  Tahoe golden are intact.
