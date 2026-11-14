# Workload termination cutoff admission — k85

`controller._terminate` now establishes its original outer K before any TERM or
exit observation. It samples the immutable signal deadline immediately before
PID/group dispatch, refuses equality and expiry with `deadline_exhausted`, and
keeps termination unsuccessful. Exit observation has both entry and returned-time
checks at the inspection cutoff. A late result cannot become a timely leader
exit or authorize the final process table. The existing PID min(G,T)+A cap,
group T+A cap, group KILL after fixed grace while authority remains open, and
PID-only signal scope are preserved.

This is a bounded admission repair. It does not prevent process preemption
between the last clock sample and the syscall or make stalled kernel calls
bounded. `_wait`, group query/probe admission, reaping and the rest of the common
scheduler remain under the complete aggregate charter in
`complete-worker-scheduler-k88`. The worker is still per-query. k28/k17 stay
open, including the required final aggregate review; ten-clone acceptance stays
**0/10, unavailable**. Storage isolation remains k18 and retained identity remains
k21. No guests, installations, privileges or unrelated process signals were used.

## Frozen conditions

Each named directory contains the immutable verdict, raw stdout/stderr and JUnit
where completed. The runner pins source, configuration, checker and executable
hashes before and after each condition, captures exact receipts, uses a 20-second
control or 120-second full-suite budget, and owns its checker with bounded TERM
and KILL/reap cleanup. Full lint/format/type checks each have 30-second budgets.
`run_condition-v1.py`, `run_condition-v2.py` and current `run_condition.py` retain
all three checker source identities. `audit.py` matches each recorded checker
hash to those files; old runs are historical evidence, not claims against the
current checker bytes.

| Condition | Result | Interpretation |
|---|---:|---|
| red-1 | 20-second timeout; checker TERM/reaped; no JUnit | Initial fake clock moved backward; incomplete failed sample retained |
| red-2 | 10 failed / 2 passed | Baseline fails; two real assertions masked by bytes-to-JSON audit serialization |
| red-3 | 10 failed / 2 passed | Corrected baseline distinguishes late signals/entry/returns and actual late KILL; timely controls pass |
| red-4 | 12 failed / 2 passed | Original runtime with all 14 final cases, including observer entry equality |
| green-1 | 12 passed | Initial repair against first 12 controls |
| green-2 | 14 passed | Final repair against all final controls |
| signal-removed-1 | 8 failed / 6 passed | Sole signal guard deleted; delayed/equal/expired first turns and real children fail |
| entry-removed-1 | 4 failed / 10 passed | Sole observer entry guard deleted; expired first turns and equality fail |
| return-removed-1 | 2 failed / 12 passed | Sole observer return guard deleted; late returned exits fail |
| signal-equality-removed-1 | 4 failed / 10 passed | Sole signal `>=` becomes `>`; exact K and expired first-turn equalities fail |
| entry-equality-removed-1 | 2 failed / 12 passed | Sole entry `>=` becomes `>`; observer entry equality fails |
| return-equality-removed-1 | 2 failed / 12 passed | Sole return `>=` becomes `>`; returned-time equality fails |
| full-1 | 442 passed / 6 failed; lint/format/types pass | Failed samples and receipts retained; see below |
| full-2 | 448 passed; lint/format/types pass | 56.7718-second pytest command, same runtime/test bytes as full-1 |

Each mutant changes precisely one guard in `green-v2-inputs/controller.py`;
other frozen files match byte-for-byte. Deterministic tests cover both scopes,
before-cutoff timely exits and signals, exact equality and beyond-cutoff turns,
late observer returns, explicit failed receipts and no final query. The auditor
compares exact case identities, expected failure sets and distinguishing failure
messages. Counts alone are not the oracle.

## Failed full run and replay

`full-1` failed `test_review_later_zombie_table_cannot_repair_earlier_eperm`
with an unknown final helper table, both scopes of
`test_review_exited_leader_does_not_hide_a_surviving_group_member` and
`test_review_live_group_is_failure_even_with_expected_leader_exit` with empty
PID stdout, and `test_review_checker_unreaped_timeout_still_retains_verdict`
with a missing frozen-tree Taskfile. No earlier sample was edited or rerun
in place. The permission and empty-PID fixture mechanisms remain unresolved;
they also have aggregate ownership in k88. A later pass does not diagnose or
repair those failures.

`green-v3-inputs` is a fresh copy of green-v2. Only the frozen `self_check.py`
metadata lookup changes from `HERE.parents[1] / "Taskfile.yml"` to
`HERE / "Taskfile.yml"`, with the actual Taskfile copied beside it. No live
checker, runtime or test change was made. The auditor verifies this sole
lookup adjustment and every live/frozen Python byte. `full-2` passes all 448
identical case identities, Ruff, formatting and strict pyright.

## Real children and audit limits

Real delayed-observer controls start a direct child in its own session, forward
actual TERM and, in negative conditions, actual late PID/group KILL. The normal
non-reaping exit observer runs before an injected 0.2-second delayed return.
This is an induced late return, not a claim of a stalled kernel syscall or
kernel unreapability. Frozen signal call timestamps and receipt PID/scope are
retained. The independent test owner keeps the exact Popen child, attempts
exact-PID KILL and waits with a two-second bound, then records absence. No PID
from a process table becomes signal authority. All final-condition real fixtures
are reconciled by their recorded PID, scope, exit status and before/after state.
Later absence does not establish a historical helper reap acknowledgement or
resolve unknown helper creation. This is not a resource census of every old
full-suite fixture. Real privileged UID/ps KILL controls remain unavailable.

`audit-1/verdict.json` passes: all 14 conditions reconciled, 24 real child records
checked, 99,292 prior evidence file entries unchanged and every audited read
subject unchanged. The standalone auditor validates subject/artifact hashes, isolated changes,
individual case outcomes, real fixture records and prior evidence file bytes.
It retains its own before/after subjects and result in a fresh audit directory.
The prior inventory covers existing file contents (including resolved file
symlink contents), excluding caches and directory symlink traversal; it does
not certify symlink text, directory metadata or process ownership from absence.
Historical executable hashes are recorded without assuming today's bytes match.

## Reproduction

Run each condition into a **new** output directory; negative conditions are
expected to return failure. For example, from the workspace root:

```sh
mise exec -- task diagnostic:termination-cutoff-check INPUTS=experiments/exec-reliability/evidence/termination-cutoff-k85/green-v3-inputs OUT=/absolute/new/full MODE=full
mise exec -- task diagnostic:resource-audit AUDITOR=experiments/exec-reliability/evidence/termination-cutoff-k85/audit.py
```

The auditor defaults to a fresh `audit-1`; set `DIAGNOSTIC_AUDIT` to another new
basename for a repeat. It does not rewrite condition directories or prior files.
