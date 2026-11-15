# First worker reap at an expired cutoff (k107)

The worker owner now receives its first exact-child `waitpid(pid, WNOHANG)`
attempt even when its fixed shutdown cutoff has expired. Previously that cutoff
suppressed the first attempt, leaving an already exited worker as a residual.
An owner-level `reap_attempts` counter consumes the exception before the syscall,
including no-result, errno and delayed-return paths. Once an attempt exists,
the original cutoff still closes later polls. Signal guards and phase deadlines
are unchanged. Successful late reap releases ownership and retains failed timing.

This delivers the narrowed complete-worker-scheduler-k107 slice. The complete
original aggregate charter and history are preserved in the flat sibling
complete-worker-scheduler-k110. Shared earliest-deadline scheduling, concurrent
shutdown/teardown, remaining aggregate controls and final bounded-inspection-k28
review remain open. k28/k17 remain open; guest acceptance is **0/10, unavailable**.

## Controls and retained results

All runtime measurements used exclusive frozen inputs and output directories,
one checker at a time. Each declaration fixes commands, deadlines, subjects and
hashes before launch. Every verdict has unchanged before/after subject hashes,
raw stdout/stderr, JUnit, checker exit/reap receipts and artifact digests. Checker
limits were 60 seconds for controls, 120 for the full suite, 30 per static check,
then TERM 2 seconds and KILL/reap 2 seconds on timeout. No checker timed out.

| Condition | Cases | Result |
| --- | ---: | --- |
| `red` | 108 | 46 failed, 62 passed |
| `green` | 108 | All passed |
| `full` | 754 | All passed; Ruff, format and strict Pyright passed |
| `suppress-first` | 42 | 28 failed, 14 passed |
| `allow-later` | 42 | 12 failed, 30 passed |
| `omit-attempt` | 42 | All 42 failed |

RED runs the new tests on the exact parent runtime (`74c9ec68`), verified in
`source-comparison.json`. Fourteen pre-cutoff cases fail because the old owner
has no attempt counter. The other 28 new cases fail because the first syscall
never occurs at/after expiry. Three existing live-worker expired-close
samples and the observed-exited-worker close control fail their new contract.
There were no other RED failures. GREEN's 108 cases include those 42 new cases,
42 declared-phase samples, ten persistent-close cases, ten fixed-cutoff cases
and four existing first-workload-reap cases.

The new matrix crosses before/equal/after cutoff, PID-only/pre-ready versus
group-ready owners, and seven outcomes: real exited/live children, delayed
exited/live returns, simulated ECHILD/EIO, and delayed EIO. Child exit is gated
until kqueue registration; exit observation consumes no status. Only clocks and
errno/return delay are simulated. A live pre-cutoff owner can poll again before
cutoff. Repeated expired reap and TERM/KILL callbacks make no further syscalls.
Each case asserts fixed identity, exact WNOHANG flags, attempt count, status,
errno, ownership-loss flag and owner-ledger membership.

The persistent-close expired case observes actual SIGTERM exit before cutoff,
then closes after expiry. Runtime reaps once with `-SIGTERM`, releases the owner,
sends no new signal and retains overrun/failure. Repeated close is inert. The
live expired-close samples make one check, retain the residual and send nothing.
Existing controls that already polled before expiry still forbid every later
poll and signal. The tests exercise the real close/finalizer paths as well as
direct stale callbacks.

Each mutant changes exactly one source line, checked in `condition-audit.json`:
`suppress-first` restores the old unconditional cutoff; `allow-later` removes
the later-poll guard; `omit-attempt` fails to record the attempt. The intended
cases reject all three. Frozen runtime, tests, manifest, lock and Taskfile in
`green-inputs` match the shipped bytes. The full pytest run took 65.13 seconds
(65.305 seconds including checker launch/reap).

## Independent cleanup and evidence audit

The per-case audit restores the real monotonic clock and original exact-child
wait operation. It confirms runtime-consumed status with ECHILD, or separately
kills/reaps the still-pinned fixture child within one second. Runtime evidence
is captured before that cleanup. Simulated ECHILD intentionally leaves a real
fixture child for the independent audit; it is not proof of a real kernel
ownership-loss event. Simulated EIO retains runtime residual ownership.

Across six conditions there are 252 new first-reap audits, 126 declared-phase
audits and 30 persistent-close audits: **408**, all reaped. These counts cover
those worker fixtures, not every other helper fixture in the full suite.
`condition-audit.json` verifies all 2,211 recorded artifact hashes, expected
JUnit cases/failures, absence of checker timeout/residual, one-line mutations
and final source equality. The executed aggregation source is retained as
`audit-initial.py.txt`; final `audit.py` differs only by four type annotations.
The initial tooling check found five strict typing diagnostics in that audit
script. Adding those annotations made the final tooling Ruff/format/Pyright
checks pass; runtime and test inputs did not change.

`prior-before.json` and `prior-after.json` independently inventory **163,383**
earlier evidence paths, including symlink targets without traversing directory
symlinks. Their complete maps agree with no additions, deletions or changes.
Large raw receipts and inventories are retained with an invocation-scoped jj
snapshot size allowance; no evidence was truncated to fit the default limit.

## Limits and handoff

The 42 full-suite declared-phase receipts contain six on-time grace outcomes,
six at-deadline outcomes and 30 late outcomes. The passing oracle verifies that
strictly late grace remains failure; it does not turn these into timing success.
The earlier intermittent failures retained by k94/k106 and their reviews remain
unresolved. This passing sample does not erase or explain them.

The code graph generation `2026-11-14T22:15:36Z` predates this slice. Targeted
coverage reported `metadata_changed` for worker/controller source and
`not_tracked` for the shutdown test. Direct source reads supplied the evidence;
this report makes no graph completeness claim.

No guest, installed application, global configuration or privileged fixture
changed. Real privileged-ps KILL and UID EPERM controls remain unavailable,
recorded separately in `availability.json`. Ordinary fixture signals and waits
are real; injected errno is labelled. No claim covers shared scheduling,
teardown overlap, storage isolation or hard real-time guarantees. Read the
scheduled k107 review before relying on this boundary; k110 retains aggregate
ownership and the final complete-runtime review.
