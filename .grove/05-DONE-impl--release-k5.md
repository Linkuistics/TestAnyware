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

## Decisions (running log)

- **The version is v2.7.0.** The human chose it from three options (v2.7.0, v3.0.0, v2.6.1). The input fix is
  a repair and the refusal of macOS guests older than 27 changes the CLI surface, but the removal of the
  Windows host shipped as the minor 2.4.0, and every release from 2.1.0 to 2.6.0 was minor. Before the build,
  `git ls-remote --tags origin 'v2.7*' 'v3*'` printed nothing and `gh release view v2.7.0` and
  `gh release view v3.0.0` said `release not found`. The control `v2.6*` listed `v2.6.0` on the remote.
- **The tag.** `git tag -a v2.7.0` names `65bc05c91cc5d2a6e3d4aa8f81cb2c7c7712fa03`, the commit that retired
  `macos-guest-versions-k4`. HEAD was that commit, `jj new` put an empty change above it, and `git status`
  was clean. `task release:doctor` passed every check.
- **The candidate.** `task release:candidate TAG=v2.7.0` exited 0, and `task release:candidate-verify TAG=v2.7.0`
  passed. Tag `v2.7.0`, commit `65bc05c91cc5d2a6e3d4aa8f81cb2c7c7712fa03`. The sha256 of each artifact:
  - `testanyware-app-v2.7.0-aarch64-apple-darwin.zip`: `d0f5dd67ddd5f140bb65e3f056f743ef98376dc04d9ca5410b0ab57e182640ea`
  - `testanyware-app.rb`: `ef93244c6646b02b10524777991a1d39e69a66448cbb43d3c96374e3d4f66acb`
  - `testanyware-v2.7.0-aarch64-apple-darwin.tar.xz`: `9793e38f08ad43c01f178d3a6a3f6f014736f05afb73bfb7a79bbafa412db098`
  - `testanyware-v2.7.0-aarch64-unknown-linux-gnu.tar.xz`: `523123f03f3dc4e95c55ee25ad4bd07d9cdb8916fdfcc550d18f8c1fe174d416`
  - `testanyware-v2.7.0-x86_64-unknown-linux-gnu.tar.xz`: `f04704f07f296ad1043c0c5b50943afff5c0f151a8d33ddbc8da59c4543dea09`
  - `testanyware.rb`: `2edda32cdf1432a274d932e995684c052fdf6d100ce991c60de125478b7ddb96`
- **The bundle.** I unzipped the app zip into `target/candidates/v2.7.0.checks/app`. `task release:app-verify`
  reports `dev.antony.TestAnyware` of team `TA43A4RUP3`, the hardened runtime, notarisation and the staple. The
  version is 2.7.0. The macOS CLI of the candidate is `testanyware 2.7.0`, binary sha256
  `2298b1bd55af4f208286c57b6ff67b98e79a5ea8641c1c584632895bcc707720`.
- **No guest and no app at the start.** `tart list` showed no running guest, and no app answered.
- **`task input:live` passed on the candidate.** `task input:live APP=<bundle> BIN=<CLI of the candidate>`
  printed `PASS` for both checks and exited 0, on a macOS 27 guest under build 2.7.0. Step 0 (the oracle)
  passed. Check 1: `input click` focused the field and `input type` put "input-live-ok" in it. Check 2:
  `input scroll` moved row-20 from y=986.0 to y=936.0 and `input drag` of the thumb moved it to y=759.0.
  The log is in the scratchpad of this session, not in the repository.
- **`task queue:live` passed on the candidate.** It printed `PASS` for the three checks and exited 0.
- **The herdr acceptance run passed on the candidate.** I ran `scripts/herdr-acceptance-live.sh` with
  `TESTANYWARE_BIN` on the candidate CLI and the unzipped bundle (build 2.7.0), from a herdr session. `vm start`
  reached a healthy agent in 14 s. The agent, screen, input and file steps passed, and the six controls each
  failed with `APP_UNREACHABLE` when no app answered. `vm stop` left no guest and no sidecar. It printed `PASS`.
- **The Linux harness passed.** I started the bundle of the candidate with `open -n -g`, because the harness
  needs an app on the socket and so tests the candidate's app. `task release:linux-harness TAG=v2.7.0` ran on the
  aarch64 Linux archive (sha256 `523123f03f3dc4e95c55ee25ad4bd07d9cdb8916fdfcc550d18f8c1fe174d416`) and its binary
  (sha256 `395439871ea2893d057cfaf74b3d181539c363bd02f19235bf24940f7be90217`). All 11 endpoint-driven checks
  passed and the harness printed `test result: ok. 1 passed; 0 failed` after 270 s. The log is
  `target/candidates/v2.7.0.checks/linux/harness.log`. Then I quit the candidate app and opened
  `/Applications/TestAnyware.app` again (2.6.0).
- **No Local Network alert stopped a run.** This session cannot see the screen, and no command failed with
  `local_network_denied`.
- **The state before the approval.** `origin/main` is `ad7d2b7`, the parent of the first commit of this grove.
  The tap clone at `~/Development/Delivery/homebrew-taps` is clean on `main`. The remote has no tag or release
  `v2.7.0`.
- **The human approved the publication.** The question named the tag `v2.7.0`, the commit `65bc05c9` and the
  sha256 of the artifacts, and it proposed the push of `main` first. The human chose to push `main` first.
- **The publication used the recorded candidate.** `git push origin 65bc05c9:refs/heads/main` fast-forwarded
  `origin/main` from `ad7d2b7` to `65bc05c` (4 commits). Then
  `TESTANYWARE_TAP_DIR=$HOME/Development/Delivery/homebrew-taps scripts/release-publish.sh v2.7.0` verified the
  candidate again, pushed the tag (`[new tag] v2.7.0 -> v2.7.0`), created
  `https://github.com/Linkuistics/TestAnyware/releases/tag/v2.7.0`, and pushed the commit `8ee639c`
  `testanyware v2.7.0` to `linkuistics/taps` (`06d3ca7..8ee639c`). It rebuilt nothing. I ran the script
  directly, as for v2.6.0, because `release:publish` has a prompt for a person.
- **The upgrade of this Mac.** `brew upgrade linkuistics/taps/testanyware` and
  `brew upgrade --cask linkuistics/taps/testanyware-app` took both from 2.6.0 to 2.7.0. `spctl` says `accepted`
  and `Notarized Developer ID` of team `TA43A4RUP3`. `testanyware doctor` reports the app installed and running,
  build 2.7.0, protocol 1-2, and the Host CLI and the app at the same build. It reports two items that are not
  faults of the upgrade: the login item is off, and no guest connection has shown the Local Network grant yet.
- **`task input:live` passed again on the installed release.**
  `task input:live APP=/Applications/TestAnyware.app BIN=/opt/homebrew/bin/testanyware` printed `PASS` for both
  checks and exited 0. No alert stopped a command.
- **The spec.** `docs/specs/testanyware-app.md` ("Test seams", the input commands) records release 2.7.0 and both
  runs of `input:live`. The 4 findings that `ste-lint` still reports in that file are on lines I did not write.
- **The issues.** The human asked to close or adjust the relevant issues, so I took that as the agreement to a
  comment on issue 12. The comment names the cause (the reset of 2.5.0 overtakes input that tart has not read),
  the change, the checks and release 2.7.0. Then I closed issue 12 as completed. Issue 2 has a macOS 26.5
  reproduction, so I added a comment that says what 2.7.0 changed for it and that the issue stays open. Issues 1,
  4, 5, 8, 9, 10 and 11 do not name an older macOS guest, so I left them alone.
- **Work that no release carries.** Every leaf of the tree is retired, and no commit follows `v2.7.0` apart from
  this leaf's own. The research note names one open item: whether tart sends an update that we did not ask for,
  which would end the wait early. One run of `input:live` does not give a rate, and the note records it as not
  measured. Nothing else is deferred. The Grove repository holds no change from this grove.
- **Source for the API.** No framework API of a changing version entered this leaf.
- **Review.** I spent no in-session reviewer. The publication waited for the human's approval, and every claim
  above is an output shown in this log.
