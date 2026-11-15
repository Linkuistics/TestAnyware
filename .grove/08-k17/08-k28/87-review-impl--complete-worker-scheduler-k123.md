# complete-worker-scheduler-k123

**Reviews:** complete-worker-scheduler-k119

## Goal

Adversarially review the incremental workload-leader reap boundary delivered by
k119 against its narrowed contract. Report defects or evidence gaps that matter
to exact child ownership, immutable cutoff behavior and future scheduler use.

## Context

Read the committed k119 producer task, controller.py `_LeaderReap` and
`_reap_leader`, the focused controller controls, and
experiments/exec-reliability/evidence/leader-turns-k119/report.md with its retained
conditions, tooling revisions, declarations and audit. Historical first-reap
semantics are in leader-reap-k93, workload-wait-k91 and worker-first-reap-k107;
termination-review-k118 supplies the adjacent nonblocking termination seam.
This reviews the bounded reap artifact, not the aggregate scheduler. The entire
original charter remains live in complete-worker-scheduler-k122.

## Done when

- Check the at-most-one exact positive-PID WNOHANG turn, no sleeping within a
  turn, absolute wake/cutoff, mandatory first attempt at/past cutoff, later
  deadline admission, terminal idempotence and compatibility consumer against
  the committed source and tests. Check return-code/strong-owner/residual
  behavior and closure of signal/probe authority.
- Independently challenge whether the RED/mutation and real-child controls
  detect relevant regressions, and whether measured subject identities, tooling
  revisions, expected failure sets and child/fixture absence support the report.
  Do not infer a timing guarantee from one full passing sample or cancel any
  retained historical failure. Check the flat split preserved all aggregate
  requirements with k122 as owner.
- Retain any verification with explicit subjects and limits. Findings must be
  actionable against this contract; visible trade-offs and unproved claims
  remain distinct from demonstrated runtime defects. If findings earn an
  integration, insert it before the next live sibling using Grove's review
  procedure; otherwise retire only this review. Ancestors and k122 stay open.

## Notes

This ownership state machine is a load-bearing seam for the eventual shared
scheduler. The specific doubt is whether a turn can violate wait ownership or
its unchanged cutoff when interleaved with other work, and whether the finite
controls distinguish that from an admitted first-attempt exception. This review
must supply fresh context before k122 changes the same boundary. No current
GROVE_PROVIDER_EVIDENCE was present on the finishing producer; no Creator line
was invented.
