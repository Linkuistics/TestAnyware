# acceptance-plan-k242

**Integrates:** acceptance-plan-k241


## Goal

Independently triage the acceptance plan review and reconcile the accepted
findings before acceptance-preparation-k224 freezes the trial.



## Context

- Read the findings from the commit of acceptance-plan-k241. Its durable
  report is `docs/experiments/exec-reliability-acceptance-plan-review.md`.
- Producer: acceptance-plan-k223 and its report
  `docs/experiments/exec-reliability-acceptance-plan.md`.
- Root brief: "Done when", "Constraints" and "Acceptance inventory and bounds".
- Spec: `docs/specs/guest-command-lifecycle.md`.

## Done when

- Every finding is verified against the source and classified against the
  agreed acceptance and material-risk threshold, with its disposition recorded.
- Accepted repairs and their relevant verification are complete. Any remaining
  limits are explicit in the durable acceptance report and preparation handoff.
- The independent controller still disposes only its owned resources, and the
  full trial remains for lifecycle-acceptance-k225 under a subsequent freeze.

## Notes

Read the review's commit rather than treating this task as a restatement of
its findings. Do not boot trial guests or patch the global CLI or app to
obtain a local pass. A changed trial candidate needs a new freeze.
