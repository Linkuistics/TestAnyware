# release-2-5-0-k3

## Goal

Publish TestAnyware v2.5.0 with the fix of `guest-stream-end-k2`, as the root
brief describes. Then upgrade this Mac and record that upgrade.

## Context

- The decision log of `plan-k1`: the version, the upgrade evidence, the void
  candidate and the harness failure.
- The leaves of the v2.4.0 release, in the parent of commit `95a3484`, under
  `.grove/24-k21/`. `candidate-rebuild-k53` moved an unpushed tag and rebuilt.
  `release-publish-k37` asked for the approval and published.
- The Taskfile tasks `release:*` and `app:herdr-acceptance`.

## Done when

- Before the rebuild, the remote has no tag `v2.5.0` and no release `v2.5.0`.
  If either exists, stop and ask the owner, because the version is then
  published.
- `task release:doctor` passes.
- The local tag `v2.5.0` names the commit to build, and that commit holds the
  fix. HEAD is that commit, and the tree is clean. Before the build,
  `target/candidates/v2.5.0` and `target/candidates/v2.5.0.checks` are gone.
- `task release:candidate TAG=v2.5.0` and `task release:candidate-verify
  TAG=v2.5.0` pass. The unzipped bundle passes `task release:app-verify`. The
  decision log records the tag, the commit and the sha256 of each artifact.
- `task release:linux-harness TAG=v2.5.0` passes. The log records the hashes
  that it prints.
- The herdr acceptance run passes on the candidate's bundle, with the CLI of the
  candidate's macOS archive.
- The owner approves the publication. The question names the tag, the commit
  and the sha256 of the artifacts. It also proposes to push the commits to
  `main` before the tag, as for v2.4.0.
- `task release:publish TAG=v2.5.0` publishes the recorded candidate and
  rebuilds nothing.
- This Mac runs release 2.5.0 of the formula and of the cask, and
  `testanyware doctor` reports the app.
- The decision log records the real upgrade from 2.4.0 to 2.5.0:
  - the signing identity before and after, and the executable UUID
  - the first-start approval, and any Local Network alert
  - the adoption of the running guests, and the first command.
- The spec ("Packaging and identity", "Open risks") and ADR-0020
  ("Consequences") record release 2.5.0 and the second sample of an upgrade.

## Notes

- The local tag `v2.5.0` names `f0108944`, a void candidate without the fix.
  Never publish it. In the v2.4.0 release, the auto-mode classifier refused the
  deletion of an unpushed tag, and the owner ran the command. Expect the same.
- The herdr acceptance script refuses to run while another app answers. Quit
  the installed app first, and open it again afterwards. The candidate app has
  a new executable UUID, so macOS may show the Local Network alert. Then the
  owner allows it, and the run goes again.
- For the real upgrade, the installed app runs first. Then Homebrew quits it and
  opens the new one. Guests keep running through the upgrade.
- The Linux harness and the acceptance run each start a macOS guest, and the
  acceptance script needs fewer than two running tart guests. Other sessions
  can hold both slots. Ask the owner before you stop a guest that this grove did
  not start.
- `origin/main` is `345faf54`. The commits after it are on no remote branch.

## Decisions (running log)

- **The remote was clean.** Before the rebuild, `git ls-remote --tags origin 'v2.5*'` printed nothing and `gh release view v2.5.0` said `release not found`. The control `v2.4*` listed `v2.4.0`. `task release:doctor` passed every check.
- **The tag moved.** `git tag -d v2.5.0` removed the void tag (it named `f0108944`), and the command ran without a refusal. Then `git tag -a v2.5.0 -m "Release v2.5.0" 6011dba9` named the commit of the fix. HEAD was that commit, `git status` was clean, and I removed `target/candidates/v2.5.0` and `target/candidates/v2.5.0.checks` before the build.
- **The candidate.** `task release:candidate TAG=v2.5.0` exited 0, and `task release:candidate-verify TAG=v2.5.0` passed. Tag `v2.5.0`, commit `6011dba9e04f807afcf835e3b3d74d721aef16ae`. The sha256 of each artifact:
  - `testanyware-app-v2.5.0-aarch64-apple-darwin.zip`: `8dfa3a58db50b7d6a8e4976c4700f2318f23371965a66711a7317756e43ec246`
  - `testanyware-app.rb`: `7d55cfcd550f08f149c6697b8dd07f986c4160dcfd3d7da844206ae934c813c8`
  - `testanyware-v2.5.0-aarch64-apple-darwin.tar.xz`: `3a16d7b8d2cd72f2f2b31b938586aa393d6f780058b6a173bd927dbc842c6be0`
  - `testanyware-v2.5.0-aarch64-unknown-linux-gnu.tar.xz`: `d7c4fc1d5cbae5ca8ca6c16e8dbb7a8efd522cf1d5efacb5d64e6d1c07ef4b80`
  - `testanyware-v2.5.0-x86_64-unknown-linux-gnu.tar.xz`: `1dd97a85ad99ff0b5e89db46081588ef8de54d1ac25f98ac10dfeae232da9a86`
  - `testanyware.rb`: `fd9e0c301072260ab3bbc6210362862143a151df38d0a6f9a320f3174c5d3f50`
- **The bundle.** I unzipped the app zip into `target/candidates/v2.5.0.checks/app`. `task release:app-verify` reports `dev.antony.TestAnyware` of team `TA43A4RUP3`, the hardened runtime, notarisation and the staple. The executable UUID is `D1773BE8-29F9-3DAB-9274-C22C602C3ADE`, and the installed 2.4.0 has `BC3E960A-BC3A-3178-9667-29BDEDC4334C`.
- **Both macOS slots are full.** `koine-verify-44327` (started 15:15) and `koine-verify-58679` (started 20:11, used 16 minutes before this check) run under `tart run`. This grove did not start them, so I ask the owner before I stop one. The installed app (pid 57228) holds 183 descriptors of its 256.
- **The owner chose the slot and the restart.** The owner allowed me to stop `koine-verify-44327` and to restart the installed app. `testanyware vm stop koine-verify-44327` worked. I quit the app, and it quit within 15 s. I opened `/Applications/TestAnyware.app` again (pid 60930, 34 descriptors). While I did this, another session replaced the Koine guests: `koine-verify-58679` was gone, and `koine-verify-23707` ran.
- **The first harness run failed on a slot, not on the candidate.** `task release:linux-harness TAG=v2.5.0` ran on the aarch64 Linux archive (sha256 `d7c4fc1d…4b80`) and its binary (sha256 `e95cc174…611b`). The 6 endpoint-free checks passed, and the library check read 350 native libraries and found no foreign need. Then the start of the macOS guest failed after 224 s with `macos guest limit reached: 2 VMs are already running (koine-verify-23707, koine-verify-42801, testanyware-1b90caf1)`. Another session started `koine-verify-42801` after my check. The harness stopped its own `testanyware-1b90caf1`. The log is `target/candidates/v2.5.0.checks/linux/harness.log`.
- **The Linux harness passed.** The owner chose to pause the Koine session, and a slot was free at once, so I ran `task release:linux-harness TAG=v2.5.0` again. It ran on the aarch64 Linux archive (sha256 `d7c4fc1d5cbae5ca8ca6c16e8dbb7a8efd522cf1d5efacb5d64e6d1c07ef4b80`) and its binary (sha256 `e95cc1743be05a5e55d1c24b8d1ecf363339778c9e9cee4da52af1dcd223611b`). The 6 endpoint-free checks passed, the library check read 350 native libraries and found no foreign need, the harness started and stopped `testanyware-8f60deef`, and it printed `test result: ok. 1 passed; 0 failed` after 241 s. The log is `target/candidates/v2.5.0.checks/linux/harness.log`. The restarted app made the difference: the first failure of the old app (`Too many open files`) did not occur. The binary has the same hash as in the void candidate, because the fix is in the app crate, which the Linux CLI does not contain.
- **The herdr acceptance run passed on the candidate.** I quit the installed app, as the notes direct. The first run used `task app:herdr-acceptance APP=<bundle>`, which sets `TESTANYWARE_BIN` itself to the dev build of the CLI (the notice said "Host CLI is build 0.0.1"), so it does not meet the done-when. I ran `scripts/herdr-acceptance-live.sh` directly with `TESTANYWARE_BIN` on the CLI of the candidate's macOS archive (`testanyware 2.5.0`, sha256 `cc711f6bb93c357c61b6e74f9159421b0fb98dcdadb6baad6603f5f8afe10895`) and the unzipped bundle (build 2.5.0), from herdr session `w2E:p1`. `vm start` reached a healthy agent in 14 s. The agent, screen, input and file steps passed, and the six controls each failed with `APP_UNREACHABLE` when no app answered. `vm stop` left no guest and no sidecar. The script printed `PASS`. No Local Network alert appeared. The candidate app's executable UUID differs from the installed app's, but macOS did not ask again on this run. Afterwards I opened `/Applications/TestAnyware.app` again (2.4.0, pid 92579), and the Koine guest `koine-verify-23707` kept running.
- **The state before the approval.** `origin/main` is `345faf54` and is an ancestor of `6011dba`, which is 15 commits ahead. `release:candidate-verify` still passes. The tap clone is now at `~/Development/Delivery/homebrew-taps`, not at the default `~/Development/homebrew-taps` of `release-publish.sh`, so the publication sets `TESTANYWARE_TAP_DIR`. The clone is clean, on a detached HEAD at `8d2cdfd`, which is also `main` and `origin/main`.
- **The owner approved the publication.** The question named the tag `v2.5.0`, the commit `6011dba9`, and the sha256 of each artifact, and it proposed the push of `main` first. The owner chose to push `main` first.
- **The publication used the recorded candidate.** `git push origin 6011dba9:refs/heads/main` fast-forwarded `origin/main` from `345faf5` to `6011dba`. In the tap clone, `git switch main` moved nothing. The tap clone is at `~/Development/Delivery/homebrew-taps` now, so I ran `TESTANYWARE_TAP_DIR=$HOME/Development/Delivery/homebrew-taps scripts/release-publish.sh v2.5.0` directly, because `release:publish` has a prompt for a person. The script verified the candidate again, pushed the tag (`[new tag] v2.5.0 -> v2.5.0`), created `https://github.com/Linkuistics/TestAnyware/releases/tag/v2.5.0`, and pushed the commit `8f44dde` `testanyware v2.5.0` to `linkuistics/taps` (`8d2cdfd..8f44dde`). It rebuilt nothing.
- **The real upgrade of this Mac.** Before: the formula and the cask were 2.4.0, the app ran as pid 92579, the signing identity was `Developer ID Application: Antony Blakey (TA43A4RUP3)` with identifier `dev.antony.TestAnyware`, and the executable UUID was `BC3E960A-BC3A-3178-9667-29BDEDC4334C`. I ran `brew upgrade linkuistics/taps/testanyware` and `brew upgrade --cask linkuistics/taps/testanyware-app`. Homebrew said "Quitting application", replaced the bundle, and "Reopening 1 application closed during upgrade". After: both are 2.5.0, the identity and the team are the same, `spctl` says `accepted` and `Notarized Developer ID`, and the executable UUID is `D1773BE8-29F9-3DAB-9274-C22C602C3ADE`. The new app (pid 40391) holds 33 descriptors.
- **The first-start approval and the Local Network alert.** The quarantine attribute of the new bundle is `01c1`, which includes the bit of the user's approval, so the first start needed no dialog. This session cannot see the screen. No command failed with `local_network_denied`, and `testanyware doctor` reported `local network: a dial to 192.168.64.7:8648 connected` on its first run. So I record no alert that stayed unanswered. I cannot say whether the owner allowed one in the first seconds.
- **The adoption and the first command.** The guest `koine-verify-23707` kept its tart process (pid 25793) and its keeper (pid 26816), and `testanyware vm list` shows it with its agent. `testanyware screen size --vm koine-verify-23707` printed `1920x1080` through the new app. `doctor` also shows `adoption: not adopted: koine-verify-54176 (process_gone)`, a stale record of a guest that no longer runs, which is not from this upgrade. It shows one failure, `login item: the app does not start at login`, as after 2.4.0. That step is the person's.
- **The documents.** The spec ("Packaging and identity") records release 2.5.0 and the second sample of an upgrade, and "Open risks" counts the second sample. ADR-0020 ("Consequences") records the same. Neither changes a decision. The `ste-lint` run on the changed passages shows no hard finding.
- **Source for the API.** No framework API of a changing version entered this leaf.
- **Review.** I spent no in-session reviewer. The publication gated on the owner's approval, and every claim above is an output shown in this log.
