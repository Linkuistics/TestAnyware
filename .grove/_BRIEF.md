# TestAnyware.gh-issue-8 — brief

## Goal

Publish TestAnyware v2.5.0. The previous grove of this checkout said that two
of its features "ship with the next release". It cut no release leaf, so its
finish deleted the tree and released nothing. This grove makes that release.

The release carries three changes:

- When the app starts and finds no `config.rhai`, it writes a default file.
- Golden creation grows the disk of the Linux golden to 50 GB.
- The app stops a leak of descriptors in its guest streams. The release checks
  found the leak in the app 2.4.0, and the owner chose to fix it first.

## Done when

- When the client of a guest stream has gone, the app closes the guest
  connection, even when the guest never ends its direction. Many VNC commands
  to a tart guest leave the descriptors of the app and of tart flat.
- The tag `v2.5.0`, its GitHub release, and the formula and the cask in
  `linkuistics/taps` hold one candidate. That candidate holds the fix, and it
  passed the checks below.
- The Linux host harness passed on the aarch64 Linux binary of the candidate,
  as ADR-0009 requires.
- The herdr acceptance run passed on the app bundle of the candidate.
- The owner approved the publication. The question named the tag, the commit
  and the sha256 of the artifacts.
- This Mac runs release 2.5.0 of the formula and of the cask, and
  `testanyware doctor` reports the app.
- The spec records the real upgrade of this Mac from 2.4.0 to 2.5.0.

## Decomposition

1. `guest-stream-end` (impl): the fix of the leak, with its tests and the
   spec's rule for the end of a guest stream.
2. `release-2-5-0` (impl): the candidate with the fix, the checks, the
   publication, and the upgrade of this Mac. It is the last leaf. Put any new
   work before it, never after it, so that the grove cannot finish unreleased.

## Pointers

- ADR-0004: each command opens its own guest connections.
- ADR-0009: the Linux host harness runs on each release binary.
- ADR-0018: tart guests get a VNC keeper.
- ADR-0020: one signing identity, and the cask `testanyware-app`.
- Glossary terms in play (see `CONTEXT.md`): TestAnyware app, Guest stream,
  Host CLI, Host-side framebuffer and VNC keeper.
- Spec: `docs/specs/testanyware-app.md`, the sections on guest streams,
  "Packaging and identity", "Test seams" and "Open risks".
- The Taskfile tasks `release:*` and `app:herdr-acceptance`.
- The record of the v2.4.0 release is in the history: the leaves under
  `.grove/24-k21/` in the parent of commit `95a3484`.
- Test seams for the fix, which the owner agreed in `plan-k1`:
  - The app's protocol seam. The app runs in its test mode. The real Host CLI
    opens many VNC streams to a synthetic RFB guest that never closes after the
    client's end. The app's open descriptors stay flat, and the fake guest sees
    each connection end.
  - The relay's unit tests. A client that half-closes and still reads gets the
    guest's answer.
  - A Taskfile live check on this Mac. Many `screen` commands go to a tart
    guest. The app's descriptors and tart's connections in `CLOSE_WAIT` stay
    flat.

## Notes

- The build that passes the checks is the build that ships. A candidate is a
  build of a local annotated tag, with a manifest of the commit and the sha256
  of each artifact. `release:publish` pushes that tag and never rebuilds.
- A change to code, scripts or build inputs after the tagged commit voids the
  candidate. A change to `.grove/` or to the documents does not.
- The local tag `v2.5.0` names `f0108944`, a candidate without the fix. That
  candidate is void. Never publish it.
- Publication is outward-facing. Before it starts, ask the owner. The question
  covers each push of a commit or a tag, the GitHub release and the tap.
