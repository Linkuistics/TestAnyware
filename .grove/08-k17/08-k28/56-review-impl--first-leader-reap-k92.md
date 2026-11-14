# first-leader-reap-k92

**Reviews:** complete-worker-scheduler-k88

## Goal

Independently challenge the first workload-leader reap boundary in the committed
k88 artifact before complete-worker-scheduler-k91 changes or relies on it.

The contract is in docs/specs/diagnostic-process-inspection.md, especially the
original reap deadline, closure of target authority, exact-child nonblocking
reaping and ownership requirements. At equality or after K, the first exact-PID
WNOHANG attempt must still occur after authority closes. Later polls and sleeps
are permitted only within the original K. A late successful reap releases its
strong owner and retains status but must fail timing. Unsuccessful or errored
expired attempts retain the residual owner and errno where present. The caller
must not turn a late observation into success or grant another wait interval.

## Scope and evidence

Review experiments/exec-reliability/controller.py (_reap_leader and its caller),
test_controller.py (new boundary cases and changed expired-owner expectation),
the diagnostic:leader-reap-* Taskfile tasks, and
experiments/exec-reliability/evidence/leader-reap-k88/. Read the raw red-1,
green-1, full-1 and green-2 verdicts, JUnit, child receipts and source snapshots.
Check their actual provenance, exact test identities, bounds and cleanup against
the contract. Challenge the auditor and its missing-record negative control;
verify its archive exceptions, inventory and limits independently. Distinguish
real-child observations from simulated clock/wait adapters. Do not infer the
full worker/scheduler boundary from these controls.

## Done when

- Produce actionable findings anchored to the committed artifact and contract,
  or explain which bounded scope was challenged without finding a defect.
- Assess ownership/cleanup, fixed deadlines and failed-timing preservation, and
  whether the retained controls can detect a broken first-attempt boundary.
- If findings require reconciliation, insert an integrate-review-impl sibling
  before complete-worker-scheduler-k91 and point it to this review; do not copy
  findings into that task or add an empty integration placeholder.

The entire original aggregate charter remains in complete-worker-scheduler-k91.
Its separate final review must name bounded-inspection-k28 before k21. k28 and
k17 stay open; ten-clone acceptance remains 0/10 unavailable. Existing historical
permission-table and empty-PID fixture failures remain unresolved.
