# complete-worker-scheduler-k84

**Reviews:** complete-worker-scheduler-k81

## Goal

Independently contest the committed signal-0 probe timing slice. Derive the
contract from the producer task, ancestor briefs and inspection spec; read the
actual source and retained evidence rather than accepting the producer report
as a verdict. This review owns the independent adversarial assessment.

## Context

The producer adds nullable probe_started_monotonic/probe_finished_monotonic to
GroupObservation around killpg(pid, 0), including operating-system errors.
Existing helper.worker.published_times and outer observation timing retain their
meanings. Read the committed producer diff and
experiments/exec-reliability/evidence/probe-intervals-k81/report.md, frozen inputs,
condition declarations, JUnit, raw receipts and deterministic audit. The full
original persistent-worker/shared-scheduler charter remains in
complete-worker-scheduler-k83; k28/k17 remain open.

Inspect at least these doubts:

- Do attempted probes timestamp the actual call on success and OSError, before
  error classification, with null/null on every skipped observation path? Can
  deadline exhaustion leave misleading timestamps, state or permission outcome?
- Does the narrow change preserve the existing deadline, parsing, ownership,
  signaling and sticky-failure behavior? Are query capture, worker receipt,
  controller cleanup and probe intervals described without an atomic-snapshot
  claim or treating injected EPERM as real privilege evidence?
- Do the deterministic controls distinguish the helper interval from the probe,
  and do failed/skipped/late cases expose a misplaced timestamp? Does the real
  receipt prove the ordering and exact helper/worker/fixture reap claims made,
  rather than inferring them from later PID absence?
- Can ten RED failures or the GREEN/full results be explained by an unrelated
  setup failure, changed input or different case identities? Contest the
  auditor's subject coverage, artifact hashes, frozen sources and historical
  evidence comparison, including the symlink inventory and uncredited setup
  failures. Check that large raw artifacts actually landed in the commit.
- Is the flat split independently useful, and does k83 preserve every original
  aggregate requirement, control and final review obligation without treating
  this timing slice as scheduler completion?

## Done when

The committed artifact has an independent assessment with concrete path/line
evidence, appropriate validation and explicit limits. No guests, installed app
mutation or privilege is authorized; ten-clone acceptance stays 0/10 unavailable.
If findings earn integration, insert integrate-review-impl using this producer's
bare stem before the next live sibling complete-worker-scheduler-k83, referring
to this review's handle without transcribing findings. Otherwise retire without
an empty integration. The eventual aggregate runtime review with
Reviews: bounded-inspection-k28 before retained-evidence-k21 remains the final
aggregate producer's responsibility.
