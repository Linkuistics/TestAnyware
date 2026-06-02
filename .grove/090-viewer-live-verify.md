# 090-viewer-live-verify

**Kind:** work (live verification)

## Goal

Live **macOS-host GUI** verification of the embedded viewer's auto-reconnect +
`vm start --viewer` sugar (the retired `060-egui-viewer` leaf `030`). The one
pending follow-up from the previous wave; must not be lost.

## Context

Carried over from the retired `060-egui-viewer` node (ADR-0005) and promoted into
the root brief. The viewer code is built & committed (render loop, input
forwarding, reconnect + start-sugar — commits `42f8f1f`, `17c8bcc`); only the
**live macOS-host GUI** check is outstanding. Requires a human at the Mac with a
live VM — it is the *display surface* path that headless gates can't cover.

## Done when

- Verified on a macOS host: (1) `testanyware viewer` opens a window rendering the
  guest framebuffer; (2) bouncing the VM (`vm stop` then `vm start`) makes the
  viewer **auto-reconnect**; (3) `vm start --viewer` opens the viewer inline
  after the `vm-start` envelope.
- Result recorded in this leaf in the style of leaf `020`'s verify log
  (`.grove/done/060-egui-viewer/BRIEF.md`).
- Any regression found → file a follow-up leaf (don't fix inline unless trivial).

## Notes

- VM cost is just clone+start (memory [[vm-costs]]) — cheap.
- `tart list` state column, not `tart ip`, for running/stopped (memory
  [[tart-ip-lies]]).

## Verification log

**Live-verified 2026-06-03** on the macOS primary host (Darwin 25.5.0, arm64)
against fresh `testanyware-golden-macos-tahoe` clones, using the release binary
(`testanyware 0.0.1`, rebuilt at HEAD `bebb0fb`). Driven by the agent itself:
the host display surface was captured with `screencapture -x -l<windowid>`
(window located via Quartz `CGWindowListCopyWindowInfo`, owner `testanyware`,
title "TestAnyware viewer") and the PNGs read back to confirm rendering — the
display-surface path headless gates can't cover.

- **(1) Window renders the guest framebuffer — PASS.** `testanyware viewer --vm
  viewer-verify` opened a 1024×800 window rendering the live guest: macOS menu
  bar (Apple logo, Finder/File/Edit/View/Go/Window/Help, Spotlight, Control
  Centre, clock), desktop, and full Dock. VM booted to VNC-ready in ~14s.

- **(2) Bounce → auto-reconnect — PASS (mechanism), with a regression found.**
  With the viewer streaming, a `vm stop` dropped the connection: the window
  stayed up and painted the "No spec found for VM id 'viewer-verify'" overlay
  over the last frame (reconnect loop active, not a crash). A same-id `vm start`
  then brought the guest back and the viewer **auto-reconnected on its own** —
  overlay cleared, live rendering resumed (clock advanced across the bounce,
  confirming a fresh stream), without restarting the viewer process. The
  "productive drop" budget reset (`viewer.rs:286`) kept it retrying.
  - **REGRESSION (filed → leaf `150-tart-restart-stale-vnc`):** the *default*
    bounce (`vm stop` then `vm start`, same id) is broken upstream of the
    viewer. The per-VM tart log is append-only and `vm stop` doesn't clear it, so
    `poll_vnc_url` resolves the **previous** run's now-dead `vnc://` endpoint and
    writes it into the spec; the viewer (correctly) connects to a dead port,
    gets `Connection refused (os error 61)`, and after the 12-attempt budget
    paints the terminal "gave up" overlay. Confirmed root cause by clearing the
    stale log between stop/start — the spec then matched the live `lsof` port
    (58374, 58375) and reconnect succeeded. Check (2) above was verified with
    that log-clear workaround; the viewer's own reconnect logic is sound.

- **(3) `vm start --viewer` sugar — PASS.** With a fresh auto-generated id
  (`testanyware-da4788a5`, unaffected by the log bug), `vm start --platform
  macos --viewer --json` emitted the full `vm-start` envelope (id, vnc
  127.0.0.1:58376, agent) to stdout **first**, then opened the viewer inline
  rendering the guest — matching ADR-0005 Q2 (data envelope before the blocking
  window).

**Net:** leaf 030's viewer behaviour (render, auto-reconnect, start-sugar) is
confirmed live on macOS. One upstream regression in the tart same-id restart
path was discovered and filed as `150` rather than fixed inline (different
crate, shared `spawn_detached`, real design choice).
