# macos-golden-refresh-run-k13

## Goal

Refresh the macOS 27 golden of this Mac to macOS 27.2.1 with the code of
`macos-golden-refresh-k7`, and pass the live criteria that k7 handed over.
Delete the base and the IPSW that ADR-0027 discards.

## Context

- `macos-golden-refresh-run-k11` owned this goal first. Its second refresh
  passed the fast checks and the install, and then the OCR of the host hung
  at Setup Assistant. `vision-ocr-hang-k12` fixes that hang first. The
  failed builds kept the earlier golden and the older base, as the design
  requires.
- k11 also changed the Setup Assistant driver. It opens Terminal from the
  Utilities folder of Finder, not through Spotlight. Live run 4 of k11
  passed that step.
- The cache holds `macos-27.2.1-26B131.ipsw` (26 GB) and
  `macos-27.0.1-26A434.ipsw` (27 GB). The refresh needs no download.
- k11 deleted `testanyware-golden-macos-tahoe`.

## Done when

- `task vm:refresh-golden-macos` rebuilds `testanyware-golden-macos-27`.
  Afterwards `testanyware-base-macos-27.2.1-26B131` exists, and
  `testanyware-base-macos-27` and `macos-27.0.1-26A434.ipsw` are gone.
- A clone of the new golden reports `27.2.1` from
  `sw_vers -productVersion`. The live gate in `live-vm-gate.rs` passes on
  that golden with `TESTANYWARE_LIVE_VM=1`. This is the golden-build seam
  of the root brief.
- After the refresh, `vm create-golden --platform macos --dry-run` says
  that the base exists, and it names no older base.

## Notes

- With the IPSW cached, the refresh takes about 25 minutes. The install takes
  about 4, Setup Assistant about 7 and golden creation about 15.
- Golden creation connects to the guest directly (ADR-0019), so it needs
  Local Network access. In k11, a binary from the agent session reached the
  guest once and was refused once. If SSH fails with `No route to host`,
  compare with `/usr/bin/nc`, and ask the owner to run the refresh from
  Terminal.
- Golden creation asks the TestAnyware app for a slot. Keep both macOS
  slots free, and allow the Local Network alert if it appears.
- Keep the build logs in scratch, not in the repository.
