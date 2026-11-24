> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Declared bootstrap bounds (complete-worker-scheduler-k101)

Plan and direct Worker startup now validate immutable `BootstrapBounds` before
freezing evidence or creating pipes/forking. Positive finite numeric allowances
have ceilings readiness=1s, failed-bootstrap TERM grace=2s and KILL/reap=2s;
bools, strings, null, NaN, infinity, zero, negatives and over-ceiling values are
rejected. Defaults retain the existing 1/2/2-second policy. The controller passes
the frozen declaration to its persistent worker.

Startup uses the declared readiness deadline. Failed bootstrap fixes its TERM
and KILL/reap deadlines once, preserving an earlier owner cutoff. Signals stay
PID-only before confirmed readiness. Receipts include bounds, absolute readiness
and shutdown deadlines, acceptance/finish times, outcome, overrun, signal history
and residual ownership. A delayed observation can exceed a sampled deadline;
it records failed timing and cannot reopen signal authority. Healthy and
request-failure close scheduling still use the existing policy.

## Frozen conditions

Each directory retains declaration, raw stdout/stderr, itemwise JUnit, verdict,
subject digests before/after, artifact digests and resource receipts. Every
condition had stable subjects and a reaped checker parent without timeout.

| Condition | Result | Interpretation |
| --- | --- | --- |
| red | 30 failed | Feature API absent in the original runtime; initial checker retained. |
| green1 | 621 passed, 1 failed; type check failed | The ready fixture reset its session twice. Typed runtime guards also needed explicit object casts. Original failures retained. |
| green2 | 30 passed | Corrected real-child checker and runtime evidence. |
| full-green2 | 622 passed; all quality checks passed | Corrected frozen source. |
| mutant-plan-validation | 24 failed, 6 passed | Removing Plan validation reaches forbidden freezing. |
| mutant-plan-wiring | 1 failed, 29 passed | Dropping the Plan declaration at Worker construction loses it. |
| mutant-readiness | 4 failed, 26 passed | Restoring the hard-coded one-second readiness allowance exceeds selected bounds. |
| mutant-term | 2 failed, 28 passed | Restoring the two-second TERM grace loses the selected KILL transition. |
| mutant-kill-reap | 4 failed, 26 passed | Restoring the four-second outer allowance changes the fixed terminal deadline. |
| mutant-overrun | 1 failed, 29 passed | Hiding the overrun violates the receipt assertion. |
| full-final | 622 passed; lint, format, strict types passed | Final frozen source and Taskfile, after adding reproducible preparation tooling. |

The six mutations change one named source site each and use the corrected green2
checker. `prepare.py` rejects ambiguous mutation sites and existing output paths.
`tooling.stdout` and `tooling.stderr` retain successful lint, format and strict
type checks of both tooling files. Frozen full runs cover the complete diagnostic
pytest suite and runtime quality checks; they do not run guest verification.

## Controls and resource accounting

Twenty-four cases reject invalid bounds before evidence directory creation,
subject hashing or fork, and repeat validation at direct Worker startup. Five
real direct-child modes cover delayed successful readiness, TERM exit, real
KILL after ignored TERM, deferred observation and a delayed syscall return.
The last two fault only the parent observation adapter; they are simulated
observation stalls, not claims of uninterruptible kernel processes. Every child
is audited with the original exact-child nonblocking waitpid adapter. An
unreaped child pins its PID until independent KILL/reap, with no widened group
cleanup or guest access. The sixth integration case records Plan/Worker bounds
and independently confirms the worker was reaped.

`condition-audit.json` checks every retained artifact digest and subject pair,
all checker-parent reap outcomes and all 50 real-child audit receipts. It also
confirms final live runtime/test/manifest/lock/Taskfile bytes match inputs-final.
The aggregate suite's other resources retain their own existing receipts; this
50-child count describes only the new bootstrap fixture audits. The Plan
integration's independent waitpid assertion is recorded in its JUnit case.

`prior-evidence-audit.json` preserves an initial audit that incorrectly followed
symbolic links, producing false differences. The separate link-aware audit
compares the parent commit's blob identity to regular file bytes and stored
link text without dereferencing links. Its result is recorded in
`prior-evidence-link-aware-audit.json`: all 123,249 prior paths, including 18
symbolic links, match the pre-session parent commit. Neither audit rewrites
prior evidence.

## Reproduction

From the repository root, choose fresh output paths:

```sh
mise exec -- task diagnostic:bootstrap-bounds-prepare INPUTS=experiments/exec-reliability OUT=/tmp/bootstrap-inputs-new
mise exec -- task diagnostic:bootstrap-bounds-check INPUTS=/tmp/bootstrap-inputs-new OUT=/tmp/bootstrap-controls-new
mise exec -- task diagnostic:bootstrap-bounds-check INPUTS=/tmp/bootstrap-inputs-new OUT=/tmp/bootstrap-full-new MODE=full
mise exec -- task diagnostic:bootstrap-bounds-tools
```

For a negative condition, prepare a fresh output from inputs-green2 with
`MUTANT=plan-validation`, `plan-wiring`, `readiness`, `term`, `kill-reap` or
`overrun`, then run the control task. Nonzero pytest is expected. The runner
bounds controls to 60s and full pytest to 120s, with independent 2s TERM and 2s
KILL/reap fallback for checker timeout. These checker budgets are separate from
the runtime's declared bootstrap phases.

## Remaining contract

This delivers the declared bootstrap slice only. Worker-failure and orderly-close
phase declarations, one shared scheduler, shutdown overlap, remaining full-spec
controls and the final bounded-inspection-k28 review remain live in
complete-worker-scheduler-k104, with the entire original aggregate charter.
The scheduled review-impl leaf supplies independent review of this producer.
k28/k17 remain open; ten-clone acceptance remains **0/10, unavailable**.
No guest, installed app/CLI, privilege or connection-file changes were used.
