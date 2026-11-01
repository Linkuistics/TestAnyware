# bounded-inspection-k30


## Goal

Deliver the complete bounded inspection worker and shared scheduler through the
real controller, with the evidence and limits required by bounded-inspection-k28.

## Context

Read the parent `_bounded-inspection.md` in full: its Done when and Required
control inventory remain binding. fork-admission-k29 supplies only initial
preacquisition rejection, not worker startup or safe fork handoff. Review and
reconcile that producer before building on it. The earlier unintegrated worker
draft survives at jj snapshot `0f8aca80`; it has known protocol/shutdown defects
and is not a trusted implementation. Do not restore it as delivered code.

## Done when

- Satisfy every worker/scheduler, authority, fixed-deadline, protocol, resource
  safety, shutdown and residual requirement in the parent charter and spec.
- Repeat thread/SIGCHLD admission before each worker/helper fork, with blocked
  signal/reset/fd/exit handling; initial admission alone is insufficient.
- Demonstrate the parent's complete real-controller control inventory with frozen
  per-case receipts and independent identity audits. Keep simulated faults and
  unavailable genuine privileged/timing controls explicit. Preserve prior evidence.
- Extend runner subjects and reusable tasks, update the foundation report and
  provide k21 immutable inputs while keeping k18 storage scope separate.
- As the final producer act insert specific review-impl `bounded-inspection`
  before retained-evidence-k21, `**Reviews:** bounded-inspection-k30`, requiring
  the actual runtime traces and audits stated in the parent. No empty integration.

## Notes
