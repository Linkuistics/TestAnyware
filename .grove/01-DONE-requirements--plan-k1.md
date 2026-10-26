# plan-k1

## Goal

Settle what grove `gh-issue-12` builds, from the words of the human in GitHub
issue 12. Record the decisions, and agree the test seams with the human. Then
write the root brief, and cut the leaves that deliver the work.

## Context

- Issue 12 is the input of this grove:
  <https://github.com/Linkuistics/TestAnyware/issues/12>. The human wrote it on
  26 October 2026, after this Mac went from 2.5.0 to 2.6.0 at 15:53.
- This session found these facts:
  - From v2.5.0 to v2.6.0, the changes are the admission queue and the skill.
    No file of the RFB client, of the `input` command or of the relay changed.
    One test file of the guest streams changed by four lines.
  - The reset of the guest connection at the end of a relay (`close_guest`)
    first shipped in v2.5.0, in commit 6011dba at 20:21 on 24 October. The
    release leaf published v2.5.0 at about 22:24 on that day.
  - The VNC keeper (ADR-0018) first shipped in v2.2.0.
  - tart 2.40.1 has not changed since 16 October. The golden
    `testanyware-golden-macos-27` has not changed since 19 October. So the run
    on 24 October and the run on 26 October used the same tart and golden.
  - `input type` connects, sends its key events, prints its line and exits. Its
    client closes straight after the last event and waits for no answer.
  - The spec testanyware-app, in "Guest traffic on a macOS host", says that the
    bytes of the client reach the guest before the stream ends. It also says
    that the app resets the guest connection when the client has gone.
  - A loopback experiment on this Mac: a sender writes 48 bytes, half-closes and
    closes with a reset. A plain reader that reads 0 ms or 50 ms later still
    gets the 48 bytes, and then `ECONNRESET`. An ordinary close never gave the
    error. So the kernel keeps unread bytes after a reset. If the reset is the
    cause, tart's VNC server drops the buffered input when it sees the reset.
  - No live check verifies the effect of an `input` command.
    `app:herdr-acceptance` and `app:routing-live` send input and read only the
    exit status.
  - The user guide `docs/user/golden-images.md` names two macOS guest versions:
    macOS 27, the default, and macOS 26 (Tahoe). Both goldens exist on this Mac.
    `testanyware-golden-macos-tahoe` dates from 24 June.

## Done when

- The running log below holds each decision.
- The human agreed the test seams, and the root brief records them.
- The root brief states the goal, the done criteria, the decomposition and the
  pointers.
- Each new leaf exists, with its body written.
- This leaf is retired, and its commit is sealed.

## Decisions (running log)

**The deliverable is the next release of TestAnyware.** In it, `input` commands
take effect in a macOS guest again. The issue states the expected result: typed
text reaches the focused field, and a scroll event moves a scrollable view.
Until then, the fault blocks Koine's client script. So this grove publishes the
release itself, and it cuts the release leaf now (memories `focused-deliveries`
and `release-before-grove-ends`).

The conversation called the release 2.6.1. But the release also removes the
older macOS guests, and that changes the CLI surface. The removal of the
Windows host shipped as the minor release 2.4.0. So the release leaf asks the
human for the version, as the leaf of 2.6.0 did.

**The two live checks of the issue ship in the same release, and they come
first.** They are the reproduction of the fault and the proof of the fix. The
type check clicks a text field, types text and reads the value back through
`agent snapshot`.

The scroll check uses a view taller than its window. It reads the position of
one element, scrolls with the wheel and reads the position again. Then it drags
the thumb of the scroll bar and reads the position again. So the two checks
cover the four commands that failed in the issue: click, type, scroll and drag.

**Linux and Windows guests are out of scope for the live checks.** The issue
scopes macOS guests. The cause may lie in a path that every tart guest shares,
such as the relay of the app. A host-side test then covers that path for every
platform.

**The fix follows the cause that the reproduction shows, not the 2.6.0 diff.**
2.6.0 changed no file on the input path. The reset shipped in 2.5.0, and the
passing run on 24 October can have used 2.4.0. So the first leaf runs a control
that takes the app out of the path, before any fix.

**The CLI surface stays compatible.** Many projects drive TestAnyware through the
CLI only (memory `testanyware-consumers`). The fix adds no required flag, and it
changes no exit code and no `--json` field.

**The checks run on macOS 27 guests only.** The user guide names macOS 27 and
macOS 26 (Tahoe). Asked whether the checks run on every release, the human
answered:

<!-- tc: off because this quotes the human verbatim -->

> We only need MacOS 27, we can abandon anything that is MacOS 26 specific.
> Other than that, I agree with every release

<!-- tc: on -->

So no leaf of this grove runs, keeps or fixes anything for macOS 26 alone. The
golden `testanyware-golden-macos-27` is on this Mac, so the checks need no new
golden build.

**The checks run on every release.** Each release candidate passes both checks
on a macOS 27 guest. The spec testanyware-app lists them with the other live
checks of a release. The human agreed in the answer above. Each existing release
check that sends input reads only the exit status. That is how 2.5.0 and 2.6.0
both shipped this fault.

**This grove removes macOS 26 from TestAnyware, and the next release carries the
removal.** The human chose this over a separate issue. A leaf after the fix and
before the release does the removal.

**The removal covers every macOS guest version older than 27, and hosts keep
macOS 14 or later.** The human chose this scope. Golden creation for macOS then
builds only from Apple's IPSW, for macOS 27 or a later major. The Cirrus Labs
path for `tahoe`, `sequoia` and `sonoma` goes. So does a major below 27, such as
`--version 26`. The Cirrus Labs path for Linux goldens stays.

**No consumer starts an older macOS golden.** A sweep of the scripts under
`~/Development` found only comments, a usage example and recorded evidence. Its
positive control found `scripts/queue-live.sh` of TestAnyware, which passes
`--version tahoe`. UIAnyware's `verify.sh` starts the default golden, which is
macOS 27.

**The human agreed three test seams.** The live checks are the acceptance seam.
They drive a real macOS 27 guest through the app with `input`, and they read
each effect through `agent snapshot`. The existing CLI tests of `task cli:test`
cover the removal.

The routing seam of `task app:routing-test` is the regression guard of the
fault. Its fake RFB server gains a mode that copies what the live evidence shows
tart does, so that its test fails before the fix. If the cause lies elsewhere,
the fix uses the highest existing seam at that point.

**The grove has four leaves, in this order.** `input-live-k2` reproduces the
fault, and `input-delivery-k3` fixes it. `macos-guest-versions-k4` does the
removal, and `release-k5` publishes the release. No review leaf follows this
one. The human settled each decision here in the session, and no later work
builds on this log for months.
