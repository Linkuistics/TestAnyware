# plan-k1

## Goal

Establish what this grove builds, in the owner's words. Then make the release
v2.5.0 in this session, as the root brief describes. If the work proves bigger,
cut the leaves that make it.

## Context

- The previous grove's root brief, items 5 and 6 of its decomposition. Read it
  with `git show 95a3484^:.grove/_BRIEF.md`.
- The v2.4.0 release leaves in the same commit, under `.grove/24-k21/`. The
  leaf `release-publish-k37` shows the approval question and the publication.

## Done when

- The decision log records the owner's words and each decision.
- The "Done when" of the root brief holds, or the tree holds the leaves that
  make it hold.

## Notes

- The release changes no code, so it needs no new test seam. It runs the
  existing Taskfile live checks of the spec.

## Decisions (running log)

- **The requirement.** On 2026-10-24 the owner said:
  <!-- tc: off because this quotes the owner verbatim -->
  "I wanted you to do a release, but you ended the grove after deleting the grove files".
  <!-- tc: on -->
  The previous grove's leaf `default-config-file-increments-k59` recorded that
  its work ships with the next release, together with `linux-golden-disk-size-k57`.
  It cut no release leaf. Then `finish-k61` deleted the tree. So this grove
  publishes the next release.
- **No tree.** The path to done is clear. The pipeline that published v2.4.0 is
  in the Taskfile, and `task release:doctor` passed every check on 2026-10-24.
  The tap clone at `~/Development/homebrew-taps` is clean and on `main`. The
  owner is present for the approval. So this leaf makes the release, and it cuts
  no leaf.
- **No grilling.** Three questions were open: the version, the evidence of the
  upgrade, and a free macOS guest slot. Their answers do not depend on each
  other, so the threshold of three interdependent questions is not met. The owner
  answered each one.
- **What ships.** Twelve local commits follow `v2.4.0` (`345faf54`). Outside
  `.grove/` and the documents, they change the app's code and tests, the
  configuration crate, the golden creation code, `Taskfile.yml` and three live
  check scripts. No command, flag, environment variable, exit code or `--json`
  field changes. One value changes: on a macOS host, the config check of
  `doctor` reports `loaded` after the first start of the app, where it reported
  `absent` before.
- **The version is v2.5.0.** The owner chose it. The release adds two features
  and keeps the CLI surface, and the releases v2.1.0 to v2.4.0 were all minor.
  Before the tag, neither this checkout nor the remote had a tag `v2.5*`. The
  control `v2.4*` listed `v2.4.0` on the remote.
- **The upgrade evidence is the real upgrade of this Mac, after publication.**
  The owner chose it over a run of `release:upgrade-check` with a throwaway
  release before publication. Eleven files name the bundle id. Of these, only
  `Taskfile.yml`, `scripts/app-bundle-live.sh` and
  `scripts/upgrade-identity-live.sh` changed since `v2.4.0`, and the build reads
  none of them. The build inputs that set the identity did not change:
  `build-app.sh`, `release-build.sh`, `notarize-app.sh`, `verify-app.sh`,
  `release_candidate.py`, the templates and the two `bundle.rs` files. The build
  refuses another bundle id or team. The owner accepts that a fault of the
  upgrade shows only after publication.
- **The guest slot.** Both macOS slots held a guest: `k26-live`, which tart last
  accessed two days earlier, and `koine-verify-54176`, which was active. The
  Linux harness and the herdr acceptance run each start a macOS guest. The owner
  consented to stop `k26-live`.
- **The candidate.** I committed the requirements as `f0108944` and tagged that
  commit with `git tag -a v2.5.0`. HEAD named the tagged commit, and
  `git status` was clean. `task release:candidate TAG=v2.5.0` exited 0 after
  about six minutes, and `task release:candidate-verify TAG=v2.5.0` passed. Tag
  `v2.5.0`, commit `f01089441e26ad34b7bfa929d5b0936a71cd11b0`. The sha256 of each
  artifact:
  - `testanyware-app-v2.5.0-aarch64-apple-darwin.zip`: `7c5519e84833838355b3550eb32604e0b0ab509eae2eb9c7b51d7ac1f43eeb20`
  - `testanyware-app.rb`: `5707b073575d9ca76248e2dc169006fa8600913d83df1f1d51b54ece33bdb983`
  - `testanyware-v2.5.0-aarch64-apple-darwin.tar.xz`: `8bb314e4e699397f0fb123cc4cc8f9ab160ae8f853087cea5609d78cfaddc210`
  - `testanyware-v2.5.0-aarch64-unknown-linux-gnu.tar.xz`: `f0110a66ac813c91760debc208cbc444388571fd522153e6e1522e66972c4f21`
  - `testanyware-v2.5.0-x86_64-unknown-linux-gnu.tar.xz`: `713732d4644b80dbd866ad8dae2929bef3016606d5aa43d8039d29b068dd5852`
  - `testanyware.rb`: `7730bf05142ea9072c5ef3f8012c9b03e066fcb7a10b0ae10a79d5aea4f909f6`
- **The bundle.** I unzipped the app zip into the new directory
  `target/candidates/v2.5.0.checks/app`. `task release:app-verify` reports
  `dev.antony.TestAnyware` of team `TA43A4RUP3`, the hardened runtime, the
  notarisation and the staple. `codesign --verify --deep --strict` says
  `valid on disk`. `spctl --assess` says `accepted` and
  `source=Notarized Developer ID`, and `stapler validate` worked. Both version
  keys say 2.5.0. The executable UUID is `8158ACB3-6493-3C8F-8FED-549FDEB074D8`.
  The installed 2.4.0 has `BC3E960A-BC3A-3178-9667-29BDEDC4334C`.
- **No Windows host archive.** The manifest names six artifacts, and none of them
  is for a Windows host.
- **One compiler warning.** The two Linux builds report unused imports in
  `testanyware-vm/src/lifecycle.rs`. That file is unchanged since `v2.4.0`, so
  the warning is not new.
- **The Linux harness failed in the running app, not in the candidate.** I
  stopped `k26-live` with `testanyware vm stop`. Then
  `task release:linux-harness TAG=v2.5.0` ran on the aarch64 Linux archive
  (sha256 `f0110a66…4f21`) and its binary (sha256 `e95cc174…611b`). The 6
  endpoint-free checks passed. The library check read 350 native libraries of
  the OCR venv and found no foreign need. Then the harness started its macOS
  guest through the app. The app answered with `IO_ERROR`: "build the runtime of
  the start: Too many open files (os error 24)". The test failed after 333 s.
  The log is `target/candidates/v2.5.0.checks/linux/harness.log`.
- **The app was at its limit of descriptors.** The installed app 2.4.0, with
  the pid 72756, started on 2026-10-23 at 00:25. It held 254 numbered
  descriptors. `launchctl limit maxfiles` gives a soft limit of 256. Of these, 122 were TCP
  connections from the app to `127.0.0.1:65387` in `FIN_WAIT_2`. Another 123
  were client connections that the app took on `testanyware.sock`. Port 65387 is the VNC server
  of the tart process of `koine-verify-54176`. That tart process held 94
  connections in `CLOSE_WAIT`. Snapshots of `lsof` are in the session's
  scratchpad, not in the repository.
- **The cause.** `relay` in `testanyware-app/src/streams.rs` calls
  `copy_bidirectional_with_sizes`, which returns only when both directions end.
  When a command ends, the client closes its stream. The app passes the
  half-close on to the guest and waits for the guest to end its direction. tart's
  VNC server never closes such a connection and sends nothing more. So the relay
  never returns, and its two descriptors stay open. Each VNC stream to a tart
  guest leaks two descriptors, and the app fails after about 120 of them. The
  socket states and the code show this. No test reproduces it yet.
- **The candidate has the same leak.** The code of the guest streams is
  unchanged since `v2.4.0`. The run says nothing about the candidate's Linux
  binary, because the failure came before the endpoint-driven band.
- **Clean-up.** The harness could not stop its Linux HUT `testanyware-9da5e0f2`.
  I deleted the stopped clone with `tart delete`. During the run, the Koine guest
  `koine-verify-44327` started, so both macOS slots are in use again.
- **The restart.** The owner chose to restart the installed app at once. I sent
  the quit event, and the app quit within 15 s. Both Koine guests and their VNC
  keepers kept running. I opened `/Applications/TestAnyware.app` again. The new
  app, build 2.4.0, adopted both guests. Its descriptors fell from 254 to 10.
- **tart releases its side.** Right after the restart, tart still held 95
  connections in `CLOSE_WAIT` on port 65387. About two minutes later it held
  none, and its descriptors fell from 115 to 22. So tart releases its side once
  the app closes its own, and a fix in the app is enough.
- **Fix first.** The owner chose to fix the leak before the release, over a
  release of the current candidate with the fix in a later release. One release
  gives consumers the fix. Each upgrade can make a person allow the app again,
  so one release costs less than two. The fix changes code, so the candidate of
  `f0108944` is void, and its hashes above describe only it. The local tag
  `v2.5.0` stays on `f0108944` until the release leaf moves it.
- **The test seams.** The owner agreed three seams for the fix. Each is an
  existing seam of the spec, so the fix adds none. They are the protocol seam,
  the relay's unit tests and a Taskfile live check. The root brief describes
  each one.
- **No design leaf.** The owner agreed the goal of the fix and its seams, so no
  agreement point remains. The spec already says that the app closes a guest
  connection "when either side closes", and that a half-close passes through.
  The code does not match the first rule. So one `impl` leaf makes the code match
  the spec and sharpens its wording.
- **The tree.** The fix brings fog into the release, so this leaf no longer
  makes the release itself. It cuts `guest-stream-end-k2` and then
  `release-2-5-0-k3`. The release leaf comes last, so the grove cannot finish
  before the release.
