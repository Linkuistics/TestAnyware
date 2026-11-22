# first-frame-size-k9

## Goal

Decide whether TestAnyware must handle a stale frame size at the first VNC
connection after a tart boot. Measure how often it happens on the latest host.
If the design keeps a fix, insert its implementation leaf ahead of the release
leaf.

## Context

- `keeper-removal-code-k5` removed the VNC keeper. From 2.2.0 to 2.9.0 the
  keeper was the first client of each tart guest, so no command was ever that
  first client.
- On 2026-11-22, `task vm:vnc-idle-live` passed on the development bundle. Its
  macOS guest switched to 1920x1080 in `vm start`. Its first `screen size`
  then reported 1280x720.
- Minutes later, a second macOS guest from the same golden reported 1920x1080
  at once, after 5 s and after 60 s.
- On 27.0.1 the first connection after a boot once saw a 1280x720 frame too.
  The idle reproduction connects to a fresh clone with no display switch. On
  27.2.1 it saw 1024x768, the display size of the golden.
- `task vm:vnc-idle-live` now prints the first and the second `screen size`
  of its macOS guest. Its next run reported 1280x720 and then 1920x1080, 5 s
  later. So two of three macOS guests reported the stale size at the first
  connection.
- On 2026-11-22, `setup-assistant-zrle-fault-k10` recorded a fresh
  27.2.1 boot on the wire. ServerInit gave 1280x720, and a `DesktopSize`
  change to 1024x768 followed some seconds later. So tart's VNC server
  itself reports the stale size first.
- On 2026-11-23, `macos-golden-refresh-run-k13` recorded Recovery boots.
  A capture at the stale size timed out and left its request open. After
  the resize, the server answered that request with its old region as the
  header. In 2 of 8 boots the tiles covered the whole new framebuffer, and
  the decoder failed. k13 changed the decoder, and its log holds the
  evidence.

## Done when

- A measurement on the latest host counts the first `screen size` after
  `vm start` over several macOS guests. It also records the second size, and
  a screenshot taken first.
- The design states whether the stale size reaches a user command. If it
  does, the design states the fix and the test that proves it, and it inserts
  the implementation leaf ahead of the release leaf. Otherwise it records why
  no fix is needed.

## Notes

- The stale size may come from tart's VNC server or from the guest's display
  switch in `vm start`. The design names which one before it chooses a fix.
- No backward compatibility. A fix must not bring back a process that holds a
  VNC connection between commands (ADR-0018).
