# start-failure-tart-k8

**Integrates:** start-failure-tart-k7

## Goal

Triage the implementation review of `start-failure-tart-k2`, apply the
findings that hold, and verify the resulting increment before the QEMU work
builds on it.

## Context

- Read `start-failure-tart-k7` from its committed review artifact for the
  findings and evidence; this task deliberately does not repeat them.
- The producer is `start-failure-tart-k2`; its committed changes and Decisions
  log establish this increment's contract and accepted limits.

## Done when

- Each review finding has a recorded disposition supported by source or
  verification evidence.
- Accepted findings are addressed, with appropriate verification through the
  repository Taskfile and any necessary live checks.

## Notes

The review was inspection-only and performed no test/build/lint/live runs.
Keep triage independent: a finding may be accepted, rejected with evidence,
or identified as a documented trade-off.

## Decisions (running log)

Triage of `start-failure-tart-k7`, each finding checked against the source at
`c7c3fe3`/`66c5b29` (the review was inspection-only; none of it was run).

**T1 — F1 accepted (real issue).** `settle_failed_tart_start` discards
`TartRunner::stop`'s result and removes both sidecars regardless, so a failed
`tart delete` strands a clone that `vm stop` can no longer find (it needs the
meta, `lifecycle.rs` `stop`). Fix: settlement checks whether the clone still
exists after teardown; if so it keeps (or writes) the meta, leaves the error's
`kept` flag true so the caller is told to `vm stop <id>`, and warns on stderr.
Accepted limit: the leftover is reported through the existing `kept` field and a
stderr warning, not a new error code or details field (a new code would change
the exit-code contract for a rare path).

**T2 — F2 accepted (real issue).** On the keep path a failed meta write left a
running VM that `vm stop` cannot reclaim while the error still said
`kept: true`. Fix: the keep path writes the meta first; if that fails the VM is
torn down instead and the error reports `kept: false`.

**T3 — F3 accepted (real issue).** `check_keep_on_failure` and
`check_slot_exhaustion` start VMs under ids that are not in `started_ids` until
after the assertions, so an unexpected success or an early abort leaks them.
Fix: register every id before the start that may create it, and make `cleanup`
fall back to `tart stop/delete` when `vm stop` cannot find the meta.

**T4 — F4 accepted (real issue).** `TartRunner::launch` reclaims a same-id VM
but leaves its spec/meta, so a launch that fails before `settle_failed_tart_start`
(`tart clone`, `tart set --display bogus`, spawn) leaves sidecars describing a
deleted VM. Fix: `launch` clears the id's sidecars as soon as it begins, since
a start in progress invalidates whatever they described.

**T5 — Verification.** `task cli:test` passes (`testanyware-vm`: 191 tests, 7
new in `lifecycle::tests::settle`, covering F1, F2 and the sidecar clearing; a
mutation that ignores whether the clone survived fails the F1 test, so it is
not vacuous). `task scripts:check` passes. Live on the macOS 27 host: a kept VM
with both sidecars, restarted with the same id and `--base k8-no-such-golden`
(fails at `tart clone`, after the old VM is reclaimed), left no clone and no
sidecars (F4). One correction to the review's example: tart *accepts*
`--display bogus`, so that restart ran a full start and tore down by default
(also clean); the early-failure path needs a failing `tart clone`. F1's failed
`tart delete` and F2's failed meta write are not reproducible live without
breaking tart or the state volume, so they are covered by the injected-seam unit
tests only. F3 is covered by `shellcheck`/`shfmt` and reading; the live script's
failure paths were not forced.
