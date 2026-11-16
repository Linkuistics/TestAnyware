# storage-publication-k181

**Integrates:** storage-publication-k180

## Goal

Triage the publication review and repair accepted findings against the agreed
storage contract, then establish whether evidence-writer-k162 can close.

## Context

Read the findings from the commit for `storage-publication-k180`. The producer
is `storage-publication-k168`; the contract is
`docs/specs/diagnostic-evidence-storage.md`. The foundation report's
"Delivered behaviour (k162)" records the delivered scope and limits.

## Done when

Classify every review finding against the source and agreed acceptance as an
unclear contract, actionable issue, visible trade-off or noise. Record the
disposition and evidence. Repair accepted findings and run the relevant
reusable checks through the project Taskfile, including controls that fail
without each repaired rule. Keep the foundation report consistent with the
verified behaviour and its limits. Check the parent brief's completion
conditions before closing k162; retain concrete unfinished acceptance as live
work. Another review round requires a concrete change or new evidence needing
independent challenge.

## Notes

This is a bounded triage, repair and validation handoff under the existing
node. The review commit owns the findings; this body does not pre-accept them.

## Decisions (running log)

- **P1, late close acknowledgement: actionable issue, repaired.** The spec
  says `complete` needs an `ok` close acknowledgement that arrives before P.
  On the k168 build, a turn after P could read that acknowledgement. The
  storage state then became `closed`, the writer got no KILL, and the outcome
  was `complete`. `StorageClient` now records `closed_at` when a turn reads the
  acknowledgement. `_RunStorage._closed_before_deadline()` compares it with P.
  That check now selects the no-KILL path, the `complete` outcome and the
  identity anchor.
- **P1 control.** `test_close_acknowledgement_read_at_p_is_late` holds the
  client's reads until P. Its `live` case gives `killed`, and its `exited`
  case gives `incomplete`. Neither case anchors an identity. Both cases fail on
  the k168 controller with `complete`. Both also fail when only the
  `closed_at < P` comparison is removed.
  `test_close_acknowledgement_read_before_p_is_complete` keeps the timely path.
- **P2, residual inspection worker: actionable issue, repaired.** The run
  outcome named only call and writer residuals. `_residuals()` now takes the
  worker evidence. It names a worker with a residual remedy by its PID and
  remedy. The worker's own close sets that remedy, so the controller gains no
  signal authority.
- **P2 controls.** `test_run_outcome_names_a_residual_worker` and
  `test_run_outcome_omits_a_reaped_worker` drive a real `Worker` close with
  stubbed process calls. The residual case fails on a mutant that drops the
  worker entry. Both cases fail on the k168 controller.
- **Checks.** `mise exec -- task diagnostic:test` passed 1088 tests with no
  expected failures. `mise exec -- task diagnostic:storage-controls` passed
  28 tests. Project-scoped `pyright`, `ruff check` and `ruff format --check`
  report no findings.
- **Report.** The foundation report subsection "Delivered behaviour (k162)"
  now states the close cutoff before P, the late-close control and the worker
  residual.
- **No further review.** Each repair applies a rule that the spec already
  states. Each repair has a control that fails without it. No new contract
  needs independent challenge.
- **k162 close.** Each item in the parent brief's "Done when" holds. The
  storage controls pass with no expected-failure marker. `execute()` returns a
  `Path`. This leaf changes neither `peer.py`, `preflight.py` nor
  `retention.py`. The report records the delivered behaviour and its limits.
