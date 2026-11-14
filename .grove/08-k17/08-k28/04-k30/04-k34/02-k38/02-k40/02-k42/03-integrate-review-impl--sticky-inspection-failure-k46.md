# sticky-inspection-failure-k46

**Reviews:** sticky-inspection-failure-k45

## Goal
Reconcile the findings of review k45 on the sticky-inspection-failure slice before the worker/scheduler implementation builds on it.

## Context
The findings are in the Decisions log of k45, anchored to commit 35d806d. Re-read the cited code before changing it, because positions may have moved.

## Done when
- Each k45 finding is fixed, or rejected with a reason in this leaf's decisions.
- Unobserved leader exit and an exhausted cutoff stay distinct from a failed inspector query in the retained receipts.
- A control exercises a real failed query and checks that later calls stay unobserved.

## Notes
Scope is this slice only. k44 keeps the full worker/scheduler contract.
