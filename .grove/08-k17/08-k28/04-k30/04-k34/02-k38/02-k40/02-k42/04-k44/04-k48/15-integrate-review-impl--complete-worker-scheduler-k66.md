# complete-worker-scheduler-k66

**Integrates:** complete-worker-scheduler-k65

## Goal

Triage the one finding of the k65 review of the k62 raw helper fork boundary and apply it if it is real. Keep k64's full original contract unchanged.

## Context

Read the findings from the k65 commit, not from this body. Reviewed commit: 25b74d9ad97d (complete-worker-scheduler-k62). Source: experiments/exec-reliability/inspection.py:174-189 (`_helper_child`) and :361-388 (`reset_after_fork`, `os.setsid()` at :387). Spec: docs/specs/diagnostic-process-inspection.md:29-43 and :129. Test: experiments/exec-reliability/test_inspection.py:700. Report: experiments/exec-reliability/evidence/raw-helper-fork-k62/report.md.

Summary pointer only: the helper became its own session, while the spec says it runs in the worker's group. The k62 charter asks for "session before exec", so the two documents disagree.

## Done when

- Classify the finding: a contract stated unclearly, a real issue, a visible trade-off, or noise. Reproduce it from the source and the retained green03 observation before acting.
- If the helper stays a detached session, amend the spec so it says so and says what contains a hung helper. If it returns to the worker's group, change the helper reset so it does not call `setsid`, and keep the worker's own `setsid`.
- Add a retained control that sends a real group KILL to a worker with a live helper and records what happens to the helper. Label real and adapter parts. Preserve every earlier evidence byte for byte, and freeze subjects and bounds for the new run.
- Update the report so it states where the helper's group and session now sit.
- Do not widen k64. Keep every aggregate ancestor open. Ten-clone acceptance stays 0/10 unavailable. Acquire no privilege and touch no guest or installed application.

## Notes

No provider evidence was present in the review session. No Creator line was written.
