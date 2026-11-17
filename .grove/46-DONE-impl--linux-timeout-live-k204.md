# linux-timeout-live-k204


## Goal

Run the Linux timeout controls and the retained fixture on one fresh unchanged
clone. Observe each recorded child after its timeout, and measure the
termination phases on the real public path.

## Context

- This leaf is the second slice of `linux-timeout-lifecycle-k5`. That leaf is
  too large for one session. It keeps the deterministic seams: the process seam
  in a Linux container, the HTTP seam and the real Host CLI seam. This leaf
  keeps the live clone, because it needs a VM and a separate frozen plan.
- Read `docs/experiments/exec-reliability-linux-timeout-lifecycle.md` first.
  It records the source hash comparison, the outcome matrix of the seams and
  the regression candidates. Reuse its cases and checker where they apply.
- Issue #14 failure B is in the retained run
  `Ravel-Strategies.gh-issue-1/experiments/tutorial-vm-reliability/evidence/diagnosis-linux-01/`.
  The original fixture is
  `Ravel-Strategies.gh-issue-1/experiments/tutorial-vm-fit/fixture/r5/guest.sh`,
  with `cases.json` and `frozen.json` beside it.
- The macOS live runs show the pattern for a frozen clone plan: leaf k4
  (`timeout_k4.py`, `timeout_k4_check.py`) and leaf k193.

## Done when

- On one fresh unchanged clone of `testanyware-golden-linux-24.04`, freeze and
  run the exact `sleep 120` and fork/wait controls with timeout 2 s. Run the
  retained complete fixture as its own case. Capture raw agent and real Host
  CLI receipts, host and guest status, partial bytes and the known shell and
  child identities. Preserve every failed sample.
- Observe each recorded child independently after its timeout. A failed
  observation is unknown. Bound host calls by the guest budget plus 20 s.
  Bound ordinary observations by 30 s and guest cleanup by 30 s. Bound
  independent disposal by 45 s. Compare the actual termination phases with a
  grace of at most 2 s. Compare kill, drain and reap with at most 2 s. Record
  each violation.
- Use at most one instrumented clone, and only when the seams of k5 cannot
  settle a question.
- Add a verified Taskfile recipe for the live run. Append the per-case raw
  evidence, the causal limits and the full resource audit to the k5 report.
  Cleanup targets only positively identified owned processes and groups.

## Notes

The placement is a flat sibling, because this work shares the purpose and the
context of k5 and needs only another session. A timeout field cannot establish
cleanup, and a cooperative fixture cannot prove arbitrary descendant
containment. Client disconnect belongs to `linux-disconnect-availability-k6`.
Explicit cancellation and the production lifecycle are design work.
