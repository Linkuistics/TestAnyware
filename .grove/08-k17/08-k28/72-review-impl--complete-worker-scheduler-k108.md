# complete-worker-scheduler-k108

**Reviews:** complete-worker-scheduler-k104

## Goal

Independently challenge k104's declared worker-failure/orderly-close bounds, fixed retirement timeline and truthful phase receipts. Read the committed artifact and retained conditions; do not infer aggregate acceptance from this slice.

## Context

Read the k28 brief chain and docs/specs/diagnostic-process-inspection.md, producer complete-worker-scheduler-k104, experiments/exec-reliability/evidence/worker-phases-k104/report.md, condition-audit.json and before/after prior inventories. Runtime scope is experiments/exec-reliability/{worker_capture,controller}.py; controls are test_shutdown.py and updated close controls in test_worker_capture.py. Compare k98/k102/k103 close evidence and k101/k105/k106 bootstrap evidence, especially k106 F5's missing prior finite-cutoff/denied-attempt demonstration. k107 retains the full original aggregate charter and must read this review before relying on the new declarations.

## Done when

- Verify Plan and direct Worker validation reject booleans/non-numeric/nonpositive/nonfinite/over-ceiling values and wrong declaration objects before hashing/output/admission/fork. Verify declarations propagate and are frozen in Plan/worker evidence.
- Trace all close modes: healthy EOF grace expires directly to KILL/reap; failure has TERM grace; unready exact PID ownership; command-close failure stays on the original clock; prior TERM/KILL and denial history suppress repeated attempts; every finite cutoff wins without renewal. Check bootstrap compatibility and idempotence.
- Independently inspect absolute phase starts/deadlines/outcomes, strict overrun/equality exhaustion/lateness and sticky failure. Late wait return with no attempted signal must not claim entered KILL/reap. Prior KILL has no newly entered grace and retains the actual attempted timestamp. Residual ownership is not absence, even after checker cleanup.
- Audit frozen RED, failed green, two passing full conditions, strengthened three-case receipt RED and final five single-site mutants. Final full has 679 passes plus lint/format/strict typing; mutants fail 6/2/4/3/2 of 53 controls. Check raw streams/JUnit, checker receipts, artifact hashes and exact-child audit logic, including simulated deferred reap/late return and seeded denials. Do not relabel a failed real observation as absence or genuine UID EPERM. Prior 151,643 evidence paths must remain identical. Preserve every failed attempt; use fresh bounded evidence only for a concrete review question.
- Disclose verification limits. This is still synchronous close: shared earliest-deadline scheduling, teardown overlap, first reap after expired cutoff and final k28 aggregate review remain in k107. Ten-clone acceptance remains unavailable 0/10, storage k18, retained-file identity k21. Actual setuid ps KILL timing and genuine UID EPERM remain unavailable without separately authorized fixture ownership. Do not acquire privilege or touch guests/app/install/unrelated processes.
- Record findings or a bounded no-defect conclusion. Cut integrate-review-impl before k107 only when findings need integration; keep k28/k17 open.

## Decisions (running log)

