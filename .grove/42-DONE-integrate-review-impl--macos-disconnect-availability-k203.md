# macos-disconnect-availability-k203

## Goal

Integrate review `macos-disconnect-availability-k202` before
`lifecycle-evidence-k8` consumes the reviewed artifact.

## Context

Read the review's findings from its commit and verify each against the producer
artifact. Apply the integration acceptance and material-risk threshold; the
review's list is input for triage, not a predetermined repair charter.

This is a sibling in the existing review chain. Its immediate placement keeps
the reviewed source and evidence stable until triage and any necessary repairs
are complete, directly unblocking the downstream lifecycle evidence decision.

## Done when

- Each finding has a recorded disposition against the agreed acceptance.
- Accepted repairs and their appropriate verification are complete, or any
  necessary finite continuation is explicitly owned under the Grove rules.
- The downstream consumer has an accurate account of the resulting artifact and
  retained evidence.

## Notes

The root brief's diagnostic and evidence boundaries continue to apply.

## Decisions (running log)

- **Finding 1 (P1, cleanup kills unowned processes): real issue, repaired.**
  `audit` and `sweep` killed any process of this user whose command held a run
  token or the build directory. That breaks the root rule to clean up only
  positively identified owned processes. The new `owned()` in
  `disconnect_k13_seam.py` matches only the shapes that the run creates: the
  exact `sleep TOKEN`, the `/bin/bash -c` payload shell and a binary inside the
  build directory. Both tools use it. A new test covers owned and unowned
  commands. The matcher owns all 14 shells and sleeps recorded in `host-01`. The
  retained sweeps, final sweep and audit were empty, so the retained run killed
  no process. The checker did not change, so its retained result stays valid. A
  new host run uses changed tools and is a new condition.
- **Finding 2 (P2, host polling gap): real limit, accepted with a report
  correction.** The host seam polls only after each follow-up returns, so oracle
  condition 2 cannot fail in host mode. The retained verdicts do not depend on
  it: each HTTP follow-up answered in at most 0.073 s, and each handler ended
  after 20 s. The report now states the gap and says latency carries the host
  verdict. No tool change, because a repaired seam needs a new run to give any
  new evidence.
- **Finding 3 (P2, live PID reuse): real limit, accepted with a report
  correction.** The inspector records only the PID. Reuse can cause only a false
  `present` reading. Both retained late inspections read `absent`, so no retained
  verdict depends on PID identity. The report now says that a later present late
  reading must first verify the process identity.
- **Finding 4 (P2, late absence overclaim): real issue, repaired in the report.**
  The live `deadline` label has no upper limit on the bracket end, and the live
  `ended-with-handler` label means only absent at 30.0 s. The report no longer
  says that both live children ended with the handler. It says that they ended
  between 18.01 s and 30.12 s, and that host children ended within 0.07 s of
  their shells. The checker labels stay as recorded, because the root brief
  treats a changed tool as a new condition.
- **Test results.** `mise exec -- task diagnostic:macos-disconnect-availability-tools`
  passed: ruff, format, pyright, shellcheck and 34 tests. The report passes
  `ste-lint.py` with no hard finding.
- **Account for `lifecycle-evidence-k8`.** The retained evidence in
  `experiments/exec-reliability/evidence/macos-disconnect-availability-k13/` is
  unchanged. The report claims now match the receipts. The agent stayed
  responsive after a disconnect. The loopback agent shows that the deadline ends
  the shell and its child. The live run puts each child end only in
  (18.01 s, 30.12 s]. The host and
  seam tools now differ from the tools of the retained run. No continuation is
  needed.
