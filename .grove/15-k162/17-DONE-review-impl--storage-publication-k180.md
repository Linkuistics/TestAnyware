# storage-publication-k180

**Reviews:** storage-publication-k168

## Goal

Review the publication slice adversarially before node k162 closes.

## Context

Read the producer commit for `storage-publication-k168`. The changed code is
in `experiments/exec-reliability/controller.py`: `Plan.publication_window`,
`_validate()`, `_RunStorage.begin_publication()`, `subjects_after()`,
`_bounded_turn()`, `close()`, `finish()`, `_publication_outcome()`,
`outcome()`, `RunOutcome`, `execute_outcome()` and `execute()`. The client
changes are `kill()`, `expect_exit()` and `_reap_due()` in
`experiments/exec-reliability/storage_client.py`. The changed tests are
`test_storage_controls.py` and `test_storage_client.py`.

The spec is `docs/specs/diagnostic-evidence-storage.md`. Its sections "Phases
and bounds" (P3), "Terminal outcome", "Timing conditions" and "Test seams and
required controls" give the contract. The producer leaf states the done
conditions and records each decision.

## Done when

- Check that publication starts after the last teardown action and the
  settled worker close. Check that P is S plus W, and that W is at most
  10 seconds.
- Check that the subjects-after cutoff is S plus W/2, and that a late digest
  is `unavailable:storage`.
- Check that the controller sends one close frame and drains the outbox
  until P without a block.
- Check that an unfinished writer receives KILL at P. Check that each path
  sets the reap deadline to P plus 2 seconds. Check that a residual never
  blocks the controller.
- Check that each publication value matches the spec's "Terminal outcome"
  definition. Look for a path that reports `complete` without an `ok` close
  acknowledgement before P.
- Check that the verdict requires `complete`, and that `execute()` still
  returns the run directory as a `Path`.
- Check that the audit hook control covers file system and import events.
  Its window must run from readiness to the end of publication.
- Check that each control in `test_storage_controls.py` passes without a
  marker. Check that each review control fails on the build that lacks its
  rule.
- Check that the foundation report subsection "Delivered behaviour (k162)"
  matches the delivered code and states its limits.
- Report each finding with its file, line and a failure scenario.

## Notes

## Decisions (running log)

The review is inspection-only. It uses producer commit
`4ccb9caf7214130f2d601d3a3ddc91611b090d5a` and current source. No tests,
builds, lint or formatting commands were run. The producer records 1083 passing
tests and passing Ruff and Pyright checks; those are recorded evidence, not a
fresh review run. The graph generation `2026-11-15T02:19:52Z` has changed or
untracked coverage for the relevant files, so the findings use direct source
reads rather than treating graph absence as evidence.

Two findings meet the acceptance and material-risk threshold: publication can
credit a close acknowledgement first read after P, and the terminal residual
list omits an unreaped inspection worker. The integration belongs as a flat
sibling under k162: it can triage these concrete defects, repair accepted
findings and validate them without another context boundary. The review's own
directory has no later sibling, so appending places that integration next.
The parent remains live for that work; this review does not close k162.

## Findings

### P1 — Reject close acknowledgements first read after P

At `experiments/exec-reliability/controller.py:1501`, `_bounded_turn()` calls
the inner advance before checking its deadline. That reaches
`experiments/exec-reliability/storage_client.py:308` and then lines 324–326:
reap, read acknowledgements and write frames all precede the deadline check.
At `experiments/exec-reliability/storage_client.py:459`, an `ok` close changes
storage to `closed` without a receipt timestamp or publication cutoff.

If the last publication turn wakes after P and the close acknowledgement is
now readable, it is accepted. Back in
`experiments/exec-reliability/controller.py:1531`, a still-live writer then
gets the expected-exit path rather than KILL. Once reaped,
`experiments/exec-reliability/controller.py:1561` reports `complete` based
only on `closed` and reaping. A writer already reaped on that late turn can
also have its buffered close acknowledged and be credited as complete. With
an otherwise passing result, the overall verdict can therefore be true.

This violates the spec's definition at
`docs/specs/diagnostic-evidence-storage.md:261`: completion requires an `ok`
close before P. It also contradicts the foundation report's explicit
read-by-cutoff limit in "Delivered behaviour (k162)". Scheduling limits do
not justify crediting an acknowledgement first observed after the cutoff.
The existing healthy and result-snapshot controls do not exercise this late
close boundary. Integration should verify the deadline decision independently
of the eventual storage state and cover late reads, including an already
exited writer; such a control must reject the current implementation.

### P2 — Include residual inspection workers in the terminal residual list

At `experiments/exec-reliability/controller.py:1790`, the new `RunOutcome`
passes only call receipts and storage to `_residuals()`. The collector at
`experiments/exec-reliability/controller.py:1621` enumerates call leaders,
groups and the writer, but receives no inspection-worker ownership evidence.

An inspection worker that remains unreaped after its bounded close is a
supported terminal path: `experiments/exec-reliability/worker_capture.py:618`
records `residual_ownership`, and
`experiments/exec-reliability/worker_capture.py:704` supplies its residual
remedy. Its identity survives in the nested result through
`experiments/exec-reliability/controller.py:1768`, but if call processes and
the writer settled, the returned top-level `residuals` list is empty despite
that retained child. A caller consuming the terminal list loses the process
requiring independent reconciliation and reap.

The spec at `docs/specs/diagnostic-evidence-storage.md:252` requires any
residuals in the in-memory outcome; the `RunOutcome` documentation also
promises each owned process left behind. Integration should include the
settled worker's residual identity and verify both residual and reaped worker
paths without restoring runtime signal authority.

## Review coverage and limits

The inspected source starts publication after teardown and worker close,
validates a finite positive W no greater than 10 seconds, computes the digest
cutoff at half the publication window, queues one close, uses nonblocking
client turns and fixed P+2 reap deadlines on the ordinary close paths, and
retains the `Path` return through `RunDirectory`. The overall verdict checks
for publication `complete`; the late-close finding concerns how that state
is established.

The audit control starts its window after successful readiness and ends it
after the run returns; it observes import and filesystem audit events. The
producer's recorded control results and mutant checks were inspected. They
do not establish the two failure scenarios above, and were not rerun here.
No production or test files were changed. The foundation's acceptance claims
need to remain consistent with whichever findings integration accepts.
