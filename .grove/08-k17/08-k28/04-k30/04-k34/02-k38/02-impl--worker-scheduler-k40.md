# worker-scheduler-k40


## Goal
Complete the remaining worker-scheduler-k38 contract after nonblocking-leader-reap-k39.


## Context
Read every ancestor brief and docs/specs/diagnostic-process-inspection.md in full.
k39 changes the actual workload reap boundary only. It introduces a private
Popen subclass with no destructor polling, retained ownership on failed reap,
exact-PID WNOHANG before K, and inspection cutoff K-min(100 ms,A/4). This remains
a synchronous controller: helper creation/capture/waits can overrun the cutoff.
The existing inspection.py worker draft remains untrusted and incomplete.

## Done when
- Deliver every remaining parent requirement: private helper-owning worker;
  native/Python thread and SIGCHLD admission before every fork; child signal,
  wakeup-fd and descriptor reset; bounded separate channels and gated publication;
  one shared safety-first scheduler with immutable deadlines/closed authority;
  sticky failures and concurrent resource teardown, worker failure and shutdown;
  nonblocking worker/helper reaps, strong owners and distinct residuals.
- Preserve k33/k36 signal/deadline ordering, k37 strict table validation and k39
  workload reap ownership. Reconcile the draft against the runtime specification.
  Repeated synchronous queries are still present; the required one final query,
  stale callback rejection and full concurrency semantics are not delivered.
- Run the complete required real control inventory from the specification and
  ancestor briefs, including fault timing, child-reset seams, actual helper KILL
  and wait seam, protocol abuse, cutoff and concurrent teardown. Keep simulation,
  setuid-helper kill timing and unavailable genuine UID denial distinct.
- Freeze new inputs/receipts, independently audit every exact helper/fixture,
  update the durable report, and preserve all historical evidence byte-for-byte.
  Extend Taskfile and input declarations as new runtime files require.
- As the final producer act insert review-impl bounded-inspection before
  retained-evidence-k21, with **Reviews:** bounded-inspection-k30 and the complete
  required runtime ownership/wait/destructor/authority/concurrency/control audit.
  No empty integration. Decompose only at another independently useful green
  behavior boundary without narrowing the contract.

## Notes
