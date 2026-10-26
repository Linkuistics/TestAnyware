# input-delivery-k3

## Goal

Make the `input` commands take effect in a macOS 27 guest again. Fix the fault
at the cause that `input-live-k2` found.

## Context

- The research note `docs/research/tart-vnc-reset-drops-input-macos-27.md`. It
  holds the reproduction and the control. A reset of the guest connection right
  after the input makes tart discard the input. The same input through a bare
  client with an ordinary close reaches the guest. A pause of 10 ms before the
  reset kept the input in four runs.
- `task input:live` is the acceptance test of the fix. It fails today with four
  failed steps: click, type, scroll and drag. `scripts/tart-vnc-input-control.py`
  is the bare client. Its `--burst --close reset` option copies the app.
- The spec testanyware-app, section "Guest traffic on a macOS host", states how
  a stream ends and when the app resets the guest connection. The reset exists
  because the VNC server of tart holds a connection that the app closed in the
  ordinary way in `CLOSE_WAIT` for about two minutes. The app of 2.4.0 ran out
  of descriptors after about 120 commands. `task app:stream-end-live` checks it.
- The routing seam is `cli-rs/crates/testanyware-cli/tests/app-vnc-routing.rs`,
  with `FakeRfb` and `TestApp` in `tests/common`. Its tests already show that
  the events of `input type` and `input click` reach the fake. They pass today,
  because the fake reads each byte at once.

## Done when

- A host-side test at the agreed seam fails before the fix and passes after it.
  If the cause is in the path through the app, the test runs in `task
  app:routing-test`. Its fake RFB server then copies what the research note
  shows tart does.
- `task input:live` passes on a macOS 27 guest with the development bundle.
- The gain of 2.5.0 stays: `task app:stream-end-live` keeps the descriptors of
  the app flat.
- The CLI surface stays compatible. The fix adds no required flag, and it
  changes no exit code and no `--json` field.
- The spec states the rule that the fix makes true.
- The test tasks of each changed crate pass, for example `task app:test`, `task
  app:routing-test` and `task cli:test`.

## Notes

- Two candidate fixes, to weigh against the evidence and not to take as given:
  - The RFB client of the CLI makes sure that the server read its input before
    it closes. For example, it sends a FramebufferUpdateRequest that is not
    incremental, and it waits for the answer.
  - The app waits for a short time after the client has gone, and then it resets
    the guest connection.
- Each candidate has a cost. Choose by the evidence. Apply the ADR test of
  `ADR-FORMAT.md` to the choice.
- The cause can lie outside TestAnyware, for example in Virtualization.framework.
  A workaround in TestAnyware is then still the goal, and the research note
  records the outside cause.
- At the end, decide whether the fix needs a `review-impl` leaf.

## Decisions (running log)

**The fix is in the RFB client of the CLI. The app stays a plain byte relay.** After its last input event, the client sends a FramebufferUpdateRequest (not incremental, one pixel) and waits for the FramebufferUpdate that answers it. A server reads the messages of a connection in order, so the answer shows that the server read the input. Only then does the command close, and the reset of the app drops nothing.

The other candidate was a pause in the app before the reset. The research note measured 10 ms as enough in four runs. But the number is a guess, and the speed of tart under load decides whether it holds. The spec also says that the app carries the bytes of the guest protocol unchanged, and a wait that is tied to RFB would break that. The client already speaks RFB, so the barrier belongs there.

**A wait that gets no answer does not change the command.** The wait ends after a limit, the command prints a warning on stderr, and its exit code and its output stay as they were. The fix adds no flag and no `--json` field.

**The test seam is `app-vnc-routing`.** `FakeRfb` gains a mode that copies tart: it reads one message in each `lag`, and when a reset has arrived at that time it drops what it has not read. The new test runs `input click` and `input type` through the real CLI and the real app, and it fails before the fix.

**The choice earns no ADR.** It is easy to reverse, because it lives in client code. So the spec section "Guest traffic on a macOS host" and the research note carry it, and `docs/adr/` stays as it is.

**State when the session stopped, before the live run.** The routing seam, the unit tests of `confirm_input_read`, `task cli:test` (1654 tests) and `task app:test` pass. Both new routing tests failed before the fix, with an empty event log, and pass after it. The spec states the rule. `task input:live` has not run on the fix. The installed 2.6.0 app (pid 69159, started 20:39) answers on the socket, and the script refuses to run beside an app that is not the development bundle. The session may not quit that app, so a person must quit it. Then `task input:live` runs, and `task app:stream-end-live` runs after it. The leaf stays live until both pass.

**The live checks passed, and the leaf is done.** `task input:live` on the development bundle passed both checks: click focused the field, type put `input-live-ok` in it, scroll moved `row-20` from y=986.0 to y=936.0, and the drag of the thumb moved it to y=759.0. `task app:stream-end-live` passed: 33 descriptors after one command, 32 after 151, and one connection in `CLOSE_WAIT` before and after. The installed 2.6.0 app was already gone when the person ran the `kill`, so the run went on.

**The consumers need no new sweep.** The fix adds no flag, no exit code and no `--json` field. A command prints a warning on stderr only when the guest gives no sign of reading the input. Each input command takes one more round trip, so it takes a little longer.

**No `review-impl` leaf.** The routing test fails before the fix and passes after it, and the live check does the same on a real guest. The one open point is a possible unsolicited update from tart. One passing run gives no rate for it, and `release-k5` runs the live checks again on the candidate. A review leaf would read the same diff and the same evidence.
