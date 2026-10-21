# candidate-rebuild-k53

## Goal

Build the release candidate again. The increments of `app-all-guest-traffic-k42` and `release-without-windows-host-k44` changed code, scripts and build inputs after the commit of the candidate `v2.4.0`, so they voided it. The checks `non-macos-runtime-check-k40`, `upgrade-identity-check-k36` and `release-publish-k37` run on the new candidate.

## Context

- The node brief, "Candidate identity". A change to code, scripts or build inputs after the candidate commit voids the candidate. A change to `.grove/` or to documents does not.
- The candidate `v2.4.0` was never published. Its tag is local and unpushed, at commit `8a7ad3f31fb0f817bc17c9b2051c9f5e38bbc28f`. `app-release-bundle-k22` already removed and re-made the unpushed tag once, so this leaf does the same and keeps the version.
- The voiding leaves: `harness-guest-streams-k47` and `remaining-direct-dials-k48` at the root, and `release-without-windows-host-k44` in this node.
- The tasks: `task release:doctor`, `task release:candidate TAG=v2.4.0` and `task release:candidate-verify TAG=v2.4.0`. The build needs a clean tree with HEAD on the tagged commit. In a jj checkout, the description of `release:candidate` says how to tag.

## Done when

- Before the rebuild, the leaf checks that the remote has no tag `v2.4.0` and that no release `v2.4.0` exists. If either exists, it stops and asks the owner, because the version is then published.
- The leaf runs `task release:doctor`, and every check passes.
- The leaf commits any uncommitted change that is not under `.grove/`, moves the local annotated tag `v2.4.0` to the commit that it will build, and builds with `task release:candidate TAG=v2.4.0`. The old directory `target/candidates/v2.4.0/` and `target/candidates/v2.4.0.checks/` are removed first, so that no old artifact stays beside the new ones.
- `task release:candidate-verify TAG=v2.4.0` passes. The bundle of the candidate passes `codesign --verify`, `spctl --assess` and `stapler validate`. The manifest names no Windows host archive.
- The decision log records the tag, the new commit, and the sha256 of each artifact. It states that the earlier commit `8a7ad3f3` and its hashes are void.
- No file outside `.grove/` and the documents changes after the tagged commit. If one does, the candidate is void again.

## Notes

- Publishing is outward-facing and stays with `release-publish-k37`. This leaf pushes nothing.
- This leaf runs after every leaf that changes code, scripts or build inputs. A leaf cut later that changes them voids this candidate, and a new rebuild leaf follows it.
- The notarisation needs the network and the notary profile `testanyware-notary` in the keychain.
