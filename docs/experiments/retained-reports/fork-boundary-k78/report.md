> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Fork admission after preparation (k78)

Both actual worker and helper fork sites now revalidate admission after preparing
pipes and masking signals, then compare monotonic time against the original
capture cutoff. The final admission snapshot replaces the earlier one in the
receipt. Refusal restores the previous mask, closes prepared descriptors and
creates no owner. Helper refusal precedes the os.fork OSError handler, so it
cannot publish a kernel fork-failure receipt. The existing early checks remain.
Admission is still a snapshot, not a lock; synchronous OS/native observation or
fork can return late. The late-fork cleanup control still exercises that case.

## Retained conditions

| Condition | Frozen inputs | Result |
| --- | --- | --- |
| red-01 | inputs-original | All 14 selected controls fail at attempted fork |
| green-01 | inputs-patched | All 14 selected controls pass |
| full-01 | inputs-patched | Lint/format/types pass; 414 tests pass, one obsolete fixture fails |
| red-02 | inputs-original-final | All 14 selected controls fail at attempted fork |
| full-02 | inputs-final | Lint/format/types pass; all 415 tests pass |

Every condition retains declarations, subject hashes, raw output, JUnit, captures
and a separate checker verdict. Each checker was reaped without timeout. The
final full check took 44.97 seconds total (pytest 44.02 seconds).

Fourteen controls cross worker/helper boundaries with seven conditions: a real
late Python thread, a real native pthread absent from the Python registry,
Python SIGCHLD=SIG_IGN, native Darwin SA_NOCLDWAIT with Python SIG_DFL, native
observation EIO adapter, preparation consuming the deadline, and the second
admission observation consuming the deadline. All state mutations happen after
mask setup. Deadline delays use real elapsed time and the fixed original bound.
The os.fork tripwire raises EAGAIN and records an attempted call without making a
kernel fork: unsafe thread/auto-reap conditions never create an actual child.
GREEN asserts empty fork/publication lists, no PID/worker/owner or new helper
obligation, restored descriptor sets and mask, unchanged deadline and the final
refusal reason. Existing controls retain real successful fork/reap and actual
fork-error adapter behavior.

The full-01 failure was TypeError comparing absent creation time with deadline.
Its old _spawn_helper delay happened before the new guard, correctly preventing
creation. The final fixture instead delays within the actual os.fork adapter,
after admission, and makes a real kernel child: it still verifies pinned-child
KILL after late return, without renewing the allowance. The failed run is retained.

## Independent resource and evidence reconciliation

`audit.py` failed before resource disposition because it incorrectly assumed
Linux EAGAIN=11; Darwin uses 35. Its source and partial audit-01 output remain.
`audit-final.py` uses errno.EAGAIN. Its `audit-final-01/verdict.json` reconciles
all five conditions and exact failure reasons, rejects an intentionally wrong
failure reason, and compares all cases to full-02: 29 failing observations become
passing cases (14 + 14 + 1).

The final auditor hashed 2,818 subjects before/after, verified 85,216 older
evidence files unchanged, independently observed all 1,463 recorded exact
fixture PIDs absent with /bin/ps, and inspected 155 scratch directories. It found
no remaining plan-owned mock markers and removed none. Live/absent/mixed/invalid
PID oracle controls distinguish absence from malformed queries and nonempty
results. Sources, runner, binaries and retained condition artifacts remained
unchanged. Both auditors passed Ruff lint/format before execution; initial
formatting-only preflight refusals did not execute an auditor.

Reproduce focused conditions using `mise exec -- task diagnostic:fork-boundary-check
INPUTS=<frozen inputs directory> OUT=<new directory>`, full checks using
`diagnostic:self-check OUT=<new directory>`, and the auditor using
`diagnostic:resource-audit AUDITOR=experiments/exec-reliability/evidence/fork-boundary-k78/audit-final.py`
(the frozen auditor's existing output must not be reused).

Later exact-PID absence does not grant historical helper reap authority, prove
unpublished-helper absence, or turn failed captures into success. No guest,
installed app or privileged operation was performed. Ten-clone acceptance is
still 0/10 unavailable. The persistent pre-acquisition worker, shared scheduler
and all original aggregate controls remain assigned to
complete-worker-scheduler-k81. No aggregate ancestor is complete. A separate
review-impl leaf will contest this bounded artifact before further runtime work.
