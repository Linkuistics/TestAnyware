# first-leader-reap-integrate-k93

**Integrates:** first-leader-reap-k92

## Goal

Triage the findings of the k92 review of the k88 first workload-leader reap boundary. Apply each real one. Keep the one-line k88 loop change unless a finding proves it wrong. Keep the k91 aggregate charter unchanged.

## Context

Read the findings from the k92 commit, not from this body. Reviewed commit: 6b678ed (complete-worker-scheduler-k88). Source: `experiments/exec-reliability/controller.py` (`_reap_leader`), `test_controller.py` first-reap cases, `Taskfile.yml` `diagnostic:leader-reap-*` tasks. Evidence: `experiments/exec-reliability/evidence/leader-reap-k88/`. Contract: `docs/specs/diagnostic-process-inspection.md`.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a trade-off or noise. Record the class in the decision log.
- For each real runtime or evidence finding, add a red control first. Then change the artifact. Then retain green evidence in a new directory.
- Preserve all earlier evidence bytes. Run the full diagnostic self-check and retain it, failures included.
- Route anything that belongs to the aggregate scheduler to complete-worker-scheduler-k91 by name. Do not fix it here.

## Decisions (running log)

- Created by the k92 review. Classification is pending.
