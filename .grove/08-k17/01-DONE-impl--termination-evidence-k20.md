# termination-evidence-k20

## Goal

Make termination receipts distinguish leader exit, signal scope, group members,
inspection uncertainty and signal permission failures without releasing identity
before the last signal.

## Done when

- Observe leader exit monotonically during TERM; PID KILL is conditional.
- Inspect the group while the leader is unreaped, preserve raw bounded ps
  observations, and distinguish live members, zombie-only EPERM, genuine
  permission failure and unknown inspection. Never group-signal after reaping.
- Fix the peer's reentrant wait-lock handler; prove graceful group completion
  and opposed PID/group behavior with real owned child fixtures.
- Replace the tautological survival assertion with a child-controller result
  observed by an independent parent, and show wrong-scope mutations fail.
- Retain fresh checker logs, per-test verdicts, source hashes and an itemized
  audit of every new local PID and fake marker. Preserve historical receipts.
- Update the handoff and cut a specific review before retained-evidence-k21.

## Implementation plan

Use the existing Python 3.12 process boundary and Taskfile runner. Keep kqueue
exit observation without wait/poll, and take bounded `/bin/ps` group snapshots
while the child leader pins its PGID. Spend TERM grace and KILL/drain/reap within
their existing shared phase deadlines, with no fresh reap allowance. Unknown
inspection fails conservatively. Snapshots describe their observation interval;
they cannot prove arbitrary or escaped descendant absence.

1. Add tests for graceful PID exit (no KILL), graceful fork reaping, real opposed
   PID/group scope, and a separately observed controller survivor. Run the
   existing review-controls recipe against the old controller and retain red.
2. Correct the signal handler to set a flag; wait/reap only in the main loop.
   Add typed group snapshot and termination helpers, preserve raw observation
   and signal error classifications in call receipts. Run all existing tests.
3. Add deterministic inspection/permission controls and scope mutations. Run
   the full self-check with frozen source subjects, then audit all new runs.
4. Update the durable report, insert review ahead of k21, retire and seal one
   focused jj commit. No guests, apps or unsafe process suite.

## Decisions (running log)

- The mandated k17 scope exceeds one focused implementation: process evidence
  is k20; independent retained-file evidence is k21. This session executes only
  the first child under Grove's decomposition procedure.
- Tier 2 graph generation 2026-11-01T04:23:42Z reports no recorded coverage gaps
  in the controller, peer, tests and runner. Its `_call` trace includes unrelated
  Rust methods; exact Python source supplies the actual subprocess relationships.
  The handoff's changed metadata was resolved by reading the source directly.
- Reuse the existing macOS controller and kqueue pinning rather than add a new
  process backend. Group probes and snapshots finish before reap; PID-only
  interruption never escalates to a group signal. Storage stalls remain k18.
- `k20-red-01` retains the old controller's unconditional KILL and fork-handler
  deadlock failures (2 fail, 16 pass, 13 deselected). Corrected handlers only
  set flags, then wait/reap from the main loop.
- `k20-green-01` passes 31 tests and strict types but fails one test-source
  line-length check. `k20-green-02` passes 36/37 tests: its failed pinning
  control exposed a ps launch with only 0.7 ms remaining, not a released group.
  Preserve that timeout as a failed condition. Reserve a 10 ms sampling window
  within each existing phase; continue kqueue polling to the phase deadline.
- `k20-green-03` passes all four checkers and 39 tests, including opposed real
  child outcomes, swapped signal primitives, live permission denial, zombie
  EPERM, failed inspection and dropped-KILL survival. Scheduled tree review
  owns the adversarial read; no in-session reviewer is added beside it.
- Move the independently observed nested controller into peer.py so its exact
  source survives pytest scratch cleanup. Record checker/runner PIDs in new
  runner receipts. `k20-final-01` passes all four checks and 39 tests.
- `k20-audit-01` checks 276 recorded PIDs, 33 known-child references, 36 fake
  resource entries and 79 exact scratch directories; all are absent afterward.
  It removes only known test outputs and validates the absence oracle against
  a live process and ps usage error. Original receipts and audit inputs match
  before/after. k16's historical audit is not rerun.
- Cap KILL/reap at the original TERM deadline plus kill_wait even if observation
  runs late; fail the call on measured reap overrun. A fresh final condition
  tests this bound rather than reinterpret final-01. No ceilings are increased.
- `k20-final-02` passes 39/39 and all four checks. Final-03 additionally hashes
  the fork's `/bin/sleep` dependency as a declared subject and passes the same
  39/39 and four checks. Final-03 is the delivered source condition.
- Final audit `k20-audit-03` covers all seven conditions: 414 recorded PIDs,
  49 known-child references, 52 fake-resource entries and 115 exact scratch
  directories are absent. Original evidence and each audit's before/after
  input hashes agree. Inspection helpers are reaped by subprocess.run; the
  evidence/venv directories are intentional retained artifacts. No live VM,
  image, archive or app was acquired. No new ADR meets the hard-to-reverse test.
- Final verification confirms delivered source hashes, all seven retained
  condition artifact manifests and all three audit input manifests. Historical
  evidence paths are unchanged in jj. Review termination-evidence-k22 is placed
  before retained-evidence-k21; it owns the fresh-context challenge under Grove
  and doubt-driven-development. No current provider evidence was supplied, so
  retirement must omit Creator rather than fabricate it. k17 remains open.
