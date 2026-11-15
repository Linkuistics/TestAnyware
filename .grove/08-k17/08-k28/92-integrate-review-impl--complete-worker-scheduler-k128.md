# complete-worker-scheduler-k128

**Integrates:** complete-worker-scheduler-k127

## Goal

Triage the findings of complete-worker-scheduler-k127 against the k124 shared turn driver. Apply the real ones narrowly. Leaf k126 keeps the shared scheduler, teardown overlap and the final k28 review.

## Context

Read the k127 Decisions log (findings F1 and F2) and evidence/common-driver-k124/report.md. The k124 runtime is turns.py, controller.py and worker_capture.py under experiments/exec-reliability.

## Done when

- Each finding has a classification (unclear contract, real issue, visible trade-off or noise) with evidence.
- F1: decide whether the plan subject list at controller.py:765-776 must name turns.py. If yes, add a red control first. The control must show that a changed turns.py changes `subjects_after`. Then add the entry.
- F2: decide whether per-consumer `safety` flags and the initial-due shortcut need mutation controls. Single-turn consumers cannot observe the flag today. If a control would only restate that, record the trade-off instead.
- Keep every prior sample, mutant and receipt unchanged. Use fresh output directories.

## Notes

No guests, privilege, installed changes or unrelated processes. Do not accept this slice as shared scheduling, teardown overlap or aggregate acceptance. k28 and k17 stay open.

## Decisions (running log)

