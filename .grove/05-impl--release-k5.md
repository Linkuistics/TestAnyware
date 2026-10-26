# release-k5

## Goal

Publish the release that carries `input-live-k2`, `input-delivery-k3` and
`macos-guest-versions-k4` to `linkuistics/taps`. Upgrade this Mac to it, and run
the input checks on the installed release.

## Context

- The leaf `queue-release-k26` of the earlier grove on this repository is the
  model. Its body is in the tree of commit `3a82b78`, and commit `b0d7294`
  retired it. Together they name the tasks, the order of the steps and the
  traps.
- The tasks `release:*` of the Taskfile publish a release. The spec
  testanyware-app lists the live checks of a release.
- The last release is 2.6.0.

## Done when

- The human chooses the version. Before the build, the remote has no tag and no
  release of that version. If either exists, stop and ask.
- `task release:doctor` passes. The local tag names the commit to build, HEAD is
  that commit, and the tree is clean.
- `task release:candidate` and `task release:candidate-verify` pass for the tag.
  The decision log records the tag, the commit and the sha256 of each artifact.
- `task release:linux-harness` passes for the tag, and the log records the
  hashes.
- On the bundle and the CLI of the candidate, `task input:live` passes on a
  macOS 27 guest. `task queue:live` and the herdr acceptance run pass too.
- The human approves the publication. The question names the tag, the commit
  and the sha256 of the artifacts. It proposes to push the commits to `main`
  before the tag, as for the earlier releases.
- `task release:publish` publishes the recorded candidate and rebuilds nothing.
- This Mac runs the new release of the formula and of the cask, and
  `testanyware doctor` reports the app.
- `task input:live` passes again on the installed release.
- The spec testanyware-app records the release and the result of the live
  checks.
- The human agrees to a comment on issue 12 that names the cause and the
  release. Then the comment goes on the issue, and the issue closes.
- Before this leaf ends, look for work in this grove that no release carries.
  Tell the human what you find.

## Notes

- The removal of the older macOS guests changes the CLI surface. The removal of
  the Windows host shipped as the minor release 2.4.0. So propose a minor
  version to the human.
- The tag of the earlier releases stayed local until the human approved the
  push. The auto-mode classifier may refuse to delete an unpushed tag. Then the
  human runs the command.
- The live checks refuse to run while another app answers. Quit the installed
  app first, and open it again afterwards.
- A new executable UUID may bring the Local Network alert again. The human
  allows it, and the run goes again.
