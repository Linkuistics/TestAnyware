# Worker reap lifetime scope — k112

The k111 findings are integrated. The runtime remains owner-lifetime scoped:
request polls consume the first-reap exception, and neither a request nor close
resets it. A previously polled worker that exits after its last poll and reaches
expired close retains failed residual ownership, even if its status is waitable.
This limit is explicit in the specification and the k110 handoff. k110 retains
the shared scheduler, shutdown/teardown overlap, all remaining controls and final
aggregate review. k28/k17 remain open; ten-clone acceptance is **0/10, unavailable**.

## Disposition

Read the review from commit `2471c4b9` (reviewed implementation `11090fc3`).
Direct source inspection confirms `_Owner.reap_attempts` starts at zero and
`_reap` increments it before `waitpid(WNOHANG)`. Request polling uses the same
owner; only a clean request restores the deadline to infinity, without resetting
the count. Close preserves the count when assigning its finite deadline.

- **F1: unclear contract and visible trade-off.** Keep the lifetime rule rather
  than redesigning the aggregate scheduler in this integration. Clarify the
  specification and implementation foundation note. Add the missing real-worker
  control and carry the failed-residual limitation explicitly into k110.
- **F2: missing scope verification, fixed.** Retain a one-site close-entry
  counter-reset mutant. The new control must reject renewed reap authority.
  Earlier same-cutoff controls remain in the focused suite. Runtime code is
  unchanged, so their prior-attempt guard is unchanged too.

## Real worker control

`test_persistent_worker_close_has_one_fixed_grace[expired-polled]` starts the
actual persistent service, runs `/usr/bin/true` through `capture_helper`, and
checks successful helper completion and a nonzero worker poll count. It records
a time after the last request poll, registers a kqueue exit observer, sends real
TERM and observes the worker exit without consuming its status. Only then does
it set an already-expired cutoff and close twice.

The control requires unchanged attempt history, no extra signal, retained
ownership and residual remedy, failed timing, and idempotent close. An independent
exact-child nonblocking cleanup consumes the real TERM status and removes the
fixture ledger entry. The successful request receipt and pre-cleanup failed
runtime receipt are both retained. kqueue exit observation is macOS/BSD specific.
The finite cutoff is installed after the successful request: this demonstrates
the owner-lifetime boundary, not a request-timeout or shared-scheduler control.

The mutant changes only close's deadline-assignment line, appending
`owner.reap_attempts = 0`. It performs the extra expired reap, and the control
fails at `assert not owner.reaped and owner.pid in worker_capture._owners`.
Its child is nevertheless reaped and independently checked. Mutant lint is not
an oracle: the mutant run selects the real process control directly.

## Frozen verification

| Condition | Cases | Failures | Result |
| --- | ---: | ---: | --- |
| green focused controls | 109 | 0 | pass |
| reset-close scope mutant | 1 | 1 | rejected at required ownership assertion |
| full suite | 755 | 0 | pass, no errors or skips |

The full run also passes Ruff lint, format (15 files) and strict Pyright
(0 errors/warnings). These checks include all frozen flat Python sources and
all four new evidence scripts as appropriate to the repository's type scope.
All six checker processes were reaped without timeout. The runner declares
60-second focused, 120-second full and 30-second static bounds; checker timeout
cleanup allows TERM for two seconds then KILL/reap for two seconds.

`condition-audit.json` verifies each verdict, JUnit cases, retained artifact
hashes and the one-line mutant difference. It lists 191 independent exact-child
cleanup records: 95 focused, 95 full and one mutant. All report reaped, including
the real worker deliberately left residual by runtime. No tracked fixture or
checker child remains unreaped in these records. This is exact owned-child
evidence, not a process-name sweep or privileged/uninterruptible-kernel proof.

Each condition has its declaration, before/after subject hashes, stdout/stderr,
JUnit, per-case process receipts and verdict. Subjects include frozen Python,
configuration/lockfile/Taskfile, the live Taskfile/specification, evidence scripts,
Python and invoked helper binaries. No subject changed during a condition.
Green inputs match current source/configuration/Taskfile. Every run has its own
output directory and one writer. No prior evidence was executed or edited.
The prior inventory verifies **165,755** unchanged file/symlink entries, including
all earlier failed conditions and caches.

## Reproduction

Run through `mise exec -- task` using these tasks:

- `diagnostic:worker-reap-scope-preserve MODE=before` and `MODE=after` inventory
  earlier evidence. Their retained inventory destinations are exclusive.
- `diagnostic:worker-reap-scope-prepare INPUTS=. OUT=<fresh inputs>` freezes the
  flat experiment; `MUTANT=reset-close` applies the declared regression.
- `diagnostic:worker-reap-scope-check INPUTS=<inputs> OUT=<fresh output> MODE=controls`
  runs focused controls. `SELECT=expired-polled` selects the mutant control;
  `MODE=full` runs the suite and static checks. Mutant failure is expected.
- `diagnostic:worker-reap-scope-audit` verifies the retained condition layout
  without executing the frozen subjects. `tools` checks the scripts and `format`
  formats the implementation/control sources before freezing.

The named audit is a one-shot creation task with an exclusive output. To replay
it, copy the scripts and complete retained condition layout to a fresh sibling
directory, omitting only the derived `condition-audit.json` from that copy. Run
the copied audit there. Reproduction outputs must use fresh destinations; do
not overwrite these measurements.
