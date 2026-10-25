# queue-release-k26

## Goal

Publish a tagged release that carries the admission queue to `linkuistics/taps`.
Run the live checks of the queue on that release, on this Mac.

## Context

- The leaf `release-2-5-0-k3` of the earlier grove on this repository is the model.
  Read its body in commit `5b3fd52`, and its retirement in commit `0a2f5a9`. It
  names the tasks, the order of the steps and the traps.
- The tasks `release:*` of the Taskfile publish a release. The live checks come
  from `queue-live-checks-k25`.
- The queue changes the protocol range of the app to 1 to 2. The human chooses the
  version of the tag. The last release is 2.5.0.
- The memory of this repository says that a release leaf must exist in the same
  grove as the work that ships with it. This is that leaf.

## Done when

- The human chooses the version. Before the build, the remote has no tag and no
  release of that version. If either exists, stop and ask.
- `task release:doctor` passes. The local tag names the commit to build, HEAD is
  that commit, and the tree is clean.
- `task release:candidate` and `task release:candidate-verify` pass for the tag.
  The decision log records the tag, the commit and the sha256 of each artifact.
- `task release:linux-harness` passes for the tag, and the log records the hashes.
- The live checks of the queue pass on the bundle and the CLI of the candidate.
  The herdr acceptance run passes on the candidate's bundle too.
- The human approves the publication. The question names the tag, the commit and
  the sha256 of the artifacts. It proposes to push the commits to `main` before
  the tag, as for the earlier releases.
- `task release:publish` publishes the recorded candidate and rebuilds nothing.
- This Mac runs the new release of the formula and of the cask, and
  `testanyware doctor` reports the app.
- The live checks of the queue pass again on the installed release.
- The spec `docs/specs/admission-queue.md` and ADR-0022 record the release and the
  result of the live checks.
- Before this leaf ends, look for work in this grove that no release carries.
  Tell the human what you find.

## Notes

- The tag for the earlier releases was local until the human approved the push.
  The auto-mode classifier may refuse the deletion of an unpushed tag. Then the
  human runs the command.
- The acceptance script and the live checks refuse to run while another app
  answers. Quit the installed app first, and open it again afterwards.
- A new executable UUID may bring the Local Network alert. The human allows it,
  and the run goes again.
