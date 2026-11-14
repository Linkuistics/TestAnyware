# admission-worker-state-k63

**Reviews:** complete-worker-scheduler-k60

## Goal

Independently review k60's admission and failure-state receipt validation before the persistent-worker/scheduler remainder in k62 relies on it.

## Context

Read the producer commit by its stable handle, k60, k48, docs/specs/diagnostic-process-inspection.md and the k60 report section in docs/experiments/exec-reliability-diagnostic-foundation.md. Re-derive the genuine producer states in inspection.py against worker_capture.py; the k61 review identified ECHILD with retained ownership in the earlier positive control.

## Done when

- Trace complete admission schema and reason priority, native-observation failure states, strict counts/booleans/errnos/signal values and meaningful errors against the actual producer. Check coherent admission refusals survive as known no-helper failures and contradictions fail conservatively.
- Check error/creation/output/close coupling, ECHILD and ownership equivalence, terminal state and residual remedy consistency without conflating receipt self-consistency with independently authenticated helper identity or reap.
- Reconcile the 50 synthetic publications and genuine two-thread admission-refusal control with retained body witnesses, real worker transport, exact rejection reasons, parent timing and shutdown. Check the EIO replacement and separate contradictory ECHILD negative control. Keep synthetic helper evidence distinct from actual helper observations.
- Audit immutable evidence/k60-inputs-01, k60-red-01, k60-fixed-01, k60-fixed-02, k60-audit-01 and k60-audit-02. Check exact 45 RED failures, identical 251-case inventories, final lint/format/strict types, both failed attempts, before/after hashes, 46,714 preserved prior files and external selector/scratch reconciliation. Account for audit limitations and avoid promoting later absence into proof about unknown creation.
- Preserve k62's entire persistent bootstrap/gated publication/independent ownership/shared scheduler/control/final-review charter and all aggregate ancestors. No guest, privilege, installed-app, stalled-storage or ten-clone acceptance claim; 0/10 remains unavailable.
- Record actionable findings with source coordinates and evidence. Cut an adjacent integrate-review-impl only for findings worth acting on, before k62. Create no empty integration.

## Decisions (running log)

- Review of k60 (1eeea8a389e7) is inspection only. No test, lint, format or type command ran. No production or test file changed.
- Admission schema: `_admission` (experiments/exec-reliability/worker_capture.py:125-168) matches `fork_admission` (experiments/exec-reliability/inspection.py:82-117). Reason priority is python_threads, native_threads, sigchld, ready. Native failures need `ok` false, an error and no native SIGCHLD. Counts and errnos reject `bool` by `type(x) is int`.
- Failure state: the post-checks at worker_capture.py:266-289 match `capture_helper` (inspection.py:178-289). They cover refusal with pid, no-helper wait/output/close, ECHILD equals ownership loss, error presence, and remedy equals retained ownership. A reap failure other than ECHILD keeps ownership and a remedy, so the EIO control is a genuine producer state.
- Tests: 251 cases in each of red-01, fixed-01 and fixed-02 with identical inventories. RED has 45 failures: 44 `test_worker_failure_consistency` cases and the one real two-thread refusal control. That gives 38 malformed, six coherent refusals and one genuine control. fixed-01 fails only strict types (verdict `types` exit 1). fixed-02 passes all four checks.
- Evidence reconciliation: `prior-evidence.json` holds 46,714 entries and all exist on disk. 34,392 are tracked in the parent commit. The other 12,322 are gitignored (`.venv` 12,254, `__pycache__` 34, `.pytest_cache` 29, `.ruff_cache` 3, two other). The "unchanged" claim holds for disk. It cannot be re-checked from version-control history alone.

## Findings

No actionable finding. No integrate-review-impl leaf was cut. k62 keeps its full charter. 0/10 stays unavailable. The review makes no guest, privilege or acceptance claim.

Non-actionable observations, for k62 to remember:

1. The decoder does not tie `receipt.error` text to the admission reason. A receipt with `admission.ok` false, no pid and any non-blank error passes. The producer always writes "helper fork admission rejected: <reason>". This is receipt self-consistency, not authentication, and the structural contradictions all fail.
2. `native_observation_error` accepts `errno` None. The producer can emit it for an errno-less `OSError`, so the acceptance is correct.
3. Positive `errno` values have no upper bound. No producer path depends on one.
4. 12,322 of the 46,714 preserved files are ignored by version control. A later reader must not treat the commit alone as the preserved set.
