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

## Decisions (running log)

- **The version is v2.6.0.** The human chose it from three options. The queue is a
  new feature, `vm slots` is a new command, and the protocol range of the app grows
  from 1 to 2. The CLI surface keeps its meaning, and the releases v2.1.0 to v2.5.0
  were all minor. Before the build, `git ls-remote --tags origin 'v2.6*'` printed
  nothing and `gh release view v2.6.0` said `release not found`. The control
  `v2.*` listed `v2.5.0` on the remote.
- **The tag.** `git tag -a v2.6.0` names `3a82b78de2e0d13291a36040dbaa9fcd5902d574`,
  the commit that retired `queue-live-checks-k25`. HEAD was that commit and
  `git status` was clean. `task release:doctor` passed every check.
- **The candidate.** `task release:candidate TAG=v2.6.0` exited 0, and
  `task release:candidate-verify TAG=v2.6.0` passed. Tag `v2.6.0`, commit
  `3a82b78de2e0d13291a36040dbaa9fcd5902d574`. The sha256 of each artifact:
  - `testanyware-app-v2.6.0-aarch64-apple-darwin.zip`: `5748be80800f7f103fa4e085e393bd46639790805bfaadb03ea42b0390c225d8`
  - `testanyware-app.rb`: `4d37e56df2ee36e9b113374b06dfa79c56429d7fba4c5e3ff54a26241738f49f`
  - `testanyware-v2.6.0-aarch64-apple-darwin.tar.xz`: `7899e20a3c940a95e8ae045e5f3d0b1fd9c4422b3abe1bb94505fdbe3f563a39`
  - `testanyware-v2.6.0-aarch64-unknown-linux-gnu.tar.xz`: `61308f0a24430f0b7347eb59948c806be4684bf6267d529306028bbe8580ea57`
  - `testanyware-v2.6.0-x86_64-unknown-linux-gnu.tar.xz`: `b84f9e6c2f330e5131f83c12c9dd62a57807e84fca74be34e5d07a66045a4530`
  - `testanyware.rb`: `2e5c50d78a1944edb999460e0eff5f7e47080a50c61db06761889c8af4d0cc5b`
- **The bundle.** I unzipped the app zip into `target/candidates/v2.6.0.checks/app`.
  `task release:app-verify` reports `dev.antony.TestAnyware` of team `TA43A4RUP3`, the
  hardened runtime, notarisation and the staple. The version is 2.6.0. The
  executable UUID is `742DE1EC-13EF-390B-A9E4-D143457520A0`. The installed 2.5.0 has
  `D1773BE8-29F9-3DAB-9274-C22C602C3ADE`. The macOS CLI of the candidate is
  `testanyware 2.6.0`, binary sha256 `75a16abaaa50ea64a74f40025e8ef959da3eca3237b2c6c6bf8ac6cf716a1d42`.
- **No guest and no app at the start.** `tart list` showed no running guest, and no
  app answered, so I needed no consent to stop a guest.
- **The live checks of the queue passed on the candidate.**
  `task queue:live APP=<bundle> BIN=<CLI of the candidate>` printed `PASS` for the
  three checks and exited 0: the order of the queue (the third start waits at place 1
  and the stop of the first admits it within 17 s), an outside guest counts
  (`vm slots` names it external, and the end of it admits the waiting start after
  1 s), and a golden creation waits (place 1, and Ctrl-C ends the wait and changes
  nothing). The log is in the scratchpad of this session, not in the repository.
- **The herdr acceptance run passed on the candidate.** I ran
  `scripts/herdr-acceptance-live.sh` directly with `TESTANYWARE_BIN` on the candidate
  CLI and the unzipped bundle (build 2.6.0), from herdr session `w2E:p1`. `vm start`
  reached a healthy agent in 14 s. The agent, screen, input and file steps passed,
  and the six controls each failed with `APP_UNREACHABLE` when no app answered. `vm
  stop` left no guest and no sidecar. The script printed `PASS`.
- **The Linux harness passed.** I started the bundle with `open -n -g`, because the
  harness needs an app on the socket and so tests the candidate's app. Then
  `task release:linux-harness TAG=v2.6.0` ran on the aarch64 Linux archive (sha256
  `61308f0a24430f0b7347eb59948c806be4684bf6267d529306028bbe8580ea57`) and its binary
  (sha256 `da973408aa526533fadf1cec9d707807722f305ea1c291ad7a04d858b687a923`). The
  6 endpoint-free checks passed, the library check read 350 native libraries and found
  no foreign need, and the harness printed `test result: ok. 1 passed; 0 failed` after
  374 s. The log is `target/candidates/v2.6.0.checks/linux/harness.log`. Then I quit
  the candidate app and opened `/Applications/TestAnyware.app` again (2.5.0).
- **No Local Network alert stopped a run.** This session cannot see the screen. No
  command failed with `local_network_denied` on the new executable UUID.
- **The state before the approval.** `origin/main` is `87968a7a` and is an ancestor of
  HEAD. The tap clone at `~/Development/Delivery/homebrew-taps` is clean, on a
  detached HEAD at `57f0d6c`, which is `main` and `origin/main`. The publication
  sets `TESTANYWARE_TAP_DIR` to it.
- **The human approved the publication.** The question named the tag `v2.6.0`, the
  commit `3a82b78d` and the sha256 of the artifacts, and it proposed the push of
  `main` first. The human chose to push `main` first.
- **The publication used the recorded candidate.** `git push origin 3a82b78d:refs/heads/main`
  fast-forwarded `origin/main` from `87968a7` to `3a82b78` (32 commits). Then
  `TESTANYWARE_TAP_DIR=$HOME/Development/Delivery/homebrew-taps scripts/release-publish.sh v2.6.0`
  verified the candidate again, pushed the tag (`[new tag] v2.6.0 -> v2.6.0`), created
  `https://github.com/Linkuistics/TestAnyware/releases/tag/v2.6.0`, and pushed the commit
  `06d3ca7` `testanyware v2.6.0` to `linkuistics/taps` (`57f0d6c..06d3ca7`). It rebuilt
  nothing. I ran the script directly, because `release:publish` has a prompt for a person.
- **The upgrade of this Mac.** Before: the formula and the cask were 2.5.0, with the
  executable UUID `D1773BE8-29F9-3DAB-9274-C22C602C3ADE`. I ran
  `brew upgrade linkuistics/taps/testanyware` and
  `brew upgrade --cask linkuistics/taps/testanyware-app`. Homebrew quit the app, replaced
  the bundle and reopened it. After: both are 2.6.0. The identity is `dev.antony.TestAnyware`
  of team `TA43A4RUP3`, `spctl` says `accepted` and `Notarized Developer ID`, and the
  executable UUID is `742DE1EC-13EF-390B-A9E4-D143457520A0`. The quarantine value is `01c1`.
  `testanyware doctor` reports the app installed and running, build 2.6.0, protocol 1-2,
  and the Host CLI and the app at the same build. It reports two items that are not faults of
  the upgrade: the login item is off, and no guest connection has shown the Local Network grant
  yet. No guest ran during the upgrade, so it adopted none.
- **The live checks passed again on the installed release.**
  `task queue:live APP=/Applications/TestAnyware.app BIN=/opt/homebrew/bin/testanyware`
  printed `PASS` for the three checks and exited 0. Its guests ran under the new app with no
  alert that stopped a command.
- **The documents.** The spec `docs/specs/admission-queue.md` ("Test seams") and ADR-0022
  ("Consequences") record release 2.6.0 and both runs of the live checks. `ste-lint` reports
  0 hard findings on the added text.
- **Work that no release carries.** I read every leaf of the tree for deferred work. Each
  leaf is retired. The skill change is in the Grove repository (`Linkuistics/grove`,
  `main` at `43290da0`), and it ships through that repository's own releases, not through
  this one. Issues #9, #10 and #11 and a comment on #4 hold the parts that moved, and they
  are not work for this grove. No commit follows `v2.6.0` apart from this leaf's own.
- **Source for the API.** No framework API of a changing version entered this leaf.
- **Review.** I spent no in-session reviewer. The publication waited for the human's
  approval, and every claim above is an output shown in this log.
