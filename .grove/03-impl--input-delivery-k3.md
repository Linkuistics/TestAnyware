# input-delivery-k3

## Goal

Make the `input` commands take effect in a macOS 27 guest again. Fix the fault
at the cause that `input-live-k2` found.

## Context

- The research note that `input-live-k2` writes. That leaf adds its path here.
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
