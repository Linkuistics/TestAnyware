# host-requirement-release-k4

## Goal

Publish a TestAnyware release that carries the work of this grove. Install it
on this Mac, then move and push `main`.

## Context

- The Taskfile of the project holds the release tasks. The leaf
  `lifecycle-release-k226` of the previous grove published v2.9.0 the same
  way. Commit `8fbb7568` holds its record.
- The close rule of the owner names the facts that this leaf proves.

## Done when

- `gh release view` shows the new release.
- `brew list --versions` shows the formula at that version.
- With the TestAnyware app running, `testanyware doctor` shows the Host CLI
  and the app at that build.
- `origin/main` holds the final commit.
- The release notes state the new host requirement and the removal of the
  keeper.
- The release notes state the golden rule of ADR-0027. They say that the
  first macOS `vm create-golden` deletes the base VM of 2.9.0 and builds a
  base of the latest release.
- The release notes say that `vm start` of a tart guest now ends with the
  VNC size wait. So the first `screen size` or `screen capture` after it
  reads the real size, not 1280x720.

## Notes

- The owner chooses the version number. Without the keeper, a host on an
  earlier macOS 27 release loses its guests after an idle minute. So the
  release may count as a breaking change.
- Each design leaf inserts its implementation leaves ahead of this leaf.

## Decisions (running log)

- 2026-11-23: `task release:doctor` passed every check. `main@origin` is
  `306cb659`, an ancestor of `ee1b9e88`, so the push is a fast-forward of 16
  commits.
- 2026-11-23: the sweep of `~/Development` found no consumer that runs
  `_vnc-keeper`. The only JSON schema change since `v2.9.0` adds fields to
  `vm-create-golden.json`. Koine still runs its own keepalive captures, and
  they stay harmless. One guide in Ravel-Strategies still describes the keeper.
- 2026-11-23: **the version is v3.0.0.** The owner chose it from v3.0.0 and
  v2.10.0, because the release raises the host requirement.
- 2026-11-23: the owner allows this session to publish when notarisation,
  candidate verification, the Linux harness and the upgrade check pass. The
  owner answers any macOS dialog of the upgrade check.
- 2026-11-23: **the tags.** `git tag -a v3.0.0` and `git tag -a v2.9.99` both
  name `ee1b9e88e31517fb12a386ae218dca604955f5ff`. Both stay local. HEAD was
  that commit and `git status` was clean for both builds. `task release:test`
  passed at that commit.
- 2026-11-23: **the candidate.** `task release:candidate TAG=v3.0.0` exited 0
  (01:41:38Z to 01:52:07Z), and `task release:candidate-verify TAG=v3.0.0`
  passed. Apple accepted notarisation submission
  `c8ca7e69-ffb6-42bd-b140-66df3c7962f6`. `task release:app-verify` passed on
  the unpacked bundle, and its version is 3.0.0. The sha256 of each artifact:
  - `testanyware-app-v3.0.0-aarch64-apple-darwin.zip`: `34c01465b6a22d136bd78af2ca74b9840decad39c0fe53ff2bfd6e3db4f5bd04`
  - `testanyware-app.rb`: `0a4a23f5b315951067b9086c4c31d63da3e95ce2d50b636d8cfa609322c0d50c`
  - `testanyware-v3.0.0-aarch64-apple-darwin.tar.xz`: `24e5618f69bc4ec9fd8071e9f07ef34fda0359d3136863f7bde46dbeee798f93`
  - `testanyware-v3.0.0-aarch64-unknown-linux-gnu.tar.xz`: `c4059ed4742bd3c61e17dd915064f24df07b401c684ba2f5305d5a05c29a69a5`
  - `testanyware-v3.0.0-x86_64-unknown-linux-gnu.tar.xz`: `c262e3e6a09c4f7334d6fb56b4e8c2373540f9b43ab4517012f3e1d9039f18b5`
  - `testanyware.rb`: `2e578fa596ac12a9369447a27eac7f0f4402ea0da8dd5ba570d13cc04237fdc5`
- 2026-11-23: the throwaway candidate `v2.9.99` built (to 02:01:51Z) and
  verified. It only serves `release:upgrade-check`.
- 2026-11-23: `brew uninstall --cask linkuistics/taps/testanyware-app` quit
  and deleted the installed app 2.9.0. No guest ran. `open -n -g` started the
  candidate bundle, and `doctor` reported it as build 3.0.0.
- 2026-11-23: **the first Linux harness run failed** (02:03:05Z to 02:05:53Z,
  exit 201). `vm start --platform linux` ended with `VM_BOOT_TIMEOUT`: no
  route to `192.168.64.4:8648` after 60 attempts in 118 s. `doctor` reports
  that no guest connection has shown the Local Network grant of the new
  bundle. So the missing grant is the likely cause, and a run after the grant
  tests it. The log sha256 starts `e78c5b6e`.
- 2026-11-23: the owner allowed TestAnyware in Local Network.
- 2026-11-23: **the second Linux harness run failed** (02:06:29Z to
  02:15:45Z, exit 201, 537.9 s). The grant fixed the first failure: the Linux
  guest started, the v3.0.0 binary ran in it, and the library and OCR checks
  passed. The macOS guest `testanyware-be4ad089` then started, and the
  readiness wait panicked at `linux-host-harness.rs:1763`: "golden never became
  ready: snapshot had no Finder menu bar `File` item". The second harness run
  of v2.9.0 failed the same way, and its cause stayed unknown. The log sha256
  is `beb8841343c8ec8fe477bc2ec534b6a19c1dde6ef7e2e84b37c5a36b8bc16136`.
- 2026-11-23: the third run has a read-only observer, so a repeat of the
  failure shows the content of the guest screen. A pass alone does not explain the
  failure of the second run.
- 2026-11-23: **the third Linux harness run failed** (02:16:46Z to 02:21:36Z,
  exit 201, 265.0 s). The macOS guest was ready, and 10 of the 11
  endpoint-driven checks passed. `screen-find-text` ended with `OCR_TIMEOUT`.
  The log sha256 is
  `076ee187ae2e26e83e7a8c4fe234908daa351e6d2b841619c9c0ac97a588bc01`.
- 2026-11-23: the observer of the third run saw the Finder menu bar with
  `File` at its first poll. So this run says nothing about the readiness
  failure of the second run, and the cause of that failure stays unknown. One
  `agent snapshot` of the observer exited 1 with empty stderr, and the next
  poll succeeded.
- 2026-11-23: **no further retry.** A later pass would not explain either
  failure. The record of `harness-ocr-recognizer-k55` says that a miss of the
  OCR deadline needs its own leaf, so `linux-ocr-first-call-k15` now runs ahead
  of this leaf. This leaf stays live.
- 2026-11-23: **nothing is published.** The remote has no tag `v3*` or
  `v2.9.99`, and `gh` finds no release `v3.0.0`. `main@origin` is still
  `306cb659`.
- 2026-11-23: **the state of this Mac when this session ended.**
  - The Homebrew formula is still 2.9.0.
  - The cask is uninstalled, so `/Applications/TestAnyware.app` does not exist.
  - The v3.0.0 candidate app runs from
    `target/candidates/v3.0.0.checks/app`, and it holds the Local Network
    grant. A rebuilt app needs the grant again.
  - No guest runs.
  - The local tags `v3.0.0` and `v2.9.99` name `ee1b9e88`. Their candidates
    stay in `target/candidates`.
- 2026-11-23: **when this leaf resumes.** If `linux-ocr-first-call-k15` changes
  product code, delete both local tags and their candidate directories, then
  tag the new commit and rebuild. Then run the Linux harness and
  `release:upgrade-check`. The section "Draft release notes" holds the text
  for the release.
- 2026-11-23: **the new tags.** `linux-ocr-first-call-k15` changed product
  code, so this leaf deleted both local tags and their candidate directories.
  `git tag -a v3.0.0` and `git tag -a v2.9.99` now name
  `f4cd0c3b9ad3f134a1c1ccc679bcc55c5a8a5414`. HEAD was that commit and the tree
  was clean for both builds. `task release:test` passed at that commit.
- 2026-11-23: **the new candidate.** `task release:candidate TAG=v3.0.0` exited
  0 (03:10:45Z to 03:16:31Z), and `task release:candidate-verify TAG=v3.0.0`
  passed. Apple accepted notarisation submission
  `3b01e1d7-30d4-491f-a770-26ae302999f9`. `task release:app-verify` passed on
  the unpacked bundle, and its version is 3.0.0. The sha256 of each artifact:
  - `testanyware-app-v3.0.0-aarch64-apple-darwin.zip`: `001fb30e45824cf257e5809e9f32ea95bd44dc5fc2ee9761a27358a62a1a14d8`
  - `testanyware-app.rb`: `2f8ffe45b9d30b924b96c6eee52f21117cfff5fa3d9481c2b807215e47f39a3c`
  - `testanyware-v3.0.0-aarch64-apple-darwin.tar.xz`: `122cd3f4ec7d44bd71ae2b53efad9838405a2bdeee3d07589116302d8b1efdf5`
  - `testanyware-v3.0.0-aarch64-unknown-linux-gnu.tar.xz`: `be0d9e198c4f85509ca99fa46d0f2d861580cefddb5b3ccb70ab4f17cf3ba10c`
  - `testanyware-v3.0.0-x86_64-unknown-linux-gnu.tar.xz`: `50cf8a6a8a41001f0d1b21c8d3b67da4c787e7cf39e2e289455ae211a0750196`
  - `testanyware.rb`: `3ca48478b0a6da3cb8d0deb185fc358537f8831dcaaae49171706e275a95f728`
- 2026-11-23: the new throwaway candidate `v2.9.99` built (to 03:23:08Z) and
  verified. It only serves `release:upgrade-check`.
- 2026-11-23: the new candidate app runs from
  `target/candidates/v3.0.0.checks/app`. A Linux guest started through it in
  20 s, and `screen size` read 1920x1080. `doctor` still says that no guest
  connection has shown the Local Network grant.
- 2026-11-23: **the first harness run at `f4cd0c3b` failed** (03:25:18Z, exit
  201, 354.28 s). The Linux guest started, the 6 endpoint-free checks passed,
  and the OCR provisioning passed. The macOS guest `testanyware-6c1f8a1b` then
  failed the readiness wait with the error of the earlier second run: "snapshot
  had no Finder menu bar `File` item". The OCR check did not run. The log
  sha256 is `6f08f292b3edfd66991a158d4671ce6f32641f3e505d9d9761373479ddc698b9`.
  After this run, `doctor` reports that a dial to the guest subnet connected.
- 2026-11-23: **what the wait tests.** `focusedAppMenuBar` of the agent returns
  the menu bar of the focused app, whichever app that is. So the wait can fail
  while a focused app has no `File` menu, although the error names Finder. This
  is a hypothesis. No run kept the snapshot of a failed wait.
- 2026-11-23: the next run has a read-only observer. It records the focused app
  and the menu labels at each poll, and it saves the screen if `File` is still
  absent after 30 s. That run also tests the 60 s OCR deadline.
- 2026-11-23: **the second harness run at `f4cd0c3b` passed** (from 03:33:08Z,
  253.12 s). It ran the archive with sha256 `be0d9e19…`, and the binary reported
  `testanyware 3.0.0`. All 6 endpoint-free and all 11 endpoint-driven checks
  passed. The cold EasyOCR daemon answered `screen find-text` in 17.5 s, so the
  earlier 15 s deadline would have failed this run. The log sha256 is
  `6f2ad84bff43d650e0a371720197607753fec13c6bbf557f555b7e83763b4dfb`.
- 2026-11-23: **the readiness failure stays unexplained.** The observer polled
  the guest 9 times. Each poll showed Finder as the focused app with `File`. So
  this pass does not test the focused-app hypothesis. The failure occurred in
  one run of v2.9.0 and two runs of v3.0.0, and it is a known limit of this
  gate.
- 2026-11-23: **`release:upgrade-check` passed** (03:38:02Z to 03:40:17Z, from
  2.9.99 to 3.0.0). The owner answered the first-start dialog. `brew upgrade
  --cask` took 2 s, and Homebrew reopened the app as build 3.0.0. The signing
  identity stayed the same, and the executable UUID changed. The guest
  `upgrade-check` kept its tart process, the new app adopted it, and the first
  command succeeded in 1 s.
- 2026-11-23: **what the owner saw in the upgrade check.** After the upgrade,
  no "downloaded from the Internet" dialog and no Local Network alert appeared.
  System Settings shows one row `TestAnyware.app` and one row
  `TestAnyware Dev.app`.
- 2026-11-23: `release:upgrade-check-clean` left no guest, no installed app, no
  tap, no `v2.9.99` directory and no local tag `v2.9.99`.
- 2026-11-23: the draft release notes hold at `f4cd0c3b`. `vm create-golden
  --dry-run` names macOS 27.2.1 (26B131), and Homebrew lists tart 2.42.0.
- 2026-11-23: **the owner chose to investigate before the release.** So
  `golden-readiness-wait-k16` runs ahead of this leaf, and this leaf stays live.
  In the three candidates, each run without an observer failed the wait, and
  each run with one passed it. So the pass at `f4cd0c3b` may depend on its
  observer.
- 2026-11-23: **nothing is published.** The remote has no tag `v3*` or
  `v2.9.99`, and no release `v3.0.0` exists.
- 2026-11-23: **the state of this Mac when this session ended.**
  - The Homebrew formula is still 2.9.0.
  - The cask is uninstalled, so `/Applications/TestAnyware.app` does not exist.
  - The v3.0.0 candidate app runs from `target/candidates/v3.0.0.checks/app`.
  - No guest runs.
  - The local tag `v3.0.0` names `f4cd0c3b`, and its candidate stays in
    `target/candidates/v3.0.0`.
- 2026-11-23: **when this leaf resumes.**
  - If `golden-readiness-wait-k16` changed only `.grove/`, the candidate stands.
  - Otherwise, delete the local tag and the candidate directories. Then tag the
    new commit and build `v3.0.0` and a new throwaway `v2.9.99`. Run the Linux
    harness and `release:upgrade-check` again.
  - Get the approval of the owner before `release:publish`.

## Draft release notes

`release-publish.sh` writes a placeholder. Run `gh release edit v3.0.0 --notes-file` with this text after it. Check each claim again if the release commit moves.

<!-- notes:start -->
### Title: TestAnyware 3.0.0

This release raises the host requirement and deletes the VNC keeper. It is a
major release because a host on an earlier macOS 27 release can lose its tart
guests.

### Breaking: the new host requirement

A macOS host must now run the latest release of macOS 27 and the latest release
of tart. At this release, these are macOS 27.2.1 and tart 2.42.0. The
requirement covers the host only. A guest runs the release that its golden
holds.

TestAnyware does not check these versions at run time. Upgrade the host before
you install this release.

### The VNC keeper is gone

From 2.2.0 to 2.9.0, each tart guest had a VNC keeper. The keeper was a detached
process that held one connection to the VNC port of the guest. It prevented a
crash of Apple's VNC server on macOS 27.0.1. macOS 27.2.1 does not have that
crash, so this release deletes the keeper and the hidden `_vnc-keeper` command.

On an earlier macOS 27 release, a tart guest can now stop when a `screen` or
`input` command runs 60 seconds or more after the last one.

### Each golden holds the latest release of its major

A golden now holds the latest release of its major at the time of its build
(ADR-0027). To refresh a golden, run `testanyware vm create-golden` again.

- The first macOS `vm create-golden` deletes the base VM of 2.9.0,
  `testanyware-base-macos-27`. It builds a base of the latest release from
  Apple's IPSW. This download is about 25 GB.
- `vm create-golden --platform macos --dry-run` names the release. It also says
  whether the base exists, and it names each older base that the run deletes.
- A Linux build pulls the current Ubuntu image and installs every pending
  update.
- A build deletes the earlier golden only when the new golden is ready.

### `vm start` ends with the VNC size wait

The first VNC session after a tart boot can report 1280x720, whatever the real
screen size is. `vm start` of a tart guest now ends with the VNC size wait. So
the first `screen size` or `screen capture` after it reads the real size, not
1280x720.

The wait stops after 30 seconds. If it reaches that limit, `vm start` prints a
warning and still succeeds.

### Fixes

- OCR on macOS now runs Vision in a worker process. When a Vision request hangs
  or the worker crashes, TestAnyware replaces the worker and sends the request
  once more.
- The VNC client now decodes a ZRLE rectangle that tart sends with a stale size
  after a resize.
- `screen capture` now crops by the size of the frame that it captured.
- The first read of the EasyOCR daemon now has a deadline of 60 s, not 15 s. A
  cold read on a host with no idle CPU took up to 31.5 s.
<!-- notes:end -->
