# complete-worker-scheduler-k101

## Goal

Declare and enforce worker bootstrap readiness and failed-bootstrap TERM/KILL-reap budgets before freezing a diagnostic run or acquiring resources.

## Done when

- Immutable plan bootstrap bounds reject invalid/nonfinite/nonpositive/over-ceiling inputs before evidence creation, hashing or fork; direct Worker use also validates before fork.
- Persistent bootstrap uses the readiness declaration (<=1s); bootstrap failure fixes TERM grace and KILL/reap deadlines (each <=2s), preserving any earlier owner deadline and PID-only authority until readiness. No restart or renewed allowance.
- Receipts retain declarations, absolute readiness/shutdown phase deadlines, outcomes and measured overruns; an overrun cannot report successful preparation.
- Real owned-child readiness-delay, TERM/KILL and deferred-reap controls plus validation/controller integration controls have frozen RED/GREEN evidence, hashes and independent exact-child audits. Full diagnostic tests/lint/format/type checks pass. Old evidence remains unchanged.
- All original aggregate work remains owned by flat sibling complete-worker-scheduler-k104. k28/k17 remain open. Cut a separate review-impl for this bootstrap slice as the final producer act.

## Notes

This is the first slice of the original full-runtime mandate, retained at complete-worker-scheduler-k104. Bootstrap is independently useful: callers can select shorter preparation bounds, invalid plans cannot create a worker, and failed preparation accounts for its child under declared deadlines. Worker-failure/orderly-close declaration and shared-scheduler obligations remain open there; this slice establishes neither aggregate completion nor teardown overlap.

## Decisions (running log)

- 2026-11-15: Chose a flat sibling because the full runtime exceeds one focused session while sharing k28 context. k101 delivers bootstrap declarations end to end; k104 preserves the entire original charter and history. Existing resource/target deadlines and request shutdown remain untouched. Bootstrap phase clock observations are conditional OS scheduling guarantees; every measured overrun is failed evidence.

- 2026-11-15: BootstrapBounds is immutable and validates both Plan and direct Worker entry before side effects. Only failed startup selects its new TERM/KILL-reap durations; the earlier owner cutoff still wins. Receipts include declared bounds, absolute readiness/shutdown deadlines, readiness outcome/overrun, terminal outcome, signal attempts and residual ownership. Readiness records the exact acceptance timestamp so a later finally block cannot turn accepted readiness into an unreported overrun.
- 2026-11-15: Retained the first full candidate (green1): 621 passed and the ready fixture failed because it called reset_after_fork twice, attempting setsid twice; pyright flagged redundant typed isinstance guards. Corrected the fixture to let the real service own reset and made runtime guards explicitly inspect object values. The next frozen focused run (green2) passes all 30 controls; aggregate/shared-scheduler acceptance remains open in k104.

- 2026-11-15: Final frozen full-final passes 622 tests, lint, formatting and strict types. All six defect-restoration conditions fail their intended controls (24/1/4/2/4/1 cases respectively); all measured subject pairs remain unchanged. Independent condition audit validates retained artifact hashes, 50 bootstrap child reaps, checker-parent reaps and final live/frozen byte agreement. The initial prior-evidence audit followed symlinks incorrectly; its raw false-positive result remains, with a separate link-text-aware check against the pre-session parent commit. See the retained bootstrap report for exact claims and limitations.

- 2026-11-15: Link-aware parent audit confirms all 123,249 prior evidence paths (18 symbolic links included) unchanged. ADR reconciliation requires no new decision: this is parameterization of the existing diagnostic bootstrap contract; streaming transfer and per-command guest connection ADRs remain applicable. k104 keeps the entire remaining aggregate, so k28/k17 do not close. A separate review-impl is the final producer action; no in-session adversarial reviewer was dispatched because that scheduled fresh context owns the review.
