# golden-refresh-k3

## Goal

Design how each golden stays at the latest release of its major, and record
that rule. For macOS, `vm create-golden` must name the point release that it
would install, and it must refresh a base VM that is older. Then insert the
implementation leaves that update the macOS 27 golden to macOS 27.2.1, ahead
of the release leaf.

## Context

- The owner set the rule on 2026-11-21: keep each golden at the latest
  release of its major. The owner also wants the macOS golden at 27.2.1 now.
- Today the base VM `testanyware-base-macos-27` comes from Apple's IPSW once,
  and later runs reuse it. The dry run names no point release.
- The golden-name guard refuses to replace a golden that exists. Under the
  rule in the root brief that keeps no backward compatibility, the old base
  VM and golden are discarded and rebuilt.
- This Mac also holds `testanyware-golden-linux-24.04` and
  `testanyware-golden-macos-tahoe`. Since 2.7.0, golden creation refuses a
  macOS major below 27.

## Done when

- The rule is recorded where a future session finds it. If the test in
  `ADR-FORMAT.md` passes, the record is an ADR.
- The design states how the macOS pipeline finds and installs the latest
  point release. It also states what the rule means for the other goldens.
- Implementation leaves exist ahead of the release leaf. Each one names the
  seams from the root brief that it must pass.

## Notes

- A macOS base build downloads about 25 GB and drives Setup Assistant. Plan
  its time and its disk space.
- This design does not depend on `keeper-removal-k2`.

## Decisions (running log)

- 2026-11-21, live state. The host ran macOS 27.2.1 (26B131) with tart
  2.42.0 and TestAnyware 2.9.0. `testanyware-base-macos-27` came from the
  cached IPSW `macos-27.0.1-26A434.ipsw`, and its disk last changed on
  2026-10-18. `testanyware-golden-macos-27` changed on 2026-10-19. The disk
  of `testanyware-golden-linux-24.04` last changed on 2026-06-08 and is still
  20 GB, so that golden predates the disk growth. The Windows goldens are
  QEMU files from 2026-10-18. The volume had 1.6 TiB free.
- 2026-11-21, guard located. The Context of this task says that a
  golden-name guard refuses to replace a golden. That guard is
  `task vm:golden-name-free`, and only the two contributor recipes run it.
  `vm create-golden` itself deletes a golden of the same name before boot 1.
- 2026-11-21, catalogue. Apple's IPSW catalogue listed three builds of
  macOS 27.2.1: 26B131, 26B7131 and 26B8132. Only 26B131 lists the device
  `VirtualMac2,1`, which is the hardware model of a virtual Mac. 26B7131
  serves Mac18,1 and Mac18,2, and 26B8132 serves Mac17,10 to Mac17,13. The
  selector selects by version alone, so all three builds tie.
- 2026-11-21, selector measured. A scratch crate ran the real
  `parse_ipsw_catalogue` and `select_ipsw` of this checkout on the saved
  catalogue. `select_ipsw(27)` returned 27.2.1 (26B8132). Rust's
  `max_by_key` returns the last of equal maxima, and 26B8132 is last in the
  catalogue. So a base build today would download an image that the
  catalogue does not list for a virtual Mac. The scratch crate and the
  catalogue copy stayed in scratch.
- 2026-11-21, selection. The macOS release is the newest entry that the
  catalogue lists under `VirtualMac2,1` for the major. The read walks the
  property list, because a file name does not tell devices apart. A
  catalogue with no such entry fails golden creation, and it never falls
  back to another build. This session rejected Virtualization.framework's
  latest supported image: it answers for the host, not for a major, and a test
  cannot give it a fixed catalogue.
- 2026-11-21, base name. The base VM name carries its release:
  `testanyware-base-macos-<version>-<build>`. `tart list` then tells the
  release with no boot and no second record. A base of the major with
  another name is older, and that includes `testanyware-base-macos-27`.
  Golden creation deletes each older base, and each cached IPSW of another
  release, after the base of the latest release is ready.
- 2026-11-21, dry run. `vm create-golden --dry-run` reads the catalogue
  and `tart list`. It names the release and its build, says whether the
  base exists, and names each older base that the run deletes. It also says
  when a golden of the same name exists, because the run replaces it.
  Without the network, the dry run fails with the error of the real run.
- 2026-11-21, golden replacement. A build deletes the earlier golden only
  just before its final clone. A base refresh adds a 25 GB download and a
  Setup Assistant run, and a failed refresh must not leave `vm start` with
  no golden.
- 2026-11-21, no run-time check. Nothing checks at `vm start` or in
  `doctor` whether a golden is older, for the reason in ADR-0018. A person
  runs `vm create-golden` again after a new release. For macOS, the dry run
  tells whether that run builds a new base.
- 2026-11-21, other goldens. The Linux build pulls the Cirrus Labs tag
  again, and it installs every pending update, including the ones that need
  a new package. A failed update fails the build. The Windows golden holds
  the release of the ISO that a person supplies. TestAnyware cannot
  download that ISO and runs no Windows Update, so this grove changes no
  Windows code.
- 2026-11-21, tahoe golden. Golden creation refuses a macOS major below 27,
  so `testanyware-golden-macos-tahoe` cannot follow the rule. The owner's
  rule to discard old goldens covers it. A sweep of `~/Development` found
  no code that starts a guest from it. UIAnyware's `apps/demo/verify.sh`
  names it in a stale comment and starts the default macOS golden. The
  other hits are measurement notes. The macOS leaf deletes it.
- 2026-11-21, record. The rule passes the three-part test of
  `ADR-FORMAT.md`. Consumers depend on the major-only names, so the rule is
  hard to reverse. A reader who expects a fixed image finds a refresh
  surprising. Point-release pinning and an in-place update were real
  alternatives. The record is ADR-0027, numbered as this repository's ADRs
  are. The glossary entries "Golden image" and "Base VM" change with it.
- 2026-11-21, review. This session cut no `review-design` leaf. The doubts
  in this design are the catalogue read, the base name and the refresh. The
  live base build, the guest version and the dry run of
  `macos-golden-refresh-k7` test each of them on the real catalogue. No
  doubt remains that needs an independent read before that leaf runs.
- 2026-11-21, leaves. `macos-golden-refresh-k7` changes the macOS code and
  refreshes the macOS golden. `linux-golden-refresh-k8` changes the Linux
  build and rebuilds the Linux golden. Each is a vertical slice that a
  session can check alone. They are flat siblings ahead of the release
  leaf, because they share the root's context. The Linux leaf serves the
  root goal "each golden", although the root's Done-when names only macOS.
  Its seams are this design's proposal. The release leaf gained a line for
  the golden rule in its release notes. No leaf changes Windows code.
- 2026-11-21, sweep control. The pattern `--base[ =]\S*golden\S*` found
  `--base "$golden"` in this repository, and a mutated pattern found
  nothing. The same pattern for `tahoe` found nothing in `~/Development`.
  APIAnyware's run reports name the tahoe golden as the golden of
  `vm start --platform macos` at that time. That command now clones
  `testanyware-golden-macos-27`.
