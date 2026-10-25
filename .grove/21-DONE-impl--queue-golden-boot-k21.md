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

## Decisions (running log)

**One function serves every macOS boot of a golden creation.** `golden_boot::boot_to_vnc`
starts `tart run` (normal or `--recovery`), waits for the VNC line with the liveness-aware
`await_vnc_url` that `vm start` uses, and reads the log when `tart run` exits. The boots that
use it are the base VM (`ipsw_base`), the setup VM (`golden`), the Recovery boot and the
normal reboot after it (`recovery`). The cycle has two Recovery boots and two reboots, so
this is every boot of ADR-0008. On a failure it leaves no `tart run` of its own running, and
the caller removes the VM as before.

**The pause and the window are the constants of `vm start`.** The retry reuses
`next_slot_step` and `SLOT_RETRY_INTERVAL` (3 s), with a window of 30 s counted from the
first refusal. The final try is made at the end of the window. The 60 s wait for the VNC
line stays, but the poll is 5 per second, so a dead `tart run` is seen within 0.2 s.

**An exit that is no refusal is `TART_FAILED` at once**, with the log tail, as in `vm start`.
It is not tried again, because only the refusal of the framework is known to clear.

**Linux and Windows do not change.** `reboot_and_wait_ssh` is shared with the Linux golden,
so it keeps its code. The macOS cycle calls a new `reboot_macos_and_wait_ssh`, and both call
the same `wait_ssh_after_boot`. `golden_linux` still polls the log, and the Windows golden has
no tart.

**The seam is the argument `&ToolSearch`.** The golden functions had no fake `tart` before:
their tests are live ones that take 10 to 40 minutes. `provision_boot1`, `recovery_boot_csrutil`
and `configure_fresh_vm` pass `ToolSearch::from_process_path()` and are not tested with the
fake, so their wiring is checked by the compiler and by the live tests of a later leaf
(`queue-live-checks-k25`). `boot_to_vnc` takes the tools and a `BootBudget`, so
`tests/golden_boot.rs` drives it against the existing fake `tart` with the `capacity` mode. I
added only a `run-fails` mode to the fake, for the exit that is no refusal.

**Checks that a test can fail.** With the liveness check forced to true, the exit test took
60 s and failed its 5 s bound. With the retry replaced by an immediate failure, the three
tests of a refusal that clears, a refusal that lasts and a Recovery refusal failed. Both
mutations were reverted.

**The text of the failure.** `VM_LIMIT_REACHED` from a golden creation carries the holders and
the line of tart, and `refused_by` `framework`. `waited_secs` stays 0, because no queue was
involved. The contract already lists `vm create-golden` among the commands that return the
code, so only `error-codes.md` and `golden-images.md` gained a sentence.

**No review leaf, and the in-session allowance stays unspent.** The logic is a loop over
pieces that already exist, and both mutations of the loop failed a test.
