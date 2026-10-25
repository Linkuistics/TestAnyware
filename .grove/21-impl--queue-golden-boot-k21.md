# queue-golden-boot-k21

## Goal

A boot of golden creation notices that `tart run` exited, and it tries again
when the framework refuses. A full limit then gives `VM_LIMIT_REACHED` and not a
VNC boot timeout after about 60 s. This works with the app and without it.

## Context

- Spec `docs/specs/admission-queue.md`, the paragraph after the list in "Golden
  creation" that starts "The boot of a setup VM". Read "Problem" for the failure
  of today.
- ADR-0008: golden creation boots its setup VM five times, for the recovery
  cycle.
- Look in `testanyware-vm/src/golden.rs`, `ipsw_base.rs` and `recovery.rs` for the
  boots. The Host CLI reads the line of the limit from `tart run` today.
- This leaf needs no change in the app. It is the smallest delivery of golden
  creation, and `queue-golden-hold-k22` builds on it.

## Done when

- The boot of a setup VM and the boot of a base VM notice that `tart run` exited.
  They no longer poll the log for 60 s after an exit.
- When the framework refuses a boot, golden creation tries it again 3 s later,
  for 30 s at most. The framework can free the slot of the previous boot a moment
  after tart exits.
- After 30 s, golden creation fails with `VM_LIMIT_REACHED` and `refused_by`
  `framework`. It removes its setup VM as it does for any failure.
- Tests use a fake `tart` that refuses a boot as the framework does. One test has a
  refusal that clears within 30 s. Another has a refusal that lasts.
- The golden creation of Linux and Windows guests does not change.

## Notes

- If the seam of the existing golden tests cannot fake `tart run` at its boots,
  say so in the decision log. Add the smallest fake that the root brief names
  for the first seam.
