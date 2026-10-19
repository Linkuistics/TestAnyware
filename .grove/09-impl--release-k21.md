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
