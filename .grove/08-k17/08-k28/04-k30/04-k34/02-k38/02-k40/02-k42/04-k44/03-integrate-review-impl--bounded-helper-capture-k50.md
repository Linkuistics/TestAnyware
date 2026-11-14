# bounded-helper-capture-k50

**Reviews:** bounded-helper-capture-k49

## Goal
Reconcile the findings of review k49 on the helper capture and reap path before worker-scheduler-k48 changes the ownership boundary.

## Context
The findings are in the Decisions log of k49, anchored to commit 2d25827 (producer k47). Re-read the cited code before changing it, because positions may have moved. The cited file is `experiments/exec-reliability/inspection.py` unless a finding says otherwise.

## Done when
- Each k49 finding is fixed, or rejected with a reason in this leaf's decisions.
- Every exit from `capture_helper` after Popen returns, including an exception from stream closure, makes at least one final nonblocking reap attempt and returns a complete receipt.
- After the last wait, the capture observes a signalled helper once more. A late reap is then not a residual.
- Late Popen creation has a control that states what the helper does afterwards: signalled or left running.
- Retained evidence keeps the initially failed lint condition k47-check-01.

## Notes
Scope is this slice only. k48 keeps the worker/scheduler contract and synchronous creation. Do not credit simulated controls as privileged denial tests.
