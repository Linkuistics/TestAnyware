# fork-admission-k31

**Reviews:** fork-admission-k29

## Goal

Adversarially review the delivered initial fork-admission boundary and its
evidence before bounded-inspection-k30 consumes it.

## Context

Read the producer, parent charter, inspection spec and the k29 section of
docs/experiments/exec-reliability-diagnostic-foundation.md. Review the committed
artifact, not the unintegrated worker snapshot. Scope is the admission module,
execute integration, admission controls, test/type selection, Taskfile recipes,
retained condition and exact-resource audits. The worker and shared scheduler
remain undelivered in k30; do not mistake this slice for their completion.

## Done when

- Trace execute through validation, before hashes, admission, intent/acquisition,
  workload, safety cleanup and final receipt. Prove rejection cannot acquire or
  run a command and remains a failed result with every call unreached. Check
  source-module hashing and unchanged behavior for admitted callers.
- Verify the libproc ctypes ABI and error handling against official source;
  challenge unavailable library/symbol, errno, wrong-length and invalid-count
  paths. Inspect the real extra Python thread, native pthread invisible to the
  Python registry, SIGCHLD handler/ignore and healthy controls. Check restoration
  and thread cleanup, adapters versus real conditions, and native callback
  lifetime/bounds. The observation is a snapshot: assess whether prose keeps
  fork races, native signal state and before-every-fork work explicit.
- Read k29-final-01 raw check output, JUnit, per-case results and pinned
  source/interpreter/manifest/lock/Taskfile/artifact hashes. All 67 cases passed,
  including 13 admission cases; 12 rejected controller runs must remain failures.
  Check the runner actually selects the new controls and types the module.
- Read both k29-audit scripts and raw evidence. Audit 01 failed before controls
  or cleanup; audit 02 records 94 exact PIDs absent, 21 marker declarations and
  67 exact fixture directories cleaned, including the deliberately blocked
  marker. Challenge the live/mixed/invalid/reaped oracle, PID/path provenance,
  numeric readiness-marker parsing, simulated-PID exclusion, preflight archive,
  exact owned cleanup and input hashes. Later absence cannot prove no escape or
  bounded inspection at workload time. Do not replace failed/uncertain receipts
  with audit success.
- Confirm earlier evidence is untouched and the report retains 0/10 unavailable
  acceptance, the synchronous ps/wait limitation, open k17/k18/k21, and full k30
  worker/scheduler/control obligations. No live guest or privilege claim.
- Produce actionable findings against the producer commit. Insert an
  integrate-impl fork-admission step before k30 only if findings require work;
  do not create an empty integration leaf.

## Notes

Frozen evidence:
experiments/exec-reliability/evidence/k29-final-01/verdict.json;
experiments/exec-reliability/evidence/k29-audit-01/failure.json;
experiments/exec-reliability/evidence/k29-audit-02/result.json.
Transient development red runs were not retained as a frozen countercondition;
the failed admission results and first failed audit are retained separately.
