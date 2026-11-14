# bounded-helper-capture-k49

**Reviews:** bounded-helper-capture-k47

## Goal
Audit the actual post-creation helper capture/reap path and its integration
before worker-scheduler-k48 changes the ownership boundary.

## Context
Read the producer's committed diff, the parent contracts and
`docs/specs/diagnostic-process-inspection.md`. The implemented boundary is
`experiments/exec-reliability/inspection.py` through the actual group query in
`experiments/exec-reliability/controller.py`, with controls in both test modules.
The measured report is `docs/experiments/exec-reliability-diagnostic-foundation.md`;
retained conditions are `experiments/exec-reliability/evidence/k47-check-01`,
`experiments/exec-reliability/evidence/k47-final-01` and the independent
`experiments/exec-reliability/evidence/k47-audit-01`.

## Done when
- Inspect all helper ownership, cleanup and exception paths, including Popen
  construction, destructor behavior, waitpid, pipe closure, helper KILL and
  retained residual owners. Test whether identity release always closes signal
  authority and whether unexpected failures could lose ownership or wait forever.
- Contest the fixed capture/reserved-reap deadlines, combined output cap,
  per-turn work, stderr/EOF handling and late-creation behavior against the
  actual executable path. Distinguish the bounded post-creation guarantee from
  synchronous creation and controller-local waiting still assigned to k48.
- Check sticky failure and safety continuation, group KILL ordering, strict
  table validation, sole final observation and nonblocking workload reap for
  regressions. Evaluate the real controls and the separately labelled simulated
  unreapability fixture; do not credit them as privileged denial/setuid-ps tests.
- Audit each condition's subjects, bounds, raw receipts and exact resource
  reconciliation. Preserve the initially failed lint condition. Check report
  claims against actual receipts and ensure the full parent worker/scheduler,
  concurrency and final bounded-inspection-k30 review obligations remain live.
- Record actionable findings with committed file/line evidence. Cut integration
  before worker-scheduler-k48 only if findings warrant it; no empty integration.

## Notes
No in-session reviewer ran. The launch supplied no current provider evidence;
the retirement procedure therefore leaves the Creator declaration absent.
