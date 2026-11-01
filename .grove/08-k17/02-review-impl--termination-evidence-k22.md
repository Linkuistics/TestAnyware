# termination-evidence-k22

**Reviews:** termination-evidence-k20

## Goal

Adversarially review the local controller's termination and process evidence
before retained-evidence-k21 or a causal consumer builds on it.

## Context

Read the producer commit identified by termination-evidence-k20 and k17's node
brief. Review `controller.py`, `peer.py`, `self_check.py`, `test_controller.py`
under `experiments/exec-reliability/` and the durable foundation report. The
delivered condition is `evidence/k20-final-03`; earlier conditions, including
failed red/green runs, are preserved. The final local audit is k20-audit-03.

## Done when

- Challenge monotonic leader-exit observation, conditional PID KILL and group
  escalation after leader exit. Determine whether every group operation occurs
  while identity is pinned, and whether any helper/library path can reap early.
- Inspect group membership/state parsing, signal-0 and delivered-signal EPERM
  classification, failed/partial inspection and the 10 ms sampling rule. Try to
  disprove any certainty obtained from an old, contradictory or failed snapshot.
- Verify grace plus kill/drain/reap totals, including ps timeout/creation and
  observation errors. Check measured overrun failures and that inspection does
  not earn a fresh reaping allowance or suppress safety actions.
- Check the corrected fork handler, opposed real-child scope cases, deliberate
  signal swaps and dropped-KILL control. Verify that the independent survivor
  test can fail, and that known children are inspected/cleaned even on failures.
- Verify final source hashes, per-test JUnit verdicts, raw receipts, checker
  identities and itemized resource audits. Challenge their absence oracle and
  exact-path ownership rather than crediting counts alone. Retain all original
  failed conditions and k16 historical audits.
- State findings without fixes. Create reconciliation before the first later
  live sibling only if findings warrant it. Do not require k21's deferred
  stream/result identities or k18's storage isolation as already implemented;
  challenge any current prose that mistakenly claims those guarantees.

## Notes

No guest, app quit, unsafe macOS process suite or acceptance measurement. Static
review may use the retained process fixtures; if executing new controls, freeze
a finite condition and bounds first, retain every failure and audit new local
resources. Normal completion and ps snapshots cannot prove arbitrary or escaped
descendant absence. This review supplies the fresh context; no competing
in-session reviewer is needed. The session had no GROVE_PROVIDER_EVIDENCE, so
there is intentionally no Creator line; do not invent provider provenance.
