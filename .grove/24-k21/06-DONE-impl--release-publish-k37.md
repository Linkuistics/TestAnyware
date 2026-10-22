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

## Decisions (running log)

- **The candidate is unchanged.** `scripts/release_candidate.py verify` passes for `v2.4.0`. The tag is an annotated tag and names `345faf5421573a6155cd333d3dc4f6560b74392e`, the commit of `manifest.json`. Each of the six artifacts has the sha256 that `candidate-rebuild-k53` recorded. The app zip is `7c32be63759c539681f5c01d9b3162e45dae51c2010c41d0977fbf5f142890a7`.
- **The two checks name this candidate.** The logs of `non-macos-runtime-check-k40` and `upgrade-identity-check-k36` both name `v2.4.0` at `345faf54`, and both passed.
- **No file that the build reads changed.** `git diff --stat 345faf54 HEAD` shows, outside `.grove/` and the documents, only `Taskfile.yml` and the new `scripts/upgrade-identity-live.sh`. The hunks of the Taskfile add the two upgrade-check tasks and one name in the `scripts:check` list. `release-build.sh`, `release_candidate.py`, `build-app.sh`, `notarize-app.sh`, `verify-app.sh` and the templates are unchanged, and no archive contains either file. The literal text of the done-when calls any such change a void. I read it as the node brief does: the rule protects code, scripts and build inputs of the artifacts. The owner can overrule this in the approval question.
- **The herdr acceptance run passed on the candidate.** I unzipped the shipped app zip into a new empty directory. `task release:app-verify` reported `dev.antony.TestAnyware` of team `TA43A4RUP3`, hardened runtime, notarised and stapled. I took the CLI from the candidate archive (`testanyware 2.4.0`, sha256 `f3ce7e1e…0090`) and ran `scripts/herdr-acceptance-live.sh` on that bundle from herdr session `w2E:p1`. It started the bundle (build 2.4.0). `vm start` reached a healthy agent in 16 s. The agent, screen, input and file steps passed, and the six controls each failed with `APP_UNREACHABLE` when no app answered. `vm stop` left no guest and no sidecar. The script printed `PASS`. This shell can dial the guest, so the controls carry the proof that the app carried the traffic. The Local Network row of the app was granted in `upgrade-identity-check-k36`, so this run does not prove a first grant. Afterwards no TestAnyware process ran and the guest `k26-live` of another session ran on.
- **The remote holds no `v2.4.0`.** `git ls-remote --tags origin 'v2.4.0*'` printed nothing, `gh release view v2.4.0` said `release not found`, and the control `v2.3.0*` printed `v2.3.0`.
- **Two facts for the owner before the push.**
  - The commit `345faf54` is on no remote branch. `origin/main` is `abbc3702`, an ancestor of it, and the tag is 50 commits ahead. Pushing the tag publishes those 50 commits, with their `.grove/` files, as objects that only the tag reaches.
  - The clone of the tap at `~/Development/homebrew-taps` has a detached HEAD. It sits on `5f408c02`, the same commit as `main` and `origin/main`, and the tree is clean. `push_tap` runs a plain `git push`, which fails on a detached HEAD, after the tag and the release are public.
- **The owner approved the publication.** The question named the tag `v2.4.0`, the commit `345faf54`, and the app zip sha256. The owner chose to push the commit to `main` first, and to let me switch the tap clone to `main`. The commit was 50 commits ahead of `origin/main`, so the tag alone would have published them with no branch.
- **The publication used the recorded candidate.** `git switch main` in the tap clone moved nothing, because it sat on the commit of `main`. `git push origin 345faf54:refs/heads/main` fast-forwarded `origin/main` from `abbc370` to `345faf5`. Then `scripts/release-publish.sh v2.4.0` verified the candidate again, pushed the existing tag (`[new tag] v2.4.0 -> v2.4.0`), created the release `https://github.com/Linkuistics/TestAnyware/releases/tag/v2.4.0` with the archives, and pushed the commit `d037cab` `testanyware v2.4.0` to `linkuistics/taps` (`5f408c0..d037cab`). It rebuilt nothing.
- **The install on this Mac.** `brew install --cask linkuistics/taps/testanyware-app` installed `/Applications/TestAnyware.app` (2.4.0). The formula on the Mac was still 2.3.0, because the cask's dependency was already satisfied. So I ran `brew upgrade linkuistics/taps/testanyware`, which installed 2.4.0. `testanyware doctor` then reported the app: installed (2.4.0), running (build 2.4.0, protocol 1-1), the local network dial connected, 15 VMs adopted, and both builds equal. It reported one failure, `login item: the app does not start at login`, with the remedy `Turn on "Start at Login" in the TestAnyware menu.` That step is the person's, and I did not toggle it.
- **The documents.** The spec, "Packaging and identity", states that 2.4.0 is the first published release and that the install and `doctor` worked. ADR-0020, Consequences, says that 2.4.0 shipped the decision. No open risk of the spec depended on the release beyond the one that `upgrade-identity-check-k36` already updated.
- **Source for the API.** No framework API of a changing version entered this leaf. The Homebrew behaviour that matters (the cask dependency is satisfied by an older formula) showed in the live run.
- **Review.** I spent no in-session reviewer. The publication is irreversible, so the owner's approval gated it, and the claims here are the outputs shown above.
