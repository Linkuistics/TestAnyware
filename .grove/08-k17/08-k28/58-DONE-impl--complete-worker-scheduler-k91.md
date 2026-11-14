# complete-worker-scheduler-k91

## Goal

Bound active workload exit polling at its existing absolute deadline: admit no exit observation at or after the deadline, reject even a true result returned at or after the deadline, and do not admit a new observation after an overslept poll. Ship this behavior through the actual `_call` termination path with retained controls.

## Context

This is a flat vertical slice from bounded-inspection-k28. The entire original aggregate charter and decision history are preserved in complete-worker-scheduler-k94. Read the ancestor briefs, docs/specs/diagnostic-process-inspection.md and the latest first-leader-reap-integrate-k93 evidence. No aggregate ancestor is complete. Persistent worker bootstrap, shared scheduling and removal of private waits remain k94's work.

## Done when

- `_wait` admits observations strictly before its original workload/interruption deadline and accepts an exit only when the observation returns strictly before it. A sleep overrun cannot admit another observation. Preserve OSError propagation and healthy early exit/polling.
- Retain deterministic entry/equality/overrun/sleep controls and real local child-process controls through `_call`, including normal deadline and interruption, mandatory group TERM/KILL, exact-child reap, independent post-run identity audit, sticky failure/unreached work and resource cleanup/disposal/absence.
- RED against frozen old controller, GREEN against frozen final source, separate admission/result/sleep guard mutation controls, per-case JUnit and before/after source/tool/runner inventories, Ruff and strict types. Preserve prior evidence byte-for-byte.
- Schedule review-impl of this slice before k94. Keep the aggregate review required by k94 separate and all aggregate ancestors open.

## Decisions (running log)

- 2026-11-15: Found `_wait` called `_exited` before checking its deadline and accepted true without checking return time. Delayed observers could therefore bypass timeout TERM/KILL. Narrow k91 to this useful runtime repair; copy the full remaining charter to flat sibling k94. The polling loop stays interim pending the shared scheduler. The observation-result bound is strict at equality, consistent with the existing inspection cutoff; the distinct exact-child first reap permits equality and is unchanged.

- 2026-11-15: Initial green-attempt controls incorrectly required group KILL for the PID-only interruption and `termination_ok=True` for real Darwin zombie group TERM EPERM. Preserve that failed sample unchanged; correct the controls to assert PID-only TERM/no widening after observed exit, mandatory group TERM/KILL otherwise, and sticky denied TERM. This is ordinary local zombie behavior, not the unavailable privileged UID fixture. The repair classifies a late result; it cannot preempt a synchronous NOTE_EXIT call or provide the missing shared scheduler.

- 2026-11-15: Delivered the strict active-wait admission/result guard in controller.py and twelve boundary/real-process controls. Report: experiments/exec-reliability/evidence/workload-wait-k91/report.md. Final focused 12/12 and full 481/481 pass; Ruff/format/strict Pyright pass. Frozen old controller has exact 8 failures; admission/result/sleep mutants have exact 4/4/2 failures. All sources/artifacts stable and final root inputs equal frozen copy; all 94,996 prior tracked evidence paths unchanged against 3ab5a2cebccfe83f5367628e319d6be1c483d80b. Preserve the original failed test-draft and relocated-suite samples without certification. Graph generation 2026-11-14T20:27:30Z has metadata_changed for edited files; material source read directly after coverage check. The full shared-scheduler charter remains in k94; this slice does not renew any completion claim for k28/k17/root.
