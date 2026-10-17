# start-failure-tart-k2

## Goal

Make tart-backed `vm start` (macOS and Linux-on-tart guests) honest: it
succeeds only with a Healthy agent, and otherwise fails with the reason and
leaves no VM. Build the shared seams that the QEMU leaf and the investigation
reuse.

## Context

- Decisions in the retired `plan-k1`: D2 (strict rule), D3 (teardown, and
  `--keep-on-failure`), D6 (the seams), D8 (the Taskfile), D11
  (`VM_LIMIT_REACHED`, exit 6).
- Facts in `plan-k1`'s Notes, 1–7: the start path's polls and their budgets,
  the timing evidence, the zombie-liveness hazard, the contract as it stands,
  and the consumers (none relies on an agent-less success).
- Code: the lifecycle's tart start, `health::wait_for_agent`,
  `TartRunner::start` with its VNC-URL and IP polls, `VmError`, and the
  `vm start` handler and its clap surface.

## Done when

- `wait_for_agent` returns an outcome rather than a `bool`: healthy; the last
  error, classified with its OS errno (refused, no route or host down,
  timeout, HTTP status, decode); or VM exited. Tested at its existing seam.
- The VNC-URL poll and the agent wait both notice a dead `tart run` at once
  and return the current run's log tail.
- A pure start-outcome decision maps what the boot observed to one of:
  - success-with-agent;
  - `VM_BOOT_TIMEOUT` (exit 7; details: stage, polled endpoint, last error,
    attempts, elapsed);
  - `TART_FAILED` (tart's log tail in `details.tart_error`);
  - `VM_LIMIT_REACHED` (exit 6; details: platform, limit, holders);

  each with teardown or keep. It is unit-tested with no tart.
- The tart arm runs through that decision. It never writes an agent-less spec.
  On failure the clone is stopped and deleted and no sidecars remain, unless
  `--keep-on-failure` leaves the VM running with both sidecars.
- The contract text is updated:
  - a CLI design contract amendment: `VM_BOOT_TIMEOUT` covers the agent stage,
    `VM_LIMIT_REACHED` is added, exit 6 is used, and the new flag appears in
    the `vm start` help text with its exit codes;
  - the connection spec's `agent` row;
  - ADR-0014's degraded-agent wording, reworked in place;
  - `CONTEXT.md`'s example dialogue.

  The `cli-contract.rs` gate still passes.
- A root `Taskfile.yml` holds what this leaf ran: the `cli-rs` test, clippy
  and fmt recipes, the live slot-exhaustion repro, the 10-start reliability
  loop k4 will use, and thin wrappers over `scripts/release-*.sh`. Each recipe
  is verified by running it.
- Live, on the macOS 27 host: seam checks (a) and (b) pass, and a normal macOS
  start still succeeds with the resolution switch applied.

## Notes

- Write the contract text for tart as it now behaves, not ahead of the code.
  k3 generalises it to QEMU.
- To fill both slots for check (a), start two macOS VMs, then start a third.
  Every refused attempt must leave no clone (D3), and check (a) proves it.
- If time allows, run the reliability loop once and write what it shows into
  the root brief's Notes for k4. Do not chase finding 1 here.
- This changes a CLI contract that downstream scripts (Koine) depend on, so a
  `review-impl` is a reasonable last act.
