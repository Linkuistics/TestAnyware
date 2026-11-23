# vnc-size-wait-k14

## Goal

Make `vm start` end with the VNC size wait on each tart guest, as ADR-0018
decides. Then the first command after `vm start` reads the real screen size.
Also make `screen capture` crop by the size of the frame that it captured.

## Context

- The design is in the running log of `first-frame-size-k9`, in ADR-0018
  and in `docs/research/tart-vnc-stale-first-size-macos-27.md`. The
  glossary term is *VNC size wait*.
- The fault: the first RFB session after a tart boot can get ServerInit
  1280x720 when the client sends ClientInit at once. A new session about
  0.27 s later gets the real size. On 2026-11-23 the first command after
  `vm start` read 1280x720 on 7 of 8 macOS guests and 1 of 2 Linux guests.
- The wait, as decided:
  - It runs at the end of the tart start in `testanyware-vm`
    (`start_tart_clone` in `lifecycle.rs`), after the macOS display switch,
    for macOS and Linux tart guests. QEMU guests get no wait.
  - The target is the size that `vm start` set. On macOS it is the switch
    target when the switch applied, and twice the target when the `@2x`
    switch applied. On Linux it is the size of the resolved tart display.
    When the macOS switch did not apply, the target is not known, so the
    wait does not run, and a progress line says so.
  - Each attempt opens a direct RFB session to the guest's VNC endpoint,
    reads ServerInit and closes the session. `vm start` runs in the app on
    a macOS host, and the app dials guests (ADR-0004). Attempts are 250 ms
    apart, and the wait stops at the target or after 30 s.
  - When the first attempt reads the target, the wait prints nothing.
    When it had to wait, a progress line names the stale size, the target
    and the time. At 30 s, a warning line names the last size, and
    `vm start` still succeeds. A cancelled start ends the wait at once.
  - No session outlives `vm start`. Nothing holds a VNC connection
    between commands.
- `screen capture` reads its default crop size from the connection before
  it captures (`commands/screen.rs`). A `DesktopSize` change that arrives
  with the frame then leaves a crop of the old size. It must take the
  default crop size from the captured framebuffer, for `--physical` too.
- `docs/specs/testanyware-app.md` describes the start: the sequence above
  "Lifecycle and adoption", and the first paragraph of that section.

## Done when

- `vm start` of a tart guest ends with the VNC size wait as decided above.
- These test seams pass. They are this design's proposal, because the
  owner was not asked in this session:
  - A unit test of the wait, with a seam in place of the session. A
    target at the first attempt gives one attempt and no progress line.
    Two stale reads and then the target give three attempts and a
    progress line with 1280x720. Stale reads for ever end at the bound
    with a warning. A cancel ends the wait.
  - A test of the real session against a local RFB listener that answers
    ServerInit with a scripted size for each connection.
  - A CLI fixture in `app-vnc-routing.rs`: the fake server gives a
    ServerInit size, then a `DesktopSize` change and a frame of another
    size. `screen capture` writes a PNG of the frame size. The test
    fails before the crop fix.
  - `task vm:vnc-idle-live` checks that the first `screen size` of its
    macOS guest and of its Linux guest is 1920x1080, and it prints the
    start output of each. It passes on the development bundle.
  - The existing suites pass: `task cli:test`, `task app:test`,
    `task app:routing-test` and `task scripts:check`.
- The app spec has the wait in its start sequence and states it in
  "Lifecycle and adoption".

## Notes

- Run each live check against the development bundle, not the installed
  2.9.0 app. That app starts a VNC keeper, which can hide the fault.
  Quit the installed app for the run, and open it again afterwards.
- The live check from before the fix gives the control. In
  `keeper-removal-code-k5`, its first `screen size` of the macOS guest
  read 1280x720 on 2 of 2 runs. Record whether the start output of the
  passing run names a wait.
- No backward compatibility. Add no option to turn the wait off.
- Out of scope: the `--region` check of `screen record` and the HiDPI
  scale check. After the wait, ServerInit is right for both. A guest
  behind `--vnc` or `--connect` gets no wait (ADR-0018).

## Decisions (running log)

- 2026-11-23, module. The wait is in a new module `vnc_size_wait.rs` of
  `testanyware-vm`. The module is not gated to macOS, so its tests run on
  each host. The loop takes the session as a closure, and the unit tests
  give it a scripted closure and the paused clock of tokio. The crate gains
  the `test-util` feature of tokio for its tests only, as the app crate has.
- 2026-11-23, session. A session is `RfbConnection::connect` of the RFB
  crate, then a drop. So the wait sends ClientInit as fast as each command
  does, and it measures the fault under the same timing. The connect also
  sends SetPixelFormat and SetEncodings, but it asks for no frame. The
  session reads the physical size, because ServerInit reports pixels.
- 2026-11-23, timing. The wait sleeps 250 ms after each session ends, as
  the fast poll of `first-frame-size-k9` did. The 30 s bound also limits
  each session, so a session that hangs cannot hold `vm start`. A session
  that fails counts as a stale read, and its error stands in for the size.
- 2026-11-23, place. The wait runs after the macOS display switch and
  before the spec and meta writes. So `vm list` and adoption see no
  sidecar of a start that is still in its wait. The caller of
  `start_tart_clone` already races it against the cancel token, so a
  cancel drops the wait and closes its open session. The wait also ends
  on the cancel itself, and then `start_tart_clone` returns
  `StartCancelled` before any write, so the caller removes the VM as it
  already does.
- 2026-11-23, fixtures. The fake tart of the routing tests printed the
  port of a listener that never speaks. A Linux start through it would
  now wait the full 30 s and warn. The macOS starts there have no
  `swiftc`, so their switch does not apply and they skip the wait with a
  line. The shared RFB fake gains `start_resized` for the crop test, and
  it builds its frame at the first request, so a 1920x1080 fake costs
  nothing for a client that asks for no frame.
- 2026-11-23, live result. `task vm:vnc-idle-live` passed on the
  development bundle, with the installed 2.9.0 app quit for the run. The
  first `screen size` read 1920x1080 on the macOS guest and on the Linux
  guest. The control from `keeper-removal-code-k5` read 1280x720 on 2 of 2
  runs. The start output of each guest names the wait: "VNC size wait: the
  first session read 1280x720. Session 2 read 1920x1080 after 0.26 s." So
  the fault also occurs on Linux, and the wait hid it on both guests.
