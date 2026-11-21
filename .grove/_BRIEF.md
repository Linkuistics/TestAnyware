# TestAnyware.gh-issue-13 — brief

## Goal

Answer issue #13, then follow through on its finding. macOS 27.2.1 removed
the tart VNC fault that the VNC keeper and the keepalive captures of Koine
exist for. This grove makes TestAnyware require the latest macOS 27 and tart
on a macOS host. It removes the workarounds that this requirement makes
unnecessary. It keeps each golden at the latest release of its major, and it
ships the result in one TestAnyware release.

## Done when

- Issue #13 carries the approved reply and is closed as not planned. Done on
  2026-11-21.
- Issue #2 carries the approved update and is closed as completed. Done on
  2026-11-21.
- The docs state the host requirement: the latest release of macOS 27 and the
  latest release of tart on a macOS host. A guest is outside this
  requirement.
- The VNC keeper is gone from the code, tests, tasks and docs. ADR-0018 and
  the glossary describe the state without it.
- Each other workaround for a macOS 27 VNC fault has a test on the latest
  host. It is gone if the test shows its fault is gone and the fix is a pure
  workaround. Otherwise the design records why it stays.
- The golden rule is recorded. `vm create-golden` names the point release and
  refreshes an older base VM. The macOS 27 golden runs macOS 27.2.1.
- A TestAnyware release carries the work, this Mac runs it, and
  `origin/main` holds the final commit.

## Decomposition

The leaves are flat siblings under the root. Each one needs the context of the
root, and no subset shares a context of its own. The keeper
design comes first, because it touches the most code. The golden design does
not depend on it. Each design session inserts its implementation leaves ahead
of the release leaf, which comes last.

## Pointers

- ADRs: `docs/adr/0018-a-macos-host-runs-the-latest-macos-27-and-tart.md`
  for the host requirement and the removal of the keeper, and
  `docs/adr/0004-each-command-opens-its-own-guest-connections.md` for why
  each command opens its own connection, and
  `docs/adr/0027-each-golden-holds-the-latest-release-of-its-major.md` for
  the golden rule.
- Research: `docs/research/tart-vnc-reconnect-crash-macos-27.md` for the idle
  fault, and `docs/research/tart-vnc-reset-drops-input-macos-27.md` for the
  fault of issue #12.
- Glossary terms: VNC keeper, Base VM, Golden image, TestAnyware app and
  Host-side framebuffer, in `CONTEXT.md`.
- Test seams, agreed with the owner on 2026-11-21:
  - Keeper removal, live: `task vm:vnc-keeper-live` in a new form, renamed
    `task vm:vnc-idle-live` by `keeper-removal-k2`. A macOS guest that
    `vm start` starts takes two `screen` commands 5 s apart. It then stays
    alive through 5 minutes with no VNC client, and a Linux guest through 70
    seconds. A `screen` command then works, and no `_vnc-keeper` process
    exists.
  - The fix of Apple alone: `task vm:tart-vnc-idle-repro`, unchanged, passes
    without `HOLD=1`.
  - Code: the existing suites without keeper cases. `cli-contract.rs` shows
    that the command surface holds no `_vnc-keeper`. No suite gains a
    compatibility case.
  - Issue #12: `scripts/tart-vnc-input-control.py` on the latest host decides
    whether the fault is gone. If its workaround goes, `task input:live`
    accepts the removal, and `task app:routing-test` stays the regression
    guard.
  - Golden plan: `vm create-golden --dry-run` names the macOS point release
    that it would install, and it says when the base VM is older.
  - Golden build: a built guest reports 27.2.1 from
    `sw_vers -productVersion`, and the live gate in `live-vm-gate.rs` passes
    on that golden.
  - Release: the facts of the close rule of the owner. They are
    `gh release view`, `brew list --versions`, `testanyware doctor` with the
    app running, and `origin/main`.

## Notes

- No backward compatibility. The projects of the owner are the only users of
  TestAnyware. Old state, such as guests, metas, start records, the base VM
  and goldens, is discarded and rebuilt, not migrated.
- The test results decide, not release notes. The reproduction and
  attribution results are in the notes of `plan-k1`. macOS, not tart,
  removed the idle fault.
- Each GitHub comment needs the approval of the owner for its exact text
  before a session posts it.
- On 2026-11-21, after the macOS upgrade, the TestAnyware app did not run at
  first. Later that day it answered at 2.9.0. A live task through `vm start`
  needs it.
