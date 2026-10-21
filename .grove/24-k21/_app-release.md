# app-release-k21 — brief

## Goal

Ship the app. A tagged release publishes the signed, notarised app as a cask in `linkuistics/taps`, with the Host CLI formula. This increment ends the grove.

## Done when

- The release pipeline builds the app bundle for the release channel. It signs, notarises and staples the bundle, and it refuses a bundle id or a team other than `dev.antony.TestAnyware` and `TA43A4RUP3`.
- The cask `testanyware-app` installs `TestAnyware.app` into `/Applications`. It depends on the formula and on Apple silicon. It keeps the quarantine attribute, and its `uninstall quit:` names the bundle id.
- One tag versions the formula and the cask together.
- A release candidate exists before any tag leaves this Mac. It has a manifest with its tag, its commit and the sha256 of each artifact.
- The Linux host harness passes on the candidate binary, as ADR-0009 requires. The release ships no Windows host binary, and no release runs the Windows host harness. The x86_64 Linux archive is build-verified only.
- The live check of an upgrade passes: the app keeps its signing identity and its one Local Network row.
- The herdr acceptance run passes on the release bundle of the candidate.
- The release is published and installs on this Mac. It is the candidate that passed the checks.

## Decomposition

1. `app-release-bundle`: the bundle, the notarisation, the cask, the pipeline and the first release candidate.
2. `release-without-windows-host`: the release stops shipping the Windows host binary.
3. `candidate-rebuild`: the candidate again. The guest-traffic increments and the previous leaf voided the first one.
4. `non-macos-runtime-check`: the Linux harness runs on the new candidate.
5. `upgrade-identity-check`: the live check of an upgrade.
6. `release-publish`: the acceptance run on the candidate, and the publication.

## Candidate identity

The build that passes the checks is the build that ships. A candidate is a build of a local tag, with a manifest of the tag, the commit and the artifact hashes. Local tags publish nothing.

Each later check records the candidate that it tested. `release-publish-k37` publishes only that candidate. It pushes the existing tag and rebuilds nothing.

A change to code, scripts or build inputs after the candidate commit voids the candidate. Rebuild it, and repeat the checks. A change to `.grove/` or to documents does not void it.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, section "Packaging and identity", the open risks, and the requirement "An upgrade keeps the app's identity".
- ADR-0020.
- `scripts/release-build.sh`, `scripts/release-doctor.sh`, `scripts/release-publish.sh` and `scripts/templates/`.
- The tap at `~/Development/homebrew-taps`, with `Casks/koine.rb` as the model.
- Koine's `scripts/notarize-app.sh`, `scripts/signing-env.sh` and `scripts/verify-app.sh`.

## Notes

- Publishing is outward-facing. The leaf `release-publish-k37` asks the owner before it publishes.
- Nobody measured what notarisation does to the grant after an upgrade. The live check settles it.
