# complete-worker-scheduler-k121

**Integrates:** complete-worker-scheduler-k120

## Goal

Triage the findings of complete-worker-scheduler-k120 against the k116
implementation and apply the real ones.

## Context

Read the findings in the k120 commit and its leaf file (Decisions log). Grade
each against the source and the spec. The k116 slice stays narrow. Leaf k119
owns the shared scheduler, teardown overlap and the final k28 review.

## Done when

Each finding has a classification (unclear contract, real issue, visible
trade-off or noise) with evidence. Reproduce F1 with a red control before any
production change. Keep every prior sample and receipt unchanged.

## Notes

No guests, privilege, installed changes or unrelated processes.

## Decisions (running log)

- 2026-11-15: F1 is a real test gap, rather than a runtime defect. The
  committed review identifies M1 (result EOF alone) and M3 (EAGAIN treated as
  control EOF). The source still requires both real EOFs. Add a real empty-open
  control with a controller-owned, bounded child retaining only that pipe write
  end. Unlike an orphaned worker descendant, this holder can be released and
  reaped by its exact parent. No pipe bytes or EOF will be synthesized. First
  freeze the existing controls and reproduce both surviving mutants; then
  require the new control to reject them. Production stays unchanged unless
  those observations reveal a separate defect.
- 2026-11-15: F3 is an unclear contract: timeliness applies to admission of the
  explicit failure, whereas EOF closes its framing. The implementation permits
  that framing closure in the existing fixed shutdown reserve, without
  admitting a new late failure or extending the request deadline. Clarify that
  boundary and test a real EOF observed during cleanup. The shared scheduler,
  historical deferred-reap failure and aggregate acceptance remain with k119.
- 2026-11-15: F2 is a visible trade-off: retain the eager cleanup conflict
  guard. Removing it was not shown to violate a requirement; a surviving mutant
  alone is not a defect or a proof of equivalence. Buffered tails and parsed
  duplicate controls provide overlapping rejection, but capped reads and
  persistent completion controls preclude claiming universal equivalence from
  the focused sample. Measure M2 with the expanded controls and disclose its
  survival if observed; do not remove a conservative guard to improve a score.
- 2026-11-15: Retained old-controls, old-result-eof-only and
  old-eagain-is-eof under `evidence/fork-authority-k121/`: all 37 cases passed
  each, reproducing F1 before any runtime edit. The expanded suite rejects
  each unsafe mutant with exactly one failure,
  `test_fork_failure_empty_control_channel[open]`, at the false known-creation
  classification. The other 38 cases pass. Unchanged production passes all
  39 cases. Its cleanup-eof case observes real EOF after the query cutoff and
  before the unchanged absolute deadline. The controller-owned holder is
  released and reaped in every sample. No bytes, EOF, clock or reap result
  were synthesized by the new fixture.
- 2026-11-15: M2 still passes all 39 expanded cases. F2 remains a visible
  trade-off with an explicit measurement limit, not a runtime fix. The
  conservative guard is unchanged. F3 is clarified in the process-inspection
  spec specifically for the lost-terminal path: admission is timely, framing
  EOF can close in the existing reserve, and the failed observation stays
  failed. Neither empty-open nor EAGAIN establishes closure. A single full
  suite and static-check sample follows; no aggregate scheduler or acceptance
  claim will follow from it.
- 2026-11-15: The finite full sample retained 855 passes and one failure,
  `test_worker_shutdown_deadline_closes_owner_operations[stale-term-pid]`.
  Its audit records deadline 335692.452171875 and one WNOHANG observation at
  335692.452192166 (20.291 microseconds late); the independent exact-child
  audit reaped PID 40652, status 15. This shared deadline failure stays with
  k119, separately from k116's historical deferred-reap failure. No repeated
  full sample was used to replace it. Both new controls passed. Ruff and format
  passed; Pyright exposed eight auditor import diagnostics from the frozen cwd.
  The import fix's first static-only sample retained two strict typing errors;
  an explicit list type fixes those. The final static-only sample passes Ruff,
  format and strict Pyright from the same frozen cwd. Initial/intermediate
  tooling and every failed log are retained; the full suite remains red.
- 2026-11-15: Durable reconciliation and finite limits are recorded in
  `experiments/exec-reliability/evidence/fork-authority-k121/report.md`.
  Runtime unchanged; no shared scheduler redesign or aggregate closure is
  claimed. Existing tool provision worked, so the authorized optional
  `mise.toml` addition was unnecessary.
- 2026-11-15: Final evidence audit passed. All 189318 prior evidence paths
  are unchanged. The audit reconciles the exact mutants, raw/JUnit outcomes,
  frozen source equality, checker bounds and reaping, all ten new holder reaps,
  shared deadline audits, and the final static checks. Its verdict explicitly
  retains `full_suite_passed: false`. k119 is still live, so k28/k17 do not
  close and no ancestor promotion or teardown is due.
