# termination-signal-cutoff-k90

**Integrates:** complete-worker-scheduler-k89

## Goal

Triage the findings of the k89 review of the k85 workload termination cutoff slice. Apply each real one. Keep the other k85 runtime guards unchanged unless a finding proves them wrong. Keep the k88 aggregate charter unchanged.

## Context

Read the findings from the k89 commit and its task file, not from this body. Reviewed commit: f5a081e (complete-worker-scheduler-k85). Source: `experiments/exec-reliability/controller.py` (`_terminate`), `test_controller.py` termination-cutoff cases, `run_condition.py`. Evidence: `experiments/exec-reliability/evidence/termination-cutoff-k85/`. Contract: `docs/specs/diagnostic-process-inspection.md` lines 201-245.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a trade-off or noise. Record the class in the decision log.
- For each real runtime finding, add a red control first. Then change the runtime. Then retain green evidence in a new directory.
- Preserve all earlier evidence bytes. Run the full diagnostic self-check and retain it, failures included.
- Route anything that belongs to the aggregate scheduler to complete-worker-scheduler-k88 by name. Do not fix it here.

## Decisions (running log)

- Created by the k89 review. No decision yet.
