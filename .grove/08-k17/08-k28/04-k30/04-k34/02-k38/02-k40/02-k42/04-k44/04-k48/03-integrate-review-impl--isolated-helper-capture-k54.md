# isolated-helper-capture-k54

**Reviews:** isolated-helper-capture-k53

## Goal
Reconcile the findings of review k53 on the worker channel before complete-worker-scheduler-k52 builds on it.

## Context
The findings are in the Decisions log of k53, anchored to commit 0189003 (producer k51). Re-read the cited code before changing it, because positions may have moved. The cited files are in `experiments/exec-reliability/`.

## Done when
- Each k53 finding is fixed, or rejected with a reason in this leaf's decisions.
- The worker starts correctly when the controller has a standard stream closed, or the leaf records why it cannot.
- A malformed or hostile result frame never raises past `capture_helper`. The receipt records the failure and unknown creation.
- Controls cover oversized, malformed, duplicate-key and early-exit frames. Controls state which are real and which are adapters.
- Retained evidence keeps every earlier failed condition.

## Notes
Scope is this slice only. k52 keeps the persistent worker, gated publication and the scheduler. Do not credit adapter controls as live worker evidence.

## Decisions (running log)
