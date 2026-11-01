# fork-admission-k29


## Goal

Reject unsafe controller fork environments through `execute` before any resource
acquisition or workload, with an immutable failed receipt and all calls unreached.

## Context

First independently useful child of bounded-inspection-k28. Follow its reviewed
spec and preserve earlier evidence. Own the narrow `inspection.py` admission
module, controller integration, admission tests, test/type selection and Taskfile
recipes, new retained checks and the foundation report. No worker is delivered here.

## Done when

- Check both Python and native macOS thread counts and default SIGCHLD. Reject
  unavailable or malformed native observations without acquiring a resource.
- Real extra Python thread, native pthread invisible to Python, SIG_IGN and
  installed SIGCHLD handler controls exercise `execute`. Healthy calls still run.
  Retain rejection receipts, exact unreached operations and source hashes.
- The diagnostic suite selects the new tests and types the new runtime module;
  reusable Taskfile checks pass with new frozen evidence. Earlier evidence stays intact.
- Report the limited delivered boundary and retain full worker/scheduler scope
  in bounded-inspection-k30. Cut a specific review-impl before that consumer,
  naming `**Reviews:** fork-admission-k29`, before retirement and jj sealing.

## Decisions (running log)

- Decompose k28 because the unfinished worker plus scheduler and control inventory
  exceed one focused session. This slice has a direct execute-boundary effect;
  it does not establish inspection-stall repair or fork safety between checks.
- Replace the unintegrated worker draft with the minimal admission module and
  tests. Snapshot `0f8aca80` preserves that draft in jj history; it is not delivered
  code. k30 must implement its full charter rather than assume the draft is sound.
- Native admission uses libproc PROC_PIDTASKINFO and rejects any failed, short,
  or invalid thread-count observation. Check again before each actual worker/helper
  fork in k30; this initial observation cannot close an embedding-host thread race.

## Notes

- Admission now records Python/native counts, SIGCHLD disposition and native
  observation errors in every result. Rejection precedes the intent ledger and
  keeps acquisition/workload unreached. Hash inspection.py with the controller.
- Controls first failed against the old execute boundary (12 failures), then
  passed after integration. A missing native symbol exposed an escaped
  AttributeError; normalize that unavailable observation to ENOSYS and retain it
  in the same failed result. Native short/error/invalid-count cases are explicit
  adapter faults; real Python/pthread/signal controls remain separate.

- Frozen k29-final-01: lint/format/types/pytest all exit zero, 67 cases including
  13 admission cases pass, declared source hashes unchanged. Twelve rejected
  controller runs remain failed with no intent/acquisition/workload.
- Independent k29-audit-01 stopped on readiness text mistaken for a PID before
  controls/cleanup; preserve it. Corrected frozen k29-audit-02 observes all 94
  recorded PIDs absent, checks 21 resource declarations, cleans 67 exact fixture
  directories and preserves preflight scratch evidence; before/after inputs equal.
  The blocked-disposal marker is explicitly cleaned; original uncertain receipts
  remain uncertain. No escaped-descendant or bounded ps-wait guarantee follows.

- Final verification: all 460 frozen condition artifact digests and both successful
  condition/audit input snapshots match current files; report links resolve.
  jj confirms no pre-k29 evidence changes. No runtime source changed after the
  frozen checks. No parent closes: k30 and this slice's review remain live.
