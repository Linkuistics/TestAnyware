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

## Decisions (running log)

- Review of commit f04e6f481c41 (k119) by reading only. No test, build or lint command ran.
  Subjects: experiments/exec-reliability/controller.py:207-285, test_controller.py:421-667,
  evidence/leader-turns-k119/ (report.md, audit.py, condition-audit.json, pytest.xml, inputs-*).
- Contract holds. `_LeaderReap.advance` makes at most one positive-PID `os.waitpid(WNOHANG)` per
  turn and never sleeps in a turn. The wake is `min(deadline, observed+0.005)`. After the first
  attempt, a turn at or past the cutoff retains the residual and does not poll. Terminal turns
  return None. The strong owner leaves `_workload_owners` only at the reap (controller.py:267).
- Doubt answered. test_leader_reap_turn_yields_and_stops_at_fixed_cutoff moves the fake clock
  between turns (100.006) and asserts the cutoff does not move. The late-return and withheld cases
  separate the admitted first attempt from a later poll. The nine cases at test_controller.py:492
  pin the first-attempt exception through `_reap_leader`. Both pass at green.
- Evidence checks. inputs-final equals the committed sources. The mutants differ by one site each.
  RED ran the new tests on the old controller and fails with AttributeError for `_LeaderReap`.
  Every condition reports four leader-turn children absent.
- Trade-off, accepted: RED proves the missing symbol, not a behavioural regression. The old inline
  loop behaved the same, so only the first-attempt and caller-authority controls guard behaviour.
- Unproved claim, accepted: the two retained mutants cover the later-poll guard and sleep in turn.
  No retained mutant removes `first_attempt = False`, the `min(deadline, ...)` clamp, the
  `_workload_owners` deletion or the error residual. Nothing shows these controls miss such a
  change. k122 can add mutants when it changes the same boundary.
- Documentation nits, no integration: observed-deviations.json is `[]` while report.md records two
  tooling corrections. docs/experiments/exec-reliability-diagnostic-foundation.md:2025 and older
  records still name k119 as aggregate owner. k122 owns the aggregate and its documents.
- No actionable defect against the contract. No integrate-review-impl leaf is cut. Only this
  review retires. k17, k28 and k122 stay open.
