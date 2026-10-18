# macos-27-golden-k13

## Goal

Rebuild the macOS golden as macOS 27, because the human has found significant
changes in 27 that warrant it. The result is a macOS 27 golden that reaches a
Healthy agent and is the default macOS golden.

## Context

- Raised by the human in the `finish-k12` session (2026-10-18), after this
  grove's own goal was met. It is not part of #6: the Windows TPM and password
  faults do not touch macOS. It sits ahead of the finish sentinel so the grove
  is not torn down before it is decided.
- The current macOS golden is `testanyware-golden-macos-tahoe` (a Tart VM,
  macOS 26, about 29 GB on disk, stopped). The Linux golden is not in scope.
- The host is macOS 27.0.1. The human has not yet said what in 27 matters;
  ask, because it decides what the golden must be checked for.
- `vm create-golden` is the way to build (memory: golden creation lives in the
  CLI, the external scripts are retired). Read how the macOS path picks its
  image and names its result before building. Whether a macOS 27 IPSW or Tart
  base image is available is a fact to find, not assume.
- Guests are driven only through the In-VM agent and VNC. Keep the image
  minimal: no test-specific tooling.
- Known macOS-over-VNC detail: Command = XK_Alt_L (0xffe9), Option = XK_Meta_L
  (0xffe7). Check it still holds on 27, since the keymap is what VNC input
  depends on.
- A build is long; the old Windows golden took about an hour.

## Done when

- The human has answered the swap question, as for the Windows golden: build
  under a new name and keep the Tahoe golden until the new one is verified, or
  replace it under the default name. Deleting or replacing the existing golden
  needs an explicit yes; never do it unasked.
- A macOS 27 golden is built with `vm create-golden --platform macos`, and a
  clone of it starts and reaches a Healthy agent, with no help over VNC.
- Whatever the human found that changes in 27 is checked in the new guest, and
  anything that breaks in the CLI or agent is fixed or externalised.
- If the default golden name or the docs name macOS 26 or "tahoe", they are
  updated to match what was decided. A live Taskfile recipe is added only if
  the work needs one.
- No clone is left running (`vm list`).

## Notes

- If no human is reachable, stop and leave this leaf live. Do not build or
  swap on an assumed answer.
