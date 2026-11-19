# lifecycle-release-k226

## Goal

Publish the next TestAnyware release. It carries the Host CLI and agent
changes of leaves 61 to 66.

## Context

- Taskfile tasks `release:*` and the scripts `scripts/release-*.sh`.
- Earlier release leaves: release-k5 (commit 5fc9602280) and
  queue-release-k26 (commit b0d72942f3).
- The trial result of leaf k225.

## Done when

- Start only after leaf k225 records a passing trial.
- Run `release:doctor`, `release:candidate` and `release:candidate-verify`.
- Run `release:linux-harness` on the candidate.
- Run `release:upgrade-check`, `release:app-verify` and
  `release:app-notarize`.
- Run `release:publish`.
- Record how the new agents reach the guests of a consumer.
- Record the release in the spec.

## Notes

Ask the human before you upgrade the installed app on this Mac. Ask the human
for any credential that a release step needs.

## Decisions (running log)

- 2026-11-19: leaf k225 recorded **10/10, passed** for trial `trial-01`, so
  this leaf can start.
- 2026-11-19: `origin/main` holds four commits that this branch lacks. They
  carry the skill plugin and its packaging in the formula, and the tag
  `v2.8.0` names the third of them, `1809a490`. A release from this branch
  alone would drop that packaging. The change `svwyqqrw` merges `main` into
  the branch without a conflict. `main` changes only the release scripts,
  the plugin, `README.md`, two specs and `Taskfile.yml`.
- 2026-11-19: no file under `cli-rs/` or `agents/` changed after the trial's
  source commit `fee86f92`. So the merge does not change the code that the
  trial measured.
- 2026-11-19: `task release:test` passed on the merged tree.
- 2026-11-19: `task release:doctor` failed one check. `notarytool` refuses
  the profile `testanyware-notary` with HTTP 403: "A required agreement is
  missing or has expired." Every other check passed. Only the holder of the
  Apple Developer account can accept the agreement.
- 2026-11-19: the public surface since `v2.8.0` is additive. It adds the
  command `file cancel`, the schema `file-cancel.json` and optional fields in
  `file-exec.json`. So a minor version fits.
- 2026-11-19: new agents reach a consumer's guests only through a new golden.
  The formula installs the agents under `share/testanyware/agents`, and
  `vm create-golden` reads them from there. No command replaces the agent of
  an existing golden. With an old golden, the new Host CLI reads the legacy
  record, as the spec's compatibility table states.
- 2026-11-19: **the version is v2.9.0.** The human chose it from v2.9.0,
  v3.0.0 and v2.8.1.
- 2026-11-19: the human accepts the Apple agreement and then tells this
  session. After that, `release:doctor` runs again.
- 2026-11-19: the human chose to run `release:upgrade-check` as the leaf
  states. The cask leaves `/Applications` for the check, a throwaway
  candidate `v2.8.99` is built, and the human answers the dialogs.
- 2026-11-19: the change `svwyqqrw` is the pure merge that the tag will name.
  The running log is the change above it. Before the build, the remote has
  no tag `v2.9*` or `v2.8.99` and no release `v2.9.0` or `v2.8.99`. The
  control pattern `v2.8*` listed `v2.8.0`, and `gh` found release `v2.8.0`.
- 2026-11-19: after the agreement, `task release:doctor` passed every check.
- 2026-11-19: **the tags.** `git tag -a v2.9.0` and `git tag -a v2.8.99` both
  name `55af0aa84eea9eebbedebd1c2362edecf5368209`, the merge. Both stay
  local. HEAD was that commit and `git status` was clean for both builds.
- 2026-11-19: **the candidate.** `task release:candidate TAG=v2.9.0` exited 0
  (11:37:42Z to 11:45:57Z), and `task release:candidate-verify TAG=v2.9.0`
  passed. The sha256 of each artifact:
  - `testanyware-app-v2.9.0-aarch64-apple-darwin.zip`: `cb2332bc6e6633148bb05ec6192cfb092f56f00d3ca25ad7d1d58a82ba81785e`
  - `testanyware-app.rb`: `e9053ed32fd0ef80bd3da1e8df9d41e568f346971f784ea8032818d568cbee02`
  - `testanyware-v2.9.0-aarch64-apple-darwin.tar.xz`: `d9c03b56414e37e823d8581b8277c5a2f2c43b97994c6d451a7c09a571c83f27`
  - `testanyware-v2.9.0-aarch64-unknown-linux-gnu.tar.xz`: `aacd97940005658460792867b68229f11f35c3baea1fde4ce0c7eb784673384d`
  - `testanyware-v2.9.0-x86_64-unknown-linux-gnu.tar.xz`: `61b22e92afd90b5822c0e22f12792965d941660b75dc7c8d171d87c03aa1f988`
  - `testanyware.rb`: `2603be3d704f69a43b87313516e3b621249d5c2875773d2f3c534f9266f5d7db`
- 2026-11-19: **notarisation.** The candidate build runs
  `scripts/notarize-app.sh`, the script of `release:app-notarize`, on its
  bundle (`scripts/release-build.sh:400`). Apple accepted submission
  `5a8eceaa-493a-4b25-98ac-2bb51ec030aa`, and `stapler validate` passed. This
  leaf runs no second notarisation of the same bundle.
- 2026-11-19: **the bundle.** `task release:app-verify` on the unzipped
  bundle in `target/candidates/v2.9.0.checks/app` reports
  `dev.antony.TestAnyware` of team `TA43A4RUP3`, the hardened runtime,
  notarisation and the staple. The bundle version is 2.9.0. The macOS CLI
  reports `testanyware 2.9.0`, binary sha256
  `808eb51742ff48f310390c288fcdb370fcd6b2f599e88f80b14d3d30547820af`.
- 2026-11-19: **the agents of the release.** The `agents/` and `cli-rs/`
  trees of `55af0aa8` equal those of the trial source `fee86f92`. The 11
  files of the Linux agent in the macOS archive match the trial's frozen
  listing by sha256. The macOS agent of the archive has sha256
  `aeec230a09722abf2795bb49bbafbf122d7eb69afd9109ca83da48a58880b6cc`. The
  trial's agent, `7998bee7…`, was built from the same source in another
  workspace, so the two binaries differ.
- 2026-11-19: **the throwaway candidate.** `task release:candidate
  TAG=v2.8.99` exited 0 (11:46:10Z to 11:51:38Z), and
  `release:candidate-verify` passed for it. It
  only serves `release:upgrade-check`, and `release:upgrade-check-clean`
  removes it and its tag.
- 2026-11-19: no guest ran. `brew uninstall --cask
  linkuistics/taps/testanyware-app` quit and removed the installed app 2.8.0
  for the upgrade check. Then `open -n -g` started the candidate bundle from
  `target/candidates/v2.9.0.checks/app`, and its socket reported build 2.9.0.
- 2026-11-19: **the first Linux harness run failed** (11:53:51Z to
  11:54:38Z, exit 201). `vm start --platform linux` exited 7 with
  `VM_BOOT_TIMEOUT`, cause `local_network_denied`: no route to
  `192.168.64.2:8648` after 10 attempts in 19 s. The new executable UUID of
  the candidate app needs the Local Network grant again. The archive sha256
  is `aacd9794…`, and the binary sha256 is
  `4d20739bfb73ab2def8aa24b8a5c5667e8027068c8116d1ff8f45f9b5dcb2d9a`. The
  failed logs are kept in `target/candidates/v2.9.0.checks/linux-run1-failed/`
  (`harness.log` sha256 `1e55ce58…`). The guest `testanyware-6acbebdf` was
  not kept, and no VM remained.
- 2026-11-19: the human allowed TestAnyware in Local Network. Then the app
  reported `local_network` state `not_yet_seen`.
- 2026-11-19: **the second Linux harness run failed** (13:00:24Z to
  13:10:58Z, exit 201, 619.9 s). The Linux guest started, the v2.9.0 binary
  ran in it (`testanyware 2.9.0`), and the library and OCR checks passed.
  Then the macOS guest `testanyware-6676c155` started, and the readiness wait
  panicked at `linux-host-harness.rs:1763`: "golden never became ready:
  snapshot had no Finder menu bar `File` item". Each snapshot command
  succeeded during the 120 s wait. The harness keeps no snapshot content.
  The logs are kept in `target/candidates/v2.9.0.checks/linux-run2-failed/`
  (`harness.log` sha256 `d5f28ec9…`). Both guests stopped.
- 2026-11-19: three diagnostics did not reproduce it. Each started one
  macOS clone with `vm start --platform macos --json` and polled
  `agent snapshot --vm <id> --json`, as the harness does. Finder's menu bar
  with `File` appeared after 4 s with the candidate CLI. It appeared after
  1 s with the harness's debug CLI, and after 2 s beside a Linux guest. It stayed
  for each whole window (150 s, 45 s and 60 s). Every clone stopped. So the
  CLI and a second guest alone do not cause it. The harness's earlier eight
  minutes of OCR provisioning stay untested as a condition. The cause is
  unknown.
- 2026-11-19: **the third Linux harness run passed** (13:19:47Z to
  13:24:08Z, 247.6 s). It ran on the same archive and binary as the two
  failed runs. All 6 endpoint-free and all 11 endpoint-driven checks passed,
  with screen record and screen find-text on aarch64 Linux, and it printed
  `test result: ok. 1 passed; 0 failed`. The log is
  `target/candidates/v2.9.0.checks/linux/harness.log`, sha256
  `1bad5ef011755e998fa8e05214475ae355dd6f0f6804921653f0b2a8ca7d8891`.
- 2026-11-19: a read-only observer ran beside the third run. It saved the
  macOS guest's snapshots and screenshots in
  `target/candidates/v2.9.0.checks/linux-run3-observer/`. Finder's menu
  bar showed 5 s after the guest appeared. The failure of the second run did
  not recur, so its cause stays unknown. This is a visible limit of the
  harness gate, not a repair. Run 1 has a known cause, the Local Network
  grant of the new executable.
- 2026-11-19: the session quit the candidate app by its bundle identifier.
  No app answered on the socket, and no guest ran.
- 2026-11-20: **the first run of `release:upgrade-check` failed** at
  08:32:41Z. It installed v2.8.99 from the local tap (UUID
  `DD8B44F0-2D21-374A-9144-BFA39E90E73C`). The human answered the
  first-start dialog, and the app answered as build 2.8.99. `vm start`
  reached a healthy agent of the guest `upgrade-check` in 20 s. Then the
  first `agent snapshot` through 2.8.99 failed. The script captures that
  output and does not print it, so the cause is unknown. The same snapshot,
  repeated by hand on the running guest, returned `ok: true`. The log and
  that snapshot are kept in
  `target/candidates/v2.9.0.checks/upgrade-run1-failed/`. The script resumes
  from the state of the Mac when it runs again.
- 2026-11-20: **the resumed `release:upgrade-check` passed** (08:33:42Z to
  08:34:05Z). The baseline snapshot through 2.8.99 returned 5 windows. `brew
  upgrade --cask` took 2 s, and Homebrew reopened the app as build 2.9.0. The
  signing identity stayed `dev.antony.TestAnyware` of team `TA43A4RUP3`.
  The executable UUID changed to `42C859EC-AB9C-36DD-B61B-C34456C82892`. The
  new bundle is notarised and stapled, and its quarantine flags `01c1` keep
  the approval of the first start. The guest kept its tart process and
  keeper, the new app adopted it, and the first command succeeded in 1 s.
  The log is in `target/candidates/v2.9.0.checks/upgrade-run2-pass/`.
- 2026-11-20: `release:upgrade-check-clean` left no guest, no app, no tap,
  no v2.8.99 directory and no local tag `v2.8.99`. The `v2.9.0` candidate
  stays.
- 2026-11-20: **the state before the approval.** `origin/main` is
  `3a7c513b`, an ancestor of `v2.9.0`, so the push is a fast-forward of 255
  commits. The tap clone at `~/Development/Delivery/homebrew-taps` is a jj
  repository, and its `main` equals `main@origin` at `f0bbc9b`. The remote
  has no tag `v2.9*` and no release `v2.9.0`.
- 2026-11-20: the human saw no first-start dialog after the upgrade.
- 2026-11-20: **the human approved the publication.** The question named the
  tag `v2.9.0`, the commit `55af0aa8`, the sha256 of the artifacts and the
  two failures of unknown cause. The human chose to push `main` first, and
  to install 2.9.0 on this Mac after the publication.
- 2026-11-20: **`main`.** `jj bookmark set main -r svwyqqrw` and `jj git push
  -b main` moved `origin/main` forward from `3a7c513b` to `55af0aa8`. GitHub
  warned that
  `experiments/exec-reliability/evidence/termination-cutoff-k85/audit-1/verdict.json`
  is 52.64 MB, above its recommended 50 MB. It accepted the push.
- 2026-11-20: **the publication used the recorded candidate.**
  `scripts/release-publish.sh v2.9.0` verified the candidate again and pushed
  the tag (`[new tag] v2.9.0 -> v2.9.0`). It created
  `https://github.com/Linkuistics/TestAnyware/releases/tag/v2.9.0` and pushed
  the tap commit `11b79223` "testanyware v2.9.0" to `linkuistics/taps`
  (`f0bbc9b..11b7922`). It rebuilt nothing. This session ran the script
  directly, as for v2.7.0, because `release:publish` prompts a person.
- 2026-11-20: **this Mac.** `brew upgrade linkuistics/taps/testanyware` took
  the formula from 2.8.0 to 2.9.0. `brew install --cask
  linkuistics/taps/testanyware-app` installed the app 2.9.0, and `spctl`
  reports `Notarized Developer ID` of team `TA43A4RUP3`. The fresh install
  needed the first-start dialog, and the human answered it. `testanyware
  doctor` reports the app installed and running, build 2.9.0, protocol 1-2,
  with the Host CLI and the app at the same build. It also reports that the
  login item is off and that no guest connection has shown the Local Network
  grant yet. The release of 2.7.0 reported the same two items. The installed
  macOS agent has the candidate's sha256 `aeec230a…`, and the formula links
  the skill `testanyware` into Ravel's skill directory.
- 2026-11-20: **the spec.** `docs/specs/guest-command-lifecycle.md` has a new
  section "Release". It records release 2.9.0, its checks and the failed
  runs. Its subsection says how new agents reach a consumer's guests: only
  through a golden that `vm create-golden` builds again after the upgrade.
- 2026-11-20: **the human's view of test artifacts.** The human asked why
  `verdict.json` is so large. Its two maps of subject hashes, before and
  after, cover 102,254 paths, nearly all under
  `experiments/exec-reliability/evidence/`. They make 54 of its 55 MB. The
  human said: "I think in general we keep too much/many testing artifacts."
  The repository tracks 362,382 evidence files (7.3 GB on disk, a 340 MiB
  pack), and they are now on `main`. This leaf removed nothing. A pruning,
  or a rewrite of `main`, is the human's decision. Leaf k227 should put this
  question in its handoff.
- 2026-11-20: **residual processes for k227.** Four `pytest` processes of
  `worker-overlap-k146` (PIDs 64549, 64570, 64602 and 64617, started
  2026-11-16) and one `xctest` of `OwnedProcessFixtureTests` (PID 70224,
  started 2026-11-18) still run. This session did not start them and left
  them alone. The resource audit of k227 owns them.
- 2026-11-20: **resources of this leaf.** No guest of this leaf runs. The
  candidate app quit, and the upgrade-check cleanup removed v2.8.99, its tap
  and its tag. The directories `target/candidates/v2.9.0` and
  `target/candidates/v2.9.0.checks` stay as the evidence of this release.
  Git ignores them.
- 2026-11-20: **work that no release carries.** No commit after `v2.9.0`
  changes product code. Only this leaf's log and spec section follow it.
- 2026-11-20: **no review leaf follows this leaf.** It changed no code. The
  publication waited for the human's approval, and each claim above names
  its output. No framework API of a changing version entered this leaf.
