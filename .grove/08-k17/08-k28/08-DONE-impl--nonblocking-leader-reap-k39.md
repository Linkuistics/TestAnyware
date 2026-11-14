# nonblocking-leader-reap-k39


## Goal
Remove timed blocking workload-leader reaping from the active controller path.



## Context

## Done when
- Reap only with exact-PID WNOHANG before the fixed deadline, after workload signal/probe authority closes.
- An unreaped or unexpectedly missing leader fails the call with an explicit residual and strong ownership, without destructor or implicit subprocess cleanup reaping. No late repair wait.
- Preserve signal ordering, table validation, deadline gates; verify real ordinary and withheld reaps, expiry, destructor cleanup and late classification. Freeze evidence, independently audit exact identities, update report.
- Leave all remaining parent guarantees in a live successor. Final producer owns the full required review.

## Running log
- 2026-11-14: First independently useful slice of k38; full parent contract remains. Claim under test: exact-PID WNOHANG and explicit strong ownership remove workload wait/destructor paths after expiry. A private Popen subclass suppresses destructor reaping; retaining ordinary Popen alone still permits interpreter-finalization polling. Helper creation/waits, transport, scheduler and storage remain outside this slice. Scheduled final runtime review will contest ownership; failing controls provide the in-session adversarial instrument.
- 2026-11-14: Full-suite failures exposed inspection spending the entire K allowance. Reserve R=min(100 ms,A/4) inside K, including normal exit, so failed table/exit observation still permits exact-child reap. This does not bound synchronous inspection; overrun skips reaping and retains ownership. Updated pin and late-return controls to intercept waitpid rather than Popen.wait. Full suite: 105 passed before evidence freeze.
- 2026-11-14: Frozen k39-previous-01 fails all three new controls against the previous controller; k39-final-01 passes Ruff lint/format, strict Pyright and 105 tests. k39-inputs-01 retains both source snapshots. k39-audit-01 verifies 543 indexed artifacts and 581 stable inputs, observes 103 recorded real PIDs absent, accounts for 27 markers and cleans 38 exact-argv fixture directories. Faults simulate wait outcomes/expiry on real zombies, not OS unreapability. Durable report updated. worker-scheduler-k40 retains every remaining parent guarantee and final review; k38 is not closed.

## Notes
