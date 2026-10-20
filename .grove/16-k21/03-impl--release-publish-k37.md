# release-publish-k37

## Goal

Run the herdr acceptance run on the release build, and then publish the release.

## Context

- The done-when of the root brief.
- `herdr-acceptance-run-k29`, `upgrade-identity-check-k36` and `app-release-bundle-k22`.
- `scripts/release-publish.sh` and the tap at `~/Development/homebrew-taps`.

## Done when

- The herdr acceptance run passes from a herdr session on the release bundle.
- The upgrade check of `upgrade-identity-check-k36` passes.
- The owner approves the publication. Ask before the tag, the GitHub release and the push to the tap.
- A tagged release exists. The formula and the cask are in `linkuistics/taps`.
- `brew install --cask linkuistics/taps/testanyware-app` installs the app on this Mac, and `testanyware doctor` reports it.
- The open risks of the spec that the release settled are removed or updated, and the ADR set is current.

## Notes

- Publishing cannot be undone. Do not publish when one of the checks above fails.
