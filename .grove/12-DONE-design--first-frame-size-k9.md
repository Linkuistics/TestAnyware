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

## Decisions (running log)

- 2026-11-23, code path. `screen size` prints the size of ServerInit, the
  first message of the server after the handshake. It sends no frame
  request. `screen capture` reads its crop size from ServerInit too, before
  it asks for the frame. `capture_frame` applies a `DesktopSize` change
  that arrives with the frame, but the crop stays at the old size. So a
  stale ServerInit gives a wrong `screen size`, and it can crop the first
  screenshot. `screen record` and the HiDPI scale check also read the size
  before the first frame. `vm start` opens no VNC connection. It sets the
  tart display to 1920x1080px, waits for the agent and switches the guest
  display through the agent.
- 2026-11-23, set-up. The installed 2.9.0 app still starts a VNC keeper, so
  the runs use the development bundle of commit 9a3194f1. I quit the
  installed app for the runs. The probe, the drivers and the records stay
  in scratch. The probe is a bare RFB client in Python that asks for Raw
  pixels and logs ServerInit and each update with its time. It shares no
  code with `testanyware-rfb`.
- 2026-11-23, batch 1. 14 guests from `testanyware-golden-macos-27`
  (27.2.1), one at a time, through the development app. Right after
  `vm start`, the first `screen size` or `screen capture` saw 1280x720 on 7
  of 8 guests. Every later read saw 1920x1080. After 60 s with no client,
  the first `screen size` saw 1280x720 on 1 of 2 guests. The direct Python
  probe, as the first client right after `vm start` or 90 s after a plain
  `tart run`, saw 1920x1080 on 4 of 4. `vm start` took 15 to 43 s.
- 2026-11-23, the first screenshot. Each stale first `screen capture` wrote
  a 1280x720 PNG. Three of four were the top-left 1280x720 of the real
  1920x1080 screen, in 0.5 to 0.8 s. In each, the server sent `DesktopSize`
  and a full frame, and the crop kept the old size. The fourth waited the 5 s
  capture bound and got no frame. It wrote the empty framebuffer, which
  is black. So a stale session can also leave a frame request unanswered for
  more than 5 s.
- 2026-11-23, the speed of the client. The Python probe computes the VNC
  password answer with an `openssl` process. That costs about 5 ms each time. A
  copy that starts `openssl` before it connects sent ClientInit 4 ms after
  the connect. As the first client after `vm start`, it saw 1280x720. So
  the path through the app is not the cause.
- 2026-11-23, a 1x1 request does not settle the size. In that run, a 1x1
  request 2 ms after ServerInit came back as one Raw pixel of the stale
  1280x720 surface, with no `DesktopSize`. The `DesktopSize` of 1920x1080
  came on its own within the next second. So a client cannot learn the
  size from one cheap request at once.
- 2026-11-23, batch 2. 12 more guests, with the fast probe and the CLI.
  The fast probe with no wait before ClientInit saw 1280x720 on 3 of 3
  guests. With 300 ms before ClientInit, it saw 1920x1080 on 3 of 3. So
  the speed of the first client decides. The same fast probe, 60 s after a
  plain `tart run` with no display switch, saw 1280x720 on 2 of 2. A Linux
  guest saw 1280x720 at its first `screen size` on 1 of 2. So the source
  is tart's VNC server, and the display switch of `vm start` is not
  needed for the fault.
- 2026-11-23, only the first session. After a first session, every later
  session of every guest read the real size. That includes a session
  0.06 s after a 5 s first session. A second display switch on a guest
  whose server already knew the size left the next read at 1920x1080, on
  2 of 2 guests. After 60 s with no client, the first `screen size` saw
  1280x720 on 3 of 4 guests, so waiting alone does not help.
- 2026-11-23, the resize can be slow. After `vm start`, the `DesktopSize`
  change reached a fast probe within 1 s on 3 of 3 guests. After a plain
  `tart run`, it came 9.2 s after ServerInit once, and not within 11 s
  once. On a still screen, a second full request on one session got no
  answer within 10 s once in three runs. So a wait that holds one session
  open for a resize has no short bound.
- 2026-11-23, batch 3, the fast poll. A fast probe opened short sessions
  250 ms apart, each closed after ServerInit, until ServerInit read
  1920x1080. On 7 of 7 guests, the first session read 1280x720 and the
  second, 0.26 to 0.27 s later, read 1920x1080. The 7 were three macOS and
  two Linux guests after `vm start`, and two plain `tart run` boots. Each
  `screen size` after the poll read 1920x1080. The digests of the probes
  and drivers matched before and after each batch.
- 2026-11-23, verdict. The stale size reaches user commands. On 7 of 8
  macOS guests, the first `screen size` or `screen capture` after
  `vm start` read 1280x720. By code reading, `screen record`, the HiDPI
  scale check and `input` read the same size. So TestAnyware must handle
  it.
- 2026-11-23, source. The source is the VNC server of tart, not the
  display switch of `vm start`. A plain `tart run` with no switch and a
  Linux guest had the fault, and a slow first client did not.
- 2026-11-23, fix. `vm start` ends with the VNC size wait on each tart
  guest. It opens short RFB sessions 250 ms apart until ServerInit reports
  the size that `vm start` set, for at most 30 s. It names a wait in the
  start output, and it warns at the bound without failing the start. The
  target is the switch target on macOS, twice it at `@2x`, and the tart
  display on Linux. When the macOS switch did not apply, the wait does
  not run. Also, `screen capture` takes its default crop size from the
  frame that it captured. The seams are in `vnc-size-wait-k14`.
- 2026-11-23, rejected. A 1x1 request per command came back from the
  stale surface. A wait before ClientInit costs every command. A wait in
  the first command or a held session can last more than 11 s. A sleep
  in `vm start` did not help on 3 of 4 guests. Doing nothing leaves a
  wrong size or a cropped or black first screenshot. ADR-0018 records the
  first two of these as considered options.
- 2026-11-23, name. The step is the *VNC size wait*, not a "settle",
  because `settle_failed_start` in `lifecycle.rs` already uses that word.
- 2026-11-23, record. The decision fails the ADR test on "hard to
  reverse", so it gets no ADR of its own. ADR-0018 states which faults of
  the latest host TestAnyware still handles, so it gains the wait in its
  context, diagram, decision, options and consequences. ADR-0014 stays as
  it is: its decision holds, and it has 38 hard findings, so an edit would
  need a full conversion first. The research note
  `docs/research/tart-vnc-stale-first-size-macos-27.md` holds the
  evidence, and ADR-0018 cites it. The glossary gains *VNC size wait*. The
  root brief points at the note.
- 2026-11-23, review. This session cut no `review-design` leaf. The design
  adds one bounded step to `vm start` and one crop fix. The live seam of
  `vnc-size-wait-k14` tests the wait on the real server, and the fast poll
  already measured its criterion on 7 guests. No doubt remains that needs
  an independent read first.
- 2026-11-23, leaves. `vnc-size-wait-k14` sits ahead of the release leaf.
  It is one vertical slice: the wait, the crop fix, the spec text and the
  live check. Its test seams are this design's proposal. The release leaf
  gained a line for the wait in its release notes.
- 2026-11-23, clean-up. Each of the 33 test guests is gone from tart, and
  I removed the 27 tart logs that `vm start` left in the state directory.
  I quit the development app and opened the installed 2.9.0 app again.
  The probes, drivers, records and the design view stay in scratch.
