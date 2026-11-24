> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Persistent command dispatch through request turns

The persistent parent now sends its command from the shared request callback.
Each callback attempts one nonblocking write of the remaining, at most 4096-byte
envelope. Progress yields immediately; backpressure returns a future wake clipped
to the original query cutoff. The callback samples that cutoff again after the
ownership observation, immediately before writing. Existing request identity,
sticky failure, conservative helper ownership and fixed retirement remain the
authorities for failure handling.

This delivers the bounded command-dispatch slice of complete-worker-scheduler-k129.
The full scheduler is **not accepted**. The interleaved full comparisons retain
failures, including an unexpected standalone helper timeout that rejects the
predeclared comparison gate. complete-worker-scheduler-k132 preserves the original
aggregate charter and owns these unresolved failures, readiness wake design,
concurrent owners, actual teardown overlap and the final k28 review. k28/k17 stay
open; ten-clone acceptance remains 0/10 unavailable.

## Controls and limits

The six new exact-clock cases use real nonblocking pipes and a synthetic owned
worker. They check ready and short writes, a filled command pipe, broken pipe,
initial cutoff and cutoff reached during reap observation. They require ready
safety work first, at most one command write per turn, future backpressure wakes,
unchanged envelope/cutoff, terminal no-write behavior, sticky failure and retained
unknown creation. Synthetic ownership avoids signaling an unrelated PID; it does
not prove concurrent real-process scheduler behavior. The existing request and
identity controls retain real worker/helper execution and independent absence
checks.

- `red-parent-controls-v3`: all six controls reject the old private pre-driver
  select with `command dispatch waited outside the shared driver`.
- `candidate-controls-v2`: 20 request/shared-driver/dispatch controls pass.
- `candidate-identity-controls`: 97 dispatch, request, shared-driver and actual
  identity mutation cases pass. The parent command mutation hook now damages the
  actual `os.write` payload; child control/result mutations still wait for real
  helper reap before releasing damaged traffic.
- `mutant-idle-spin`: the filled-pipe case rejects immediate backpressure wakes
  with `command backpressure spun without advancing time`.
- `mutant-cutoff-write`: the reap-cutoff case rejects removing the second cutoff
  sample with `command write at or after query cutoff`.

The initial `red-parent-controls` setup is retained with its 60-second checker
timeout: the old private select did not advance the synthetic clock. The checker
terminated and reaped it. `red-parent-controls-v2` fixes that fixture limitation
and rejects five modes promptly. `red-reap-cutoff` rejects the first candidate,
which checked cutoff only before reap; the sixth control drove the final fix.
None of those earlier inputs or observations was overwritten.

## Frozen comparisons

`inputs-parent-verified` and `inputs-candidate-verified` share all tests,
configuration and lockfile; only `worker_capture.py` differs. The parent source
is explicitly recorded by preparation. Full parent runs exclude the six new
dispatch cases; full candidate runs include them. The final order is parent 1,
candidate 1, parent 2, candidate 2. Each condition retains its command declaration,
JUnit individual results, stdout/stderr, subject hashes before and after, artifact
hashes, checker termination/reap and exact fixture audits. Controls have a
60-second checker bound, full pytest 120 seconds, static checks 30 seconds;
timeout cleanup has TERM 2 seconds followed by KILL/reap 2 seconds.

The first four full comparisons are retained separately. Both initial candidates
missed all eleven parent command mutations because their old `_write` test hook
was bypassed. Both implementations now share the corrected actual-write hook.
All four first comparisons also found Ruff B905 in the new short-write assertion;
the final snapshots share the explicit `strict=False` correction. These failed
comparisons remain failures even when later runs pass.

| Final condition | Pytest result | Seconds | Ruff / format / strict pyright |
| --- | --- | ---: | --- |
| parent-final-1 | 881 passed | 83.76 | pass / pass / pass |
| candidate-final-1 | 886 passed, 1 failed (standalone snapshot timeout) | 83.39 | pass / pass / pass |
| parent-final-2 | 880 passed, 1 failed (expired-request residual remedy) | 95.80 | pass / pass / pass |
| candidate-final-2 | 887 passed | 78.86 | pass / pass / pass |
| static | no pytest | — | pass / pass / pass |

Every final shared test item is retained for comparison. The gate rejection is
part of this result, not a successful aggregate comparison. There is no retry
substitution: candidate-final-2 does not replace candidate-final-1.

## Failures still requiring explanation

- `candidate-full-1` retains
  `test_declared_shutdown_real_child_phases[blocking-2-deferred-reap]`:
  `test_shutdown.py:298`, `assert all(item < end for item in probes)`.
- `parent-full-2`, `candidate-full-2` and `parent-final-2` retain
  `test_persistent_worker_expired_request_is_sticky`: `test_worker_capture.py:2751`,
  `worker.evidence()["residual_remedy"]` is `None`.
- `candidate-final-1` retains `test_review_group_snapshot_uses_real_worker`:
  `test_worker_capture.py:1086`, `helper capture deadline exhausted`. Its receipt
  `runs/worker-group-snapshot-1794730372595455000.json` records standalone
  `request_identity` and `session_token` as null. `_group_snapshot(worker=None)`
  calls standalone capture, whose command buffer stays empty. This establishes
  that the new persistent write branch was not executed; it does not explain the
  helper stall. Helper PID 77581 was killed at 347651.573951541 after capture
  cutoff 347651.573231041, and reaped at 347651.579634833. The exact fixture PID
  77579 was independently killed and reaped, as retained by its paired audit.

The comparison gate declared only the first two historical cases before final
measurements. The standalone timeout must remain an unexpected failure, with
gate rejection preserved. A later isolated pass cannot explain any of these
observations. Aggregate acceptance requires the original charter's real,
interleaved scheduler and teardown controls and explanations of retained failures.

## Evidence audit

`condition-audit.json` verifies retained artifact hashes, unchanged measurement
subjects, checker reaping, fixture audits, the six RED failures, both mutation
causes, final shared test sets, runtime equality with the final frozen candidate,
and the single parent/candidate source difference. It also verifies the archived
failed setup and first comparisons; their original statuses stay recorded.
`prior-after.json` confirms all 216,731 prior evidence paths remain unchanged.

The original auditor's assertion failure is retained in
`audit-original-rejection.txt`. Its exact measured tooling is retained in
`tooling-final-measured`. The final auditor completes provenance verification and
writes the rejection to `condition-audit.json` before exiting nonzero; its output
is `audit-retained-rejection.txt`. This changes reporting, not the gate policy:
`integrity_verified` is true, `comparison_gate_passed` is false, and
`aggregate_accepted` is false. The two final pair differences are the standalone
snapshot failure in candidate 1 and the historical expired-request failure in
parent 2. All six new dispatch cases pass in both final candidates. Evidence
tooling passes Ruff, formatting and strict pyright; `static-after-audit` retains
the checks for the final auditor source.

## Reproduction

Use `mise exec -- task diagnostic:command-dispatch-prepare INPUTS=. OUT=<fresh>`
to freeze candidate inputs. Add `WORKER_SOURCE=<frozen-parent>/worker_capture.py`
for the parent, or `MUTANT=idle-spin` / `MUTANT=cutoff-write` for a single-site
mutant. `diagnostic:command-dispatch-check` takes `INPUTS`, a fresh `OUT`, `MODE`
(`controls`, `full` or `static`) and optional `SELECT`; the parent full selection
is `not command_dispatch_turns`. Every output must be fresh, preserving failures.
`diagnostic:command-dispatch-tools -- --check` checks evidence tooling;
`diagnostic:command-dispatch-audit` checks provenance and individual outcomes.
The evidence directory is a retained measurement corpus, not an application
dependency. No guests, installed application, privileges or machine configuration
were changed.
