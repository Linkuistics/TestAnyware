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

- F1 is a real issue: `execute()` implicitly freezes inspection and worker capture
  but omits their shared `turns.py` dependency. Add a real peer-driven drift control
  against an isolated runtime copy before adding the missing implicit subject.
- F2 is partly a visible trade-off: each production wrapper currently drives one
  turn, so changing its safety flag cannot change ordering. Per-wrapper flag
  mutants would certify an unobservable property. Keep that limitation visible
  for k126's actual shared scheduling and its review. The shared-driver safety
  ordering controls remain applicable. The initial-due clock bypass is observable;
  add a direct clock-consumption control and reject a scratch mutant removing it.
- The real peer drift control failed before the subject fix, then passed with
  distinct recorded before/after SHA256 values and a failed measurement receipt.
  Removing the subject and removing the initial clock bypass each produce the
  intended one-test failure in separate frozen mutants. Nine focused controls,
  all 875 full-suite cases and Ruff/format/strict-Pyright checks pass. All 873
  prior case identities/outcomes match k124, with only the two new passing cases.
- All 204,212 earlier evidence paths are unchanged. The frozen checker inputs
  and recorded artifacts verify, with no timeout or residual fixture. Evidence
  and reproduction entries are in
  `experiments/exec-reliability/evidence/driver-review-k128/report.md`.
  k126 retains the production scheduler, teardown overlap and final review;
  k28/k17 remain open and guest acceptance remains 0/10 unavailable.

