# release-k21

## Goal

Release the work this grove delivered: the Windows TPM and golden-password fixes, the macOS 27 golden image and the tart VNC keeper.

## Done when

- Read the release procedure in `scripts/release-doctor.sh`, `scripts/release-build.sh` and `scripts/release-publish.sh`.
- Run `scripts/release-doctor.sh` and resolve every failure.
- Choose the version number. If the choice is not obvious, ask the human.
- Before `scripts/release-publish.sh` runs, ask the human to confirm. Publishing is outward-facing.
- Run the build and publish scripts from this arm64 Mac. Do not add GitHub Actions CI.
- Report the published version and its URL.

## Decisions (running log)

- Version v2.2.0, chosen by the human. The work adds features and fixes and keeps the CLI contract.
- Landing: the human chose to move `main` to the tip (d1a8ed1), tag `v2.2.0` there and push both. The build needs a clean tagged HEAD, so the tag went on before the build and before this leaf's retirement commit.
- `release-doctor.sh` passed, yet the first build failed on the Linux cross target. Homebrew's `cargo` (`/opt/homebrew/bin`) came before rustup's on PATH and has no cross-target std. The second build ran with `~/.cargo/bin` first and passed. The doctor checks rustup's targets but not which `cargo` runs. This is a gap in the doctor, not fixed here.
- The tap clone `~/Development/homebrew-taps` sat on a detached HEAD, so `git push` in `release-publish.sh` would fail. `git checkout main` fixed it before publish.
- The human confirmed publish. Published: https://github.com/Linkuistics/TestAnyware/releases/tag/v2.2.0 with five archives. Tap commit c212b84 on `homebrew-taps` main.
