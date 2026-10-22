# upgrade-identity-check-k36

## Goal

Prove on this Mac that a cask upgrade keeps the identity of the app, with the release build.

## Context

- The requirement "An upgrade keeps the app's identity", and the open risks "The release build" and "An alert after an upgrade" in the spec.
- `docs/research/testanyware-app-local-network-grant.md`, for the measurement method.
- `app-release-bundle-k22`, which makes candidates.
- `non-macos-runtime-check-k40`, which ran before this leaf on the same candidate.

## Done when

- The upgrade runs from release N to release N+1. N+1 is the release candidate that `candidate-rebuild-k53` made. N is a throwaway candidate with its own local tag and a lower version. The leaf does not rebuild N+1.
- A Taskfile live check installs N and then N+1 of the cask from a local tap with the local artifacts. Nothing is published.
- After the upgrade, no "downloaded from the Internet" dialog appears, and System Settings shows one TestAnyware row.
- The first command to a tart guest succeeds within the retry, or it fails with `local_network_denied` and a remedy that names TestAnyware. After a person allows the app again, the command succeeds.
- Each running VM keeps running through the upgrade, and the new app adopts it.
- The leaf deletes the throwaway tag and its artifacts when the check ends. They never reach `release-publish-k37`.
- The leaf records the measurements and the candidate identity in the decision log: the tag, the commit and the sha256 of the artifacts of N+1. If the result differs from the spec, it names the change that the spec and ADR-0020 need.

## Notes

- A person allows the alert, and answers the first-start dialog once. The check names these steps.
- Use two builds with different executable UUIDs. A rebuild gives them.
- A change to the code of the candidate voids the candidate. Rebuild it, and repeat the checks of `non-macos-runtime-check-k40` and this leaf.

## Decisions (running log)

- **The check is two Taskfile tasks over one script.** `task release:upgrade-check OLD=v2.3.99 TAG=v2.4.0` runs `scripts/upgrade-identity-live.sh run`. `task release:upgrade-check-clean` runs its `clean` mode. A local tap `local/upgrade-check` holds the cask of each release, with its url set to the local zip. The formula is not touched, because the cask depends on it and the installed one stands. The script reads the state of the Mac each time, so a stop at a person's step is resumable. `shellcheck` and `shfmt` pass, and the script joined the list of `task scripts:check`.
- **Release N is a throwaway candidate of its own tag.** The remote had no tag `v2.3.99`. `git ls-remote --tags origin 'v2.3.99*'` printed nothing, and the same command with `'v2.3.0*'` printed `v2.3.0`, so the empty answer is not a broken instrument. I committed the script and the tasks as an interim change, `1df4bc94986d782a2ade404610720b5fcc7e0512`, and tagged it `v2.3.99` (annotated). `task release:candidate TAG=v2.3.99` exited 0 after about six minutes. The sha256 of its app zip was `77d5dec0f9eec6fa749152289a1c2e88d60625aa32188654ac7f733880ba9900`. The throwaway version gives a different executable UUID, because the build bakes the version into the binary.
- **The candidate N+1 is unchanged.** Tag `v2.4.0`, commit `345faf5421573a6155cd333d3dc4f6560b74392e`. `task release:candidate-verify TAG=v2.4.0` passed before the run and after the clean. The sha256 of each artifact, as in `manifest.json`:
  - `testanyware-app-v2.4.0-aarch64-apple-darwin.zip`: `7c32be63759c539681f5c01d9b3162e45dae51c2010c41d0977fbf5f142890a7`
  - `testanyware-app.rb`: `66fc9f9bec0d45b7a3656ddf6aa05f8bf3674db889fe1a5cf5d25728fe9445cc`
  - `testanyware-v2.4.0-aarch64-apple-darwin.tar.xz`: `0b72e8453affde6353bbd5f3a47142f44dec156adb5d14f46685246b7498a1da`
  - `testanyware-v2.4.0-aarch64-unknown-linux-gnu.tar.xz`: `28a4e07c17339dbc2bb25a775b3e32fa9ec7db25b344c171e49826e116a263be`
  - `testanyware-v2.4.0-x86_64-unknown-linux-gnu.tar.xz`: `d6b417df7cc2782a289dc748b2e0da6548b3f7f022948e9dbd078f52682a6de1`
  - `testanyware.rb`: `05bada1abcb4c2aaf4907b153b212e8184474d39989f88c506dba60cb24c0f2f`
- **This leaf does not void the candidate.** `git diff --stat 345faf54 HEAD -- . ':!.grove'` showed two files before the documents: `Taskfile.yml` and the new `scripts/upgrade-identity-live.sh`. The build scripts (`release-build.sh`, `release_candidate.py`, `build-app.sh`, `notarize-app.sh`, `verify-app.sh`) never read either one, and no archive contains them. The reasoning of `non-macos-runtime-check-k40` for the Taskfile holds for both. The spec and ADR-0020 are documents. No file that the build reads changed after `345faf54`.
- **The run passed in one go, with the owner at the Mac.** The owner agreed to quit the development app (`TestAnyware Dev.app`, which held the socket) and to click the dialogs. The guests of other sessions kept running, and `k26-live` runs on. Machine-checked facts, in order:
  - Release 2.3.99 installed from the tap. The first start answered, build `2.3.99`. `vm start --platform macos --id upgrade-check` reached a healthy agent in 14 s, and `agent snapshot` showed 2 windows.
  - `brew upgrade --cask` took 2 s: `2.3.99 -> 2.4.0`. Homebrew printed `Quitting application 'dev.antony.TestAnyware'` and `Reopening 1 application closed during upgrade`. The new app answered with build `2.4.0` and a new pid.
  - The bundle id, the identifier, the team `TA43A4RUP3` and the designated requirement are equal before and after. The executable UUID changed from `CF04989E-D772-3DEB-82C1-D6E85D2CCB67` to `BC3E960A-BC3A-3178-9667-29BDEDC4334C`. `spctl` says `accepted source=Notarized Developer ID`, and `stapler validate` says `The validate action worked!`.
  - The quarantine attribute was `01c1;6ad9db56;;…` before and `01c1;6ad9db6c;;…` after. The flags carry the user-approved bit `0x40` in both. A negative control of the same test on `0181` (an unapproved download) says not approved.
  - The guest kept its tart process and its keeper, with the same pids and start times. The new app listed it in `adoption.adopted` with `legacy` false, the agent answering and VNC answering. `GET /v1/status` of the new app reports `local_network.state` `allowed` at its first guest connection.
  - The first command after the upgrade, `agent snapshot`, succeeded in 1 s and showed 5 windows. The branch with the denial and its remedy did not occur. So no person allowed the app again after the upgrade, and the "allow again, then succeed" half of the requirement did not run.
- **What the owner saw.** The owner's words: the first-start dialog came "once, before the upgrade", and the owner clicked Open. On the alert: "I couldn't see which version was asking. I approved once." System Settings, Privacy & Security, Local Network shows `TestanywareProbe`, `TestAnyware Dev.app` and `TestAnyware.app`. So the release app has one row. The other two belong to the old probe and the development app. The machine evidence places the one alert before the upgrade. `vm start` through 2.3.99 succeeded. After the upgrade, the adoption probes and the first command succeeded at once. This is an inference, and the owner did not observe which build raised the alert.
- **The result does not differ from the spec.** The requirement "An upgrade keeps the app's identity" holds on one run. The spec and ADR-0020 changed in four places, because three statements said that nobody had measured the release build:
  - The spec, "Packaging and identity", gained the result of the run.
  - The spec, the Taskfile live checks, names `release:upgrade-check` and `release:upgrade-check-clean`.
  - The open risks "The release build" and "Homebrew's approval carry-over" are gone, and "An alert after an upgrade" says that the run was one sample.
  - ADR-0020, Consequences, replaces "nobody measured" with the result.
- **One run gives no rate.** The probe measurements raised the alert once in three upgrades, and this run is one sample. The risk stays open in the spec.
- **A limit of the run.** The owner was at the Mac and clicked within seconds, so the run never stopped at a person's step (exit 3). The stop after a denial, and the second run that resumes from it, are written and read, but no live run exercised them. The state of the Mac decides each phase, which is why the script can resume. `release-publish-k37` does not rely on them.
- **The clean.** `task release:upgrade-check-clean OLD=v2.3.99 TAG=v2.4.0` uninstalled the cask, deleted the tap, the scratch directory and `target/candidates/v2.3.99`, and deleted the local tag `v2.3.99` (it was `adcc806`). Afterwards I inspected the result with commands apart from the script. `git tag -l v2.3.99` prints nothing, and the control `git tag -l v2.3.0` prints the tag. `target/candidates/` holds only `v2.4.0` and `v2.4.0.checks`. `/Applications` has no TestAnyware, `brew list --cask` has no `testanyware-app`, `brew tap` has no `upgrade-check`, and tart has no guest `upgrade-check`. The throwaway reaches no later leaf.
- **State left on the Mac.** The development app is quit. The owner agreed to quit it, and I did not reopen it. The release app has its Local Network row now, because macOS 27.0.1 cannot reset a row. So `release-publish-k37` meets an already granted `dev.antony.TestAnyware`. The acceptance run there does not prove a first grant.
- **Source for the API.** The script depends on two Homebrew 7.0.10 behaviours. The first is the carry-over of the user-approved quarantine bit when the signer is unchanged. The second is the reopen of an app that `uninstall quit:` closed. I read both in the installed source, `/opt/homebrew/Library/Homebrew/cask/upgrade.rb` (`quarantine_release_decision`, `reopen_apps_after_upgrade`) and `cask/quarantine.rb`. The comments of the script cite the upstream path. The live run showed both.
- **Review.** I spent no in-session reviewer. The claims that the compiler cannot check are the facts of the run, and the commands and the status above show them. The one inference, the place of the alert, is marked as an inference.
