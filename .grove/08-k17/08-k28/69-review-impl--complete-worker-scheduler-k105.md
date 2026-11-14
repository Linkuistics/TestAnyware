# complete-worker-scheduler-k105

**Reviews:** complete-worker-scheduler-k101

## Goal

Independently try to break the committed k101 declared-bootstrap slice against its narrowed contract and the diagnostic inspection specification. Produce findings, not runtime fixes; do not treat the retained passing runs as a verdict.

## Context

Read the k101 producer and `experiments/exec-reliability/evidence/bootstrap-bounds-k101/report.md`, frozen inputs, raw per-case receipts, condition audit and both prior-evidence audits. Runtime scope is `controller.Plan`/validation/construction and `worker_capture.BootstrapBounds`, Worker startup, failed-bootstrap close and evidence. Checker scope includes `test_bootstrap.py`, preparation/condition runners and Taskfile recipes. The full original aggregate remains in complete-worker-scheduler-k104; k28/k17 stay open and 0/10 guest acceptance remains unavailable.

## Done when

- Re-derive whether Plan and direct Worker reject invalid declarations before evidence creation, hashing, pipes or fork; whether successful readiness and failed-bootstrap TERM/KILL-reap actually use the selected bounds and retain an earlier owner cutoff, signal history and PID-only pre-readiness authority.
- Challenge absolute phase receipts, boundary/equality semantics, delayed syscall observation, sticky failure/no restart and residual ownership without renewed signal authority. Check that healthy/request-failure close was not accidentally changed.
- Verify frozen RED/GREEN and six source mutations against the contract, case identities, child reaps, artifact/subject hashes and the link-aware preservation audit. Distinguish actual child signals from simulated observation stalls, and test proof strength rather than accepting aggregate counts.
- Record each finding and its evidence in the review's artifact; classify scope gaps against k104's retained aggregate without turning them into bootstrap acceptance. Preserve all existing receipts. Perform authorized local owned-child checks only; no guests, privilege, broad signals or installed app/CLI changes.

## Specific doubt

Do these selected phase bounds and sampled receipts remain truthful when readiness is late, a close phase inherits an earlier cutoff, or observation returns after expiry? Do the tests independently enforce the advertised lifecycle and cleanup contract, including the direct Worker seam, rather than merely checking matching constants? This is one deliberately aimed fresh-context review of the bootstrap slice.
