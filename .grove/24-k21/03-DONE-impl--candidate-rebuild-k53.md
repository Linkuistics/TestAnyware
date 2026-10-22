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

## Decisions (running log)

- **The version was still unpublished.** Before the rebuild I asked the remote. `git ls-remote --tags origin 'v2.4.0*'` printed nothing, and `gh release view v2.4.0` said `release not found`. The same commands listed `v1.0.0` and the releases `v2.3.0`, `v2.2.0` and `v2.1.0`, so the empty answer is not a broken instrument. The leaf kept the version.
- **The doctor.** `task release:doctor` passed every check: the signing identity `Developer ID Application: Antony Blakey (TA43A4RUP3)`, the notary profile `testanyware-notary`, the Linux cross-build tools and both ffmpeg sysroots.
- **No change needed a commit.** The working copy was an empty child of `345faf5`, and `git status` was clean. So the commit to build is the last commit of `release-without-windows-host-k44`, not a new one.
- **The tag moved.** The auto-mode classifier denied the deletion of the unpushed tag and the removal of the old directories when I ran them. The owner ran the command: `git tag -d v2.4.0` (it was `0bc1ffb`, the tag object that named `8a7ad3f3`), then `git tag -a v2.4.0 -m "Release v2.4.0" 345faf54`, then `rm -rf target/candidates/v2.4.0 target/candidates/v2.4.0.checks`. I checked afterwards. `v2.4.0` is an annotated tag, and it names `345faf5421573a6155cd333d3dc4f6560b74392e`, which is HEAD. The tree was clean. `target/candidates/` was empty.
- **The result.** `task release:candidate TAG=v2.4.0` exited 0, and `task release:candidate-verify TAG=v2.4.0` passed. Tag `v2.4.0`, commit `345faf5421573a6155cd333d3dc4f6560b74392e`. The sha256 of each artifact:
  - `testanyware-app-v2.4.0-aarch64-apple-darwin.zip`: `7c32be63759c539681f5c01d9b3162e45dae51c2010c41d0977fbf5f142890a7`
  - `testanyware-app.rb`: `66fc9f9bec0d45b7a3656ddf6aa05f8bf3674db889fe1a5cf5d25728fe9445cc`
  - `testanyware-v2.4.0-aarch64-apple-darwin.tar.xz`: `0b72e8453affde6353bbd5f3a47142f44dec156adb5d14f46685246b7498a1da`
  - `testanyware-v2.4.0-aarch64-unknown-linux-gnu.tar.xz`: `28a4e07c17339dbc2bb25a775b3e32fa9ec7db25b344c171e49826e116a263be`
  - `testanyware-v2.4.0-x86_64-unknown-linux-gnu.tar.xz`: `d6b417df7cc2782a289dc748b2e0da6548b3f7f022948e9dbd078f52682a6de1`
  - `testanyware.rb`: `05bada1abcb4c2aaf4907b153b212e8184474d39989f88c506dba60cb24c0f2f`
- **The bundle.** I unzipped the shipped app zip into a new empty directory. `codesign --verify --deep --strict` says `valid on disk` and `satisfies its Designated Requirement`. `spctl --assess` says `accepted`, `source=Notarized Developer ID`, `origin=Developer ID Application: Antony Blakey (TA43A4RUP3)`. `stapler validate` says `The validate action worked!`. `scripts/verify-app.sh` reports `dev.antony.TestAnyware` of team `TA43A4RUP3`, hardened runtime, notarised and stapled.
- **No Windows host archive.** `manifest.json` has six artifacts and none names Windows. `grep -ci windows` finds 0 lines in it. The control, `grep -ci linux` on the same file, finds 2.
- **The earlier candidate is void.** The commit `8a7ad3f3` and every hash from `app-release-bundle-k22` are void. This includes the app zip hash `c9c9c642…62dd`. The guest-traffic increments and `release-without-windows-host-k44` changed code, scripts and build inputs after that commit. The later checks and `release-publish-k37` use only the candidate above.
- **What binds the later leaves.** A change to code, scripts or build inputs after `345faf54` voids this candidate. The commit that retires this leaf changes only `.grove/`, so it does not. The tag is local and unpushed, and this leaf pushed nothing.
- **Review.** I spent no in-session reviewer. The claims that the compiler cannot check are the tag and the hashes, and the commands above show them.
- **Source for the API.** No framework API of a changing version entered this leaf.
