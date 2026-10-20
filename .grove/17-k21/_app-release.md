# app-release-k21 — brief

## Goal

Ship the app. A tagged release publishes the signed, notarised app as a cask in `linkuistics/taps`, with the Host CLI formula. This increment ends the grove.

## Done when

- The release pipeline builds the app bundle for the release channel. It signs, notarises and staples the bundle, and it refuses a bundle id or a team other than `dev.antony.TestAnyware` and `TA43A4RUP3`.
- The cask `testanyware-app` installs `TestAnyware.app` into `/Applications`. It depends on the formula and on Apple silicon. It keeps the quarantine attribute, and its `uninstall quit:` names the bundle id.
- One tag versions the formula and the cask together.
- The live check of an upgrade passes: the app keeps its signing identity and its one Local Network row.
- The herdr acceptance run passes on the release bundle.
- The release is published and installs on this Mac.

## Decomposition

1. `app-release-bundle`: the bundle, the notarisation, the cask and the pipeline.
2. `upgrade-identity-check`: the live check of an upgrade.
3. `release-publish`: the acceptance run on the release build, and the publication.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, section "Packaging and identity", the open risks, and the requirement "An upgrade keeps the app's identity".
- ADR-0020.
- `scripts/release-build.sh`, `scripts/release-doctor.sh`, `scripts/release-publish.sh` and `scripts/templates/`.
- The tap at `~/Development/homebrew-taps`, with `Casks/koine.rb` as the model.
- Koine's `scripts/notarize-app.sh`, `scripts/signing-env.sh` and `scripts/verify-app.sh`.

## Notes

- Publishing is outward-facing. The leaf `release-publish-k37` asks the owner before it publishes.
- Nobody measured what notarisation does to the grant after an upgrade. The live check settles it.
