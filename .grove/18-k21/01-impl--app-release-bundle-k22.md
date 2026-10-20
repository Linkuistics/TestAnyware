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
