# app-release-bundle-k22

## Goal

Extend the release pipeline so that it prepares a release candidate. A candidate has a notarised app bundle, the cask, and a manifest that fixes the identity of the build. Do not publish anything.

## Context

- `docs/specs/testanyware-app.md`, section "Packaging and identity".
- ADR-0020.
- `scripts/release-build.sh`, `scripts/release-doctor.sh`, `scripts/release-publish.sh` and `scripts/templates/testanyware.rb.tmpl`.
- Koine's `scripts/notarize-app.sh`, `scripts/signing-env.sh` and `~/Development/homebrew-taps/Casks/koine.rb`.
- `app-menu-bar-shell-k11`, which has the bundle script for the development channel.
- The current build requires a clean tree and a tagged HEAD, and it reads the version from the nearest tag. The current publish script reads the version the same way, and `gh release create` makes a missing tag from the default branch.

## Done when

- The release build makes the bundle for the release channel, signs it with the hardened runtime, notarises it and staples it. It archives it as a zip that the cask can download.
- A check refuses any bundle id other than `dev.antony.TestAnyware` and any team other than `TA43A4RUP3`.
- A template renders the cask `testanyware-app`. The cask has the `app` stanza and the dependency on the formula and on arm64. It has `uninstall quit:` with the bundle id. Its zap names the app's own support and log directories, and never the VM state.
- The cask keeps the quarantine attribute. It has no postflight that strips it.
- One tag gives one version to the formula and the cask.
- A candidate is a build of a local, unpushed annotated tag. A Taskfile task makes one candidate from a tag name. A local tag publishes nothing.
- The build and the publish script take the tag as an explicit input. Neither infers it with `git describe`, because two candidate tags can exist at the same time.
- Each candidate keeps its artifacts in a directory of its own, named by the tag. Building a second candidate does not delete the first.
- Each candidate has a manifest with the tag, the commit and the sha256 of each artifact. The publish script refuses to run when the tag names another commit or an artifact differs from the manifest.
- The publish script is ready to publish a recorded candidate. It pushes the existing local tag, then creates the release for that tag with the candidate's artifacts, then pushes the formula and the cask. It never creates a new tag, and it never rebuilds. It has not run yet.
- `release-doctor` checks the signing and notarisation prerequisites.
- Taskfile tasks wrap the new steps.
- A local run of the build on this Mac gives one candidate. Its bundle passes `codesign --verify`, `spctl --assess` and `stapler validate`.

## Notes

- Notarisation needs the credentials of the owner in the keychain. If they are missing, stop and say what is missing.
- The jj checkout may not have a tagged HEAD. State in the leaf how the build reaches a clean tagged commit, and keep the rule that the tree is clean and tagged.
- This candidate is the one that the later checks test and that `release-publish-k37` publishes.

## Decisions (running log)

- **One helper states the candidate rules.** `scripts/release_candidate.py` has four commands: `gate-build`, `write`, `verify` and `render`. The build writes a candidate with it. The publish script and the later leaves call its `verify` on one. Two scripts that each parse the tag would drift apart. Python 3 is already a prerequisite of `release-doctor`, and a unit test can run it in a scratch repository.
- **The gate.** A build needs a local annotated tag in the form `vMAJOR.MINOR.PATCH`. HEAD must be the commit of the tag, and the tree must be clean. `release-build.sh` and `release-publish.sh` take the tag as their one argument. Neither calls `git describe`. The build label of the CLI is the tag.
- **How a jj checkout reaches a clean tagged commit.** jj makes lightweight tags only, so tag the commit with git: `git tag -a vX.Y.Z -m "Release vX.Y.Z" <commit>`. Then make the working copy an empty child of that commit with `jj new <commit>`. In a colocated checkout git's HEAD is the parent of the working-copy commit. I checked this in this checkout: HEAD and `@-` are both `b8fc31bc`. So HEAD is the tagged commit and `git status` is clean.
- **The candidate directory.** It is `target/candidates/<tag>/`. A build deletes only the directory of its own tag, so a second candidate keeps the first. `.gitignore` names `/target/candidates/` and no longer names `/target/dist/`.
- **The manifest.** `manifest.json` records the schema, the tag, the version, the commit and the sha256 of every other file in the directory. That includes the rendered formula and cask, because a changed formula changes what ships. `verify` refuses a tag that names another commit, a missing artifact, an extra file and a hash that differs. The directory must be named by the tag.
- **The build makes the app first.** A rejected notarisation costs minutes, and the cross builds cost much more. `set_cli_version` runs before the app, because `build-app.sh` reads the version from `cli-rs/Cargo.toml` and the app reports it as its build. So the formula, the cask, the CLI and the app carry one version.
- **A finding: bash does not carry `set -e` into `$(...)`.** I proved it with a probe. A first draft ran `build_app_archive` inside `$(...)`. A failed notarisation there would have let the build run on to a candidate. The function now sets `APP_ZIP` and runs as a plain command, and it checks that the zip exists. Test section 6 uses a fake notariser that fails. I watched it fail when I put the `$(...)` and the existence check back.
- **The restore trap moved after the gate.** The old trap ran `git checkout` on `Cargo.toml` even after a refused run. A person's own edits to that file would have been lost. Now the trap exists only after the gate passes.
- **Notarisation.** `scripts/notarize-app.sh <bundle> <zip>` follows Koine's script. It refuses a bundle of another id or team before it uploads. It reads the notary profile `testanyware-notary`, or the profile that `TESTANYWARE_NOTARY_PROFILE` names. It submits and waits, staples the ticket, checks the staple, and only then makes the zip. `ditto` makes the zip, so the ticket is inside it. There is no fallback to an unnotarised zip.
- **The identity check.** `scripts/verify-app.sh` refuses any bundle id other than `dev.antony.TestAnyware` and any team other than `TA43A4RUP3`. It reads both from the `Info.plist` and from the signature. It also checks the hardened runtime, and `--version` checks both version keys. Without `--identity-only` it runs `spctl --assess` and requires `source=Notarized Developer ID`, then `stapler validate`. The tests ran it on real bundles: a development bundle, an ad-hoc signed bundle and a signed release bundle.
- **The cask.** `scripts/templates/testanyware-app.rb.tmpl` has the `app` stanza, `depends_on arch: :arm64`, `depends_on formula: "linkuistics/taps/testanyware"`, `uninstall quit: "dev.antony.TestAnyware"` and no postflight. `brew style` finds no offence in the rendered file once you set aside the Sorbet and frozen-string notices, which only apply inside a tap. It found two real ones, and I fixed them: the stanza order, and a `desc` that named the platform. The cask has no macOS floor, because the spec asks for the formula and arm64 only.
- **The zap.** It names `~/Library/Application Support/TestAnyware` and `~/Library/Logs/TestAnyware`. The first holds the socket. The app writes no log today, and the spec asks for a log directory, so the second is where a later log file belongs. The VM state is under `~/.local/state/testanyware` and `~/.local/share/testanyware`, and the zap never names it. A test checks the zap stanza for that.
- **The publish script.** Order: check the candidate, push the existing tag, `gh release create <tag> --verify-tag`, then one tap commit with the formula and the cask. `--verify-tag` makes gh abort when the tag is absent on the remote, where gh would make one from the default branch. The script never creates a tag and never rebuilds. It refuses a tap clone that is dirty or behind, before any push. It has not run for a real release.
- **A finding in the publish script.** My first remote check used `ls-remote ...^{}`, which lists nothing for a lightweight tag. A remote with such a tag at another commit would have received a push that git rejects. Now the script reads both lines and refuses with a clear message. A test found it.
- **Tests.** `task release:test` runs `scripts/tests/release-pipeline-test.sh`. The script runs 19 unit tests of the helper. It also runs the build refusals and the cask and formula checks. It runs the refusals of `verify-app.sh` on real signed bundles. It runs the publish script against local bare repositories with a fake `gh`. The fake records whether the tag was on the remote when the release started. No test touches Apple, GitHub or the real tap. I made the unit tests fail by removing the gate rules. I made the zap check fail with a zap that names VM state.
- **Review.** I spent no in-session reviewer. The claims that the compiler cannot check are the gate and the manifest rules, and the tests cover each refusal.
- **Source for the API.** The scripts use `notarytool`, `stapler`, `spctl` and `gh release create --verify-tag` as Koine's scripts use them and as the installed `--help` states them. I did not fetch a manual page for any of them. No framework API of a changing version enters this leaf.
- **The local run: candidate `v2.4.0`.** The notary profile `testanyware-notary` is now in the keychain, so `release-doctor` passes every check. I committed the work, tagged that commit with `git tag -a v2.4.0`, and ran `jj new` on it. HEAD and the tag both named the commit, and `git status` was clean. `task release:candidate TAG=v2.4.0` then built `target/candidates/v2.4.0/`. The tag is local, and the remote has no `v2.4.0`. Nothing was published.
- **A finding: the doctor passed while the cross builds failed.** The first run stopped at the Linux cross build with `can't find crate for core`. `/opt/homebrew/bin` comes before `~/.cargo/bin` on `PATH`, and the Homebrew `rustc` 1.99.1 has no cross std targets. The doctor asked `rustup target list` about the default toolchain, which says nothing about the compiler that runs. Now `release-build.sh` puts the toolchain of `cli-rs/rust-toolchain.toml` first on `PATH`. `release-doctor.sh` looks for `lib/rustlib/<triple>` in the sysroot of that compiler. I ran the check against both compilers: the Homebrew one has no std and the rustup one has it. The fix needed a new commit, so I deleted the unpushed tag and tagged the new commit again.
- **The result.** Tag `v2.4.0`, commit `8a7ad3f31fb0f817bc17c9b2051c9f5e38bbc28f`. I unzipped the shipped app zip into a new directory. `codesign --verify --deep --strict` passes, `spctl --assess` says `accepted, source=Notarized Developer ID`, and `stapler validate` works. `verify-app.sh` reports `dev.antony.TestAnyware` of team `TA43A4RUP3`, hardened runtime, notarised and stapled. `release:candidate-verify` passes. The cask and the formula both say 2.4.0, and the cask names the sha256 of the app zip. The sha256 of the app zip is `c9c9c6425cb4586227cab49273f6d9bb731597f3afe78a6b3e32c70db22262dd`. The other hashes are in `manifest.json`.
- **What binds the later leaves.** Any change to code, scripts or build inputs after `8a7ad3f3` voids this candidate. The later checks test it, and `release-publish-k37` publishes it with `scripts/release-publish.sh v2.4.0`.
