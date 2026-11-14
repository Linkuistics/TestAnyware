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

## Decisions (running log)

- The k65 finding is a **real issue**. The review commit `7ceaab2a8f1c6d62f072b6dedbe30cff705900cf`, `_helper_child` → `reset_after_fork` source, and retained green03 pre-exec receipt (`helper-raw-helper-reset-real-preexec-observation-a2f8ee3fd49f4fe39bb9357183728364.json`: PID/PGID/SID 9602, PPID 7885) agree that helper reset detached containment. Keep the spec's worker-group design: helper reset must preserve its inherited PGID/SID; worker reset still creates its own session. This is a local repair, not a redesign of k64.
- Add a real production-worker/exec-helper group-KILL control. Observe the helper's kernel exit event independently; never infer helper death or reap from worker status or killpg return. Label the helper fixture, cooperative cleanup and missing helper-reap evidence explicitly. Retain red and green runs with fixed bounds and exact before/after subjects; leave all earlier evidence unchanged.
- The retained `worker-group-k66/red01` run fails in exactly the two containment controls (257 passed, 2 failed). Its real worker 50518 was reaped with -9 while detached helper 50519 produced no exit event inside 0.5 seconds; cooperative fixture release then produced the helper exit event. Apply only the reset session option: `new_session=True` remains the worker default, and `_helper_child` passes `False`. This preserves every other reset action.
- The repaired `worker-group-k66/green01` run passes all 259 individual cases, lint, format and strict types. Real helper 7070 has PPID/PGID/SID 7066, matching its worker; the kernel independently reports helper exit after the actual group KILL and the exact worker is reaped with -9. Helper reap acknowledgement remains unavailable after worker death. No k64 obligation is discharged by that missing evidence.
- The frozen evidence audit passes: both runs have stable 14-subject hashes, twelve verified nonbinary source copies, matching individual-case inventories, exact expected RED failures and verified artifact hashes. All 55,603 earlier evidence files (including ignored disk files) have unchanged membership and bytes. The k62 report is part of that immutable evidence, so the current group/session correction is documented in the new `worker-group-k66/report.md` and linked from the durable experiment report. No earlier sample is overwritten.
