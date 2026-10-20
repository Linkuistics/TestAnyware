# app-release-bundle-k22

## Goal

Extend the release pipeline so that it builds a notarised app bundle and renders the cask. Do not publish anything.

## Context

- `docs/specs/testanyware-app.md`, section "Packaging and identity".
- ADR-0020.
- `scripts/release-build.sh`, `scripts/release-doctor.sh`, `scripts/release-publish.sh` and `scripts/templates/testanyware.rb.tmpl`.
- Koine's `scripts/notarize-app.sh`, `scripts/signing-env.sh` and `~/Development/homebrew-taps/Casks/koine.rb`.
- `app-menu-bar-shell-k11`, which has the bundle script for the development channel.

## Done when

- The release build makes the bundle for the release channel, signs it with the hardened runtime, notarises it and staples it. It archives it as a zip that the cask can download.
- A check refuses any bundle id other than `dev.antony.TestAnyware` and any team other than `TA43A4RUP3`.
- A template renders the cask `testanyware-app`. The cask has the `app` stanza and the dependency on the formula and on arm64. It has `uninstall quit:` with the bundle id. Its zap names the app's own support and log directories, and never the VM state.
- The cask keeps the quarantine attribute. It has no postflight that strips it.
- One tag gives one version to the formula and the cask.
- `release-doctor` checks the signing and notarisation prerequisites.
- Taskfile tasks wrap the new steps.
- The publish script pushes the cask into `Casks/` of the tap. It has not run yet.
- A local run of the build on this Mac gives a bundle that passes `codesign --verify`, `spctl --assess` and `stapler validate`.

## Notes

- Notarisation needs the credentials of the owner in the keychain. If they are missing, stop and say what is missing.
