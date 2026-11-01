# termination-evidence-k22

**Reviews:** termination-evidence-k20

<!-- tc: off because the producer wrote this charter and the review keeps it verbatim -->

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

<!-- tc: on -->

## Findings

The review read producer commit `6b9766f6`, the four named sources, the handoff
report and the retained `k20-*` evidence. It ran no test, build or checker. It
read retained receipts and hashed delivered files only. Line numbers refer to
`experiments/exec-reliability/` at `6b9766f6`.

### R1 (high): a live group after normal exit or PID-only interruption counts as terminated

`controller.py:365-371` sets `termination_ok` when the snapshot state is not
`unknown`. `controller.py:312-319` does the same after PID-only interruption.
Thus a final snapshot with live executable members gives `termination_ok: true`.

Receipt `k20-final-03/runs/f84d4883…/scope.json` shows this: member 14823 has
state `S`, and `termination_ok` is true. Its `ok` is false only because the
fixture kept the default `expected_reason="exit"`. A plan that declares
`expected_reason="interrupted"`, as the abrupt-interruption cases will, receives
`ok: true`.

Nothing signals that live member, and the ledger and `result.json`
record no residual identity or remedy. The root brief requires a concrete
residual identity for every owned resource. k17 requires positive cleanup of
each owned known child. The report mentions a surviving writer only as a stream
concern. k21 owns the surviving-writer fixture. The integrator must decide
whether this termination classification is k20's fix or an explicit k21 duty.

### R2 (medium): the 10 ms sampling reserve is shorter than measured `ps` time

`controller.py:256` starts a group snapshot when 10 ms of the phase remain.
Uninstrumented snapshots in the retained receipts took up to 23.7 ms in
`k20-final-01`, 12.1 ms in `k20-final-02` and 33.8 ms in `k20-final-03`. A
snapshot started with 10-34 ms left can time out. The result is `unknown`, and
`controller.py:318` then fails a valid termination. `k20-green-02` retained
this failure class with 0.7 ms left.

The final runs passed because no slow
snapshot fell in the last window. Live VM load can lengthen `ps` time. A
failed live sample cannot be replaced, so a false instrument failure voids it.

### R3 (medium): the real `ps` timeout path has no bound and no control

`_group_snapshot` calls `subprocess.run` with a timeout (`controller.py:187-191`).
In the pinned Python 3.12.15, a timeout calls `process.kill()` and then
`process.wait()` with no timeout (`subprocess.py:550-564`). `/bin/ps` on this
host is setuid root (`-rwsr-xr-x root`). Process creation is also outside the
timeout.

A `ps` that does not exit after SIGKILL blocks the call, the leader
reap and the `finally` disposal with no bound. `within_process_bounds` reports
the overrun only if the call returns. The timeout control replaces
`subprocess.run` with a stub that raises at once (`test_controller.py:474-475`).
So no run exercised the real kill and wait of a setuid `ps`. The report names
blocked storage as the open disposal guarantee, but not this path.

### R4 (medium): the audit credits 18 malformed scratch paths as absent

`evidence/k20-audit-03/audit.py:47-49` extracts scratch directories with a
regular expression. The match starts at the first `/` of a joined command line.
In `k20-audit-03/result.json`, 18 of the 115 entries are command lines with
spaces. An example is `…/.venv/bin/python …/peer.py scope-child /private/var/…/
test_review_signal_scope_has_o0`. `directory.exists()` is false for each, so
each reports `absent_after: true`. Audits 01 and 02 have 12 of 79 and 15 of 97
such entries.

Each real directory also appears as a separate entry, so no real
directory went unchecked. But the report's "115 exact scratch directories are
absent" credits paths that never existed. A broken instrument reads clean here.
The live and usage controls (`audit.py:96-113`) also use another argv form and
do not pass through the `absent` expression that judges the 414 PIDs.

### R5 (medium-low): the pinning control was never seen to fail against an early reap

`test_controller.py:434-464` records reaps only through `Popen.wait`. It does not
see `Popen.poll`, `os.waitpid` or `os.wait`. Its last step calls the guard
directly after the run (`:463-464`). That shows the guard function raises. It
does not show that the test fails against a controller that reaps early.

`k20-red-01` did not contain this test, and the `k20-green-02` failure was a
`ps` timeout. The report's sentence "A guard rejects even a signal-0 attempt
after observed reaping" describes the test helper, not a controller property.

Static reading finds no early-reap path in the current controller. `_call`
reaps only after `_terminate` returns. `subprocess._cleanup` touches only
dereferenced `Popen` objects. kqueue does not reap. Also, `_group_snapshot`
requires the leader in the table, so a released identity reads `unknown`. The
code holds here, but the control does not establish it.

### R6 (low-medium): the reap bound has no isolating control

No test forces an overrun on an otherwise successful termination and asserts
failure. No mutation removes the `min(now, term_deadline)` cap at
`controller.py:299`. In `k20-final-03`, every receipt with
`within_process_bounds: false` already had a failed termination: runs
`d00dfe72` (observer-error), `9992c773`, `2fe3b33d` and `4ded0a4b` (unknown),
`c3df6c32` (stubborn) and `fc8fd556` (scope). An incomplete group makes the
poll loop use the whole kill phase. The reap then always overruns by about
1 ms. So the suite cannot tell a bound violation from a termination failure.

### R7 (low-medium): the fork fixture's known child has no failure-path cleanup

`peer.py:61-74` starts `/bin/sleep 60`. `test_controller.py:154-162` and
`:326-335` inspect it, but no `finally` releases or ends it. If termination
fails, the child can live for 60 seconds with no positive cleanup. If the peer
fails before it prints, `int()` raises and the PID is lost, so the audit cannot
record it. The scope fixtures use a release marker and exit by themselves
within 5 seconds. The report's "Each known child is then released and checked
independently" is true only for the scope and stubborn fixtures.

### R8 (low): the nested-controller survival test has no retained failing control

`test_controller.py:421-431` passed against the old controller in `k20-red-01`,
as expected, and in every later run. Its failure route works by inspection: a
dead inner controller gives a non-zero exit, and the outer result fails. But no
retained mutation shows it detect a controller that signals itself.

### R9 (low): the checker runner can end without a verdict

`self_check.py:89-90` waits again after a group KILL. A second `TimeoutExpired`
is not caught there. The runner then ends without `verdict.json` or receipts
for the checkers it ran.

### Checked with no finding

- kqueue observes leader exit after TERM, with no wait or poll.
  PID-only KILL depends on need: `k20-red-01` failed with `['TERM', 'KILL']`,
  and later runs pass.
- A group with a surviving member after leader exit still receives KILL. The
  stubborn receipts show leader exit before KILL, and dropping KILL fails.
- Every `killpg` and snapshot happens before `process.wait`, while the zombie
  leader pins the group.
- Usage, timeout, foreign and missing-leader tables stay `unknown`, and any
  `unknown` snapshot fails the call. A stale snapshot cannot give zombie-only
  completion before leader exit. Every member must show `Z`, the leader included.
- Real signal-0 probes on zombie-only groups returned EPERM 99 times in
  `k20-final-03`. Thus this host shows zombie-only EPERM without
  instrumentation. Permission failure and unknown classifications are separate.
- The signal swap changes the external child outcome in both directions.
- `k20-final-03` pins the delivered hashes of all five sources, the manifest,
  the lockfile, `Taskfile.yml` and the interpreter. Its JUnit record has 39
  tests and no failure. Its 303 artifact hashes still match.
- The commit only adds `k20-*` evidence and changes no earlier evidence.
- The report defers stream and result identity to k21 and storage isolation to
  k18. It claims neither as done.

## Decisions (running log)

- Findings warrant reconciliation. Integration `termination-evidence` goes ahead
  of `retained-evidence-k21`, the first later live sibling.
- This session has no provider evidence, so the review writes no Creator line.
