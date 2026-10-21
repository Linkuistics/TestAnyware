# release-publish-k37

## Goal

Run the herdr acceptance run on the release candidate, and then publish that same candidate.

## Context

- The done-when of the root brief.
- `herdr-acceptance-run-k29`, `non-macos-runtime-check-k40`, `upgrade-identity-check-k36` and `app-release-bundle-k22`.
- `scripts/release-publish.sh` and the tap at `~/Development/homebrew-taps`.
- The "Candidate identity" rule of the node brief.

## Done when

- The candidate manifest, the artifacts and the local tag agree: the tag names the commit of the manifest, and each artifact has the recorded sha256.
- The decision logs of `non-macos-runtime-check-k40` and `upgrade-identity-check-k36` name this same candidate, and both checks passed.
- No file outside `.grove/` and the documents changed since the commit of the candidate. If one did, the candidate is void. Rebuild it and repeat the checks before you ask the owner.
- The herdr acceptance run passes from a herdr session on the bundle of this candidate.
- The owner approves the publication. The question names the tag, the commit and the sha256 of the artifacts. Ask before the push of the tag, the GitHub release and the push to the tap.
- The publication uses the recorded candidate. It pushes the existing local tag, and creates the release for that tag with the recorded artifacts. It pushes the formula and the cask of that build, and it rebuilds nothing after the approval.
- A tagged release exists. The formula and the cask are in `linkuistics/taps`.
- `brew install --cask linkuistics/taps/testanyware-app` installs the app on this Mac, and `testanyware doctor` reports it.
- The open risks of the spec that the release settled are removed or updated, and the ADR set is current.

## Notes

- Publishing cannot be undone. Do not publish when one of the checks above fails.
- Commits after the candidate that change only `.grove/` and documents do not change the candidate. The tag stays on the commit that was built and tested.
