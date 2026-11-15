# complete-worker-scheduler-k120

**Reviews:** complete-worker-scheduler-k116

## Goal

Independently challenge the completed k116 fork-failure EOF-order artifact
against its scoped criteria and the inherited process-inspection contract.
Derive the producer's commit from its stable handle and assess the artifact
from that boundary.

## Context

Read the producer task and its committed diff, the parent bounded-inspection
brief, docs/specs/diagnostic-process-inspection.md, and the F4 requirement in
experiments/exec-reliability/evidence/termination-review-k118/report.md.
The artifact is experiments/exec-reliability/evidence/fork-channel-k116/;
report.md and comparison-plan.json locate the measured inputs and outputs.
Changes also touch worker_capture.py, test_worker_capture.py, root Taskfile.yml
and docs/experiments/exec-reliability-diagnostic-foundation.md.

The entire inherited aggregate remains in complete-worker-scheduler-k119,
sequenced after this review. Check the split preserves its requirements; this
producer does not claim shared-scheduler completion. The first parent full run
also retains a named deferred-reap timing failure. Its disposition and limits
must be assessed from the actual evidence, without treating later passing
samples as a repair.

## Done when

- Independently assess whether recording cleanup EOF preserves no-child
  authority only for timely, valid, uncontradicted fork failure with both
  channels closed. Challenge late, partial, trailing, contradictory and
  open-channel paths, cutoff handling, read caps and ownership semantics.
- Assess the deterministic test's relation to real transport and reaping,
  whether its ordering adapter can conceal a failure, negative control
  adequacy and the strength of the mutation rejection. Separate injected
  faults from real observations and unexplained historical symptoms.
- Check the finite full-suite comparison, actual changed bytes, source/tool
  provenance, immutable earlier evidence, exact fixture cleanup outcomes,
  preserved failures and the report's causal and repeatability limits.
- Record findings with concrete source/evidence anchors, severity and reasons;
  distinguish demonstrated failures from uncertainties. Apply the review
  discipline's ordinary checks and bound any new experiments to owned local
  exact-identity fixtures with fresh evidence paths. Do not modify old samples,
  use guests or installed applications, or signal unrelated processes.
- Cut an integration leaf only if actionable findings warrant one, using this
  review's handle as the handoff. Keep k119 and aggregate acceptance live.

## Notes

This is an independent review of the committed producer, not a continuation of
its reasoning. Its body makes no correctness conclusion for the reviewer.

## Decisions (running log)

- 2026-11-15: Scope. Inspected commit 5b045314 against its parent. The
  production change is exactly two lines at
  `experiments/exec-reliability/worker_capture.py:1311-1312`. Blob hashes match
  the report: parent 726ad21c…, candidate 535fccf7…. The new test is
  `test_worker_capture.py:1297`. Controller pipe order (control, result) holds at
  `worker_capture.py:1114-1119`, so the test's `pairs[0]`/`pairs[1]` mapping is
  right. A wrong mapping would time out, not pass.
- 2026-11-15: F1 (medium, demonstrated test gap). No control isolates "control
  channel still open, no trailing bytes". I ran three mutants in a scratch copy
  (`/private/tmp/claude-501/.../scratchpad/k120-mut`, production tree untouched)
  against `-k "fork_failure or fork_eof"`, 37 cases: M1 `closed_channels == {0, 1}`
  to `1 in closed_channels`; M3 record EOF on `BlockingIOError` at
  `worker_capture.py:1304`. Both pass 37/37. Probe: failure published, result
  frame dropped, an owned descendant holds only the control write end for 1.5 s,
  deadline 0.5 s. Production: unknown creation, residual remedy, errno None
  (correct). M1 and M3: not unknown, no remedy, errno 35 (wrongly confirmed).
  The `open-tail` control cannot catch this because its partial bytes already
  force `creation_conflict`. Production is correct; the controls are weak.
  Reaching the case for real needs a descendant that outlives the worker.
- 2026-11-15: F2 (low, uncertain). Mutant M2, dropping
  `if chunk and fork_failure is not None` at `worker_capture.py:1315-1316`,
  also passes 37/37. The line looks redundant with `worker_capture.py:1322`,
  because chunks are buffered first. It could differ only beyond the 4096-byte
  cap. Not shown to be a defect; may be an equivalent mutant.
- 2026-11-15: F3 (low, unclear contract). Control EOF recorded after
  `query_deadline` but before `deadline` (reserve at most 50 ms,
  `worker_capture.py:945-946`) still confirms a failure admitted on time. The
  spec ties "timely" to the failure result. No control exercises a late EOF.
- 2026-11-15: Evidence checks that held. The shutdown failure receipt
  `parent-full-1/runs/shutdown-blocking-deferred-reap-1794716286099350000.json`
  has kill deadline 333565.29822554096 and last probe 333565.303169583, so 4.944 ms
  late. Its runtime outcome is `residual_ownership` with shutdown lateness
  5.497 ms. The report's disposition (open, left to k119, no causal claim) matches
  the data. `condition-audit.json` records `ok: true` with 181,726 prior paths. I did
  not re-run that audit. Limits stand: two pairs, an auditor extended after the
  last full run (disclosed), and no proof of which EOF order occurred historically.
  `inputs-parent/` retains `__pycache__` bytecode (cosmetic).
- 2026-11-15: Split check. k119 keeps its full aggregate charter and its leaf
  text; the report's "Remaining scope" repeats that. Not an issue.
- 2026-11-15: Cut k121 (integrate-review-impl) for F1, ahead of k119. First attempt
  used `leaf-insert … 85`, which resolved as permanent key k85 and renumbered 34
  siblings. I reverted it with `jj restore --from @- .grove` and re-inserted
  with key 119. Only the tree rename was involved; no content was lost.
