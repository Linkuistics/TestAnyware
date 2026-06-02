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
