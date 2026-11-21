# macos-golden-refresh-run-k11

## Goal

Refresh the macOS 27 golden of this Mac to macOS 27.2.1 with the code of
`macos-golden-refresh-k7`, and pass the live criteria that k7 handed over.
Delete the base, the IPSW and the goldens that ADR-0027 discards.

## Context

- `macos-golden-refresh-k7` changed `vm create-golden --platform macos` to
  the rule of ADR-0027, and it added `task vm:refresh-golden-macos`. Its
  running log holds the decisions and the measurements.
- On 2026-11-22, before any refresh, the dry run on this Mac named macOS
  27.2.1 (26B131), the missing base `testanyware-base-macos-27.2.1-26B131`,
  the older base `testanyware-base-macos-27` and the existing golden. That
  is the golden-plan seam of the root brief, and it passed.
- The first refresh stopped at Setup Assistant with a ZRLE fault.
  `setup-assistant-zrle-fault-k10` fixes it first. The failed build kept
  the earlier golden and the older base, as the design requires.
- The cache holds `macos-27.2.1-26B131.ipsw` (26 GB) and
  `macos-27.0.1-26A434.ipsw` (27 GB). The refresh needs no download.
- The auto-mode permission layer of the k7 session refused
  `testanyware vm delete testanyware-golden-macos-tahoe`. Ask the owner to
  allow that deletion, or to run it.

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
- `testanyware-golden-macos-tahoe` is deleted.

## Notes

- With the IPSW cached, the refresh takes about 25 minutes. The install takes
  about 4, Setup Assistant about 5 and golden creation about 15.
- Golden creation asks the TestAnyware app for a slot. Keep both macOS
  slots free, and allow the Local Network alert if it appears.
- Keep the build logs in scratch, not in the repository.
