# TestAnyware.gh-issue-8 — brief

## Goal

Publish TestAnyware v2.5.0. The previous grove of this checkout said that two
of its features "ship with the next release". It cut no release leaf, so its
finish removed the tree and released nothing. This grove makes that release.

The two features:

- When the app starts and finds no `config.rhai`, it writes a default file.
- Golden creation grows the disk of the Linux golden to 50 GB.

## Done when

- The tag `v2.5.0`, its GitHub release, and the formula and the cask in
  `linkuistics/taps` hold one candidate. That candidate passed the checks below.
- The Linux host harness passed on the aarch64 Linux binary of the candidate,
  as ADR-0009 requires.
- The herdr acceptance run passed on the app bundle of the candidate.
- The owner approved the publication. The question named the tag, the commit
  and the sha256 of the artifacts.
- This Mac runs release 2.5.0 of the formula and of the cask, and
  `testanyware doctor` reports the app.
- The spec records the real upgrade of this Mac from 2.4.0 to 2.5.0.

## Decomposition

None. The pipeline of v2.4.0 is in the Taskfile, and the release fits one
session. So the leaf `plan-k1` makes the release itself.

## Pointers

- ADR-0009: the Linux host harness runs on each release binary.
- ADR-0020: one signing identity, and the cask `testanyware-app`.
- Spec: `docs/specs/testanyware-app.md`, the sections "Packaging and identity",
  "Test seams" and "Open risks".
- The Taskfile tasks `release:*` and `app:herdr-acceptance`.
- The record of the v2.4.0 release is in the history: the leaves under
  `.grove/24-k21/` in the parent of commit `95a3484`.

## Notes

- The build that passes the checks is the build that ships. A candidate is a
  build of a local annotated tag, with a manifest of the commit and the sha256
  of each artifact. `release:publish` pushes that tag and never rebuilds.
- A change to code, scripts or build inputs after the tagged commit voids the
  candidate. A change to `.grove/` or to the documents does not.
- Publication is outward-facing. Before it starts, ask the owner. The question
  covers each push of a commit or a tag, the GitHub release and the tap.
