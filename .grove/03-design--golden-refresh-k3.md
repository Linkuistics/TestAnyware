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
