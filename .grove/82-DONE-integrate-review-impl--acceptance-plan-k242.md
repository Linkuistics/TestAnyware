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

## Decisions (running log)

- The integrated review is commit `2b1ac16c`, over producer commit
  `b62275b4`. This session reads the source again for each finding. Each
  counterexample below reproduces from that source.
- R1 is a real issue. A single child snapshot right after the Host CLI dies
  can see a zombie inside the permitted 2 s grace and 2 s kill interval. The
  repair: `interrupt-child` polls the known child for 4 s at most, and the
  checker requires absence at the last poll. The early snapshots stay in the
  raw stdout. The fallback still runs after the poll, so an absent child
  still shows the cancellation of the Host CLI.
- R2 is a real issue. The inequality subtracts only `duration_ms`. The repair
  subtracts `duration_ms` plus `finalised_after_ms` from the earlier end of
  the two replies. The reply latency after publication stays a visible limit.
- R3 is a real issue. The trial does not compare the answering agent or app
  with the freeze. The repair: the manifest names the agent digest of each
  OS and the app build and executable. The checker compares the `identity`
  output and a new `app-identity` call, which runs `doctor --json` through a
  helper because `doctor` can exit 1 on a private build.
- R4 is a real issue. The repair: the trial checks the frozen digests again
  before each clone. The checker compares the subjects before and after each
  run with the frozen digests, and its own modules with the run's subjects.
  The plan adds `completion_k186_check.py` as a subject. The manifest must
  pin the CLI, Tart and the app executable.
- R5 is a real issue. `vm start` without `--wait` never waits for a slot. A
  full slot fails at once with `VM_LIMIT_REACHED`. The repair: the boot bound
  alone limits the acquisition. The manifest still declares the slot bound,
  and the plan records that no clone exercises it.
- R6 is a real issue with a small risk. The repair checks the payload keys and
  their integer types as the Host CLI emits them.
- R7 is a real issue. The repair: the recorded call names must equal the plan
  order. Each receipt argv must equal its planned argv. The start times must
  not decrease.
- The count note is a contract stated unclearly. The repair takes the stricter
  reading: each after block has ten health and observation operations, and
  four child checks besides them.
- Verification: `self-check-01` of this leaf passed, with 1,236 tests and equal
  subject digests. Each reverted repair fails its controls. The controller and
  its disposal path did not change, so it still disposes only its owned VM.
  The plan changed, so k224 freezes the revised plan, and k225 runs the full
  trial under that freeze. The preparation leaf now names the manifest fields.
