# k66: keep raw helpers in their worker's group

The k65 review found a real containment defect. The k62 raw helper child called
`reset_after_fork`, whose unconditional `setsid()` detached the helper from the
worker's process group and session. The earlier green03 pre-exec observation
recorded helper PID/PGID/SID 9602 with parent 7885. That agreed with the source
and contradicted the worker-group contract in
[`diagnostic-process-inspection.md`](../../../../docs/specs/diagnostic-process-inspection.md).
The old test had asserted the detached identity, so its success did not establish
the specified containment.

`reset_after_fork` now has a keyword-only `new_session` option, defaulting to
`True`. Worker bootstrap keeps that default. `_helper_child` passes `False`, so
the exec helper inherits its worker's PGID and SID while keeping its own PID.
Signal disposition/mask reset, wakeup-FD reset, disabled GC, fresh helper-owner
state, descriptor closure, standard-stream setup and `_exit` failure handling
remain in place. The existing pre-exec control now checks inherited PGID/SID.

## Retained runs

Both runs used `mise exec -- task diagnostic:self-check OUT=<new directory>`.
The declaration fixes lint/format/type budgets at 30 seconds each, pytest at
60 seconds, and timeout cleanup at TERM 2 seconds then KILL/reap 2 seconds.
No source subject was edited during either run. The same 259 individual JUnit
case identities occur in both conditions.

| Condition | Retained result |
| --- | --- |
| [`red01`](red01/verdict.json), before the repair | Lint, format and strict types pass; pytest: 257 passed, 2 failed, 30.461 seconds in JUnit. The pre-exec group assertion and real group-KILL control fail. |
| [`green01`](green01/verdict.json), after the repair | Lint, format and strict types pass; all 259 tests pass, 31.059 seconds in JUnit. Every checker exits normally and is reaped. |

Each verdict records exact before/after SHA-256 values for 14 subjects: nine
diagnostic Python files, `pyproject.toml`, `uv.lock`, the root `Taskfile.yml`,
the selected interpreter and `/bin/ps`. The first twelve have matching retained
copies in [`red01-sources`](red01-sources) and
[`green01-sources`](green01-sources); the two binaries have digests only.
Per-run artifact hashes, individual cases, stdout/stderr and every failure
remain retained. The only subject that differs between the runs is
`inspection.py`.

## Real worker-group KILL control

The new control uses the production worker child and genuine helper
admission/fork/reset/exec path. A live Python helper writes its identity before
waiting. The parent checks the worker's separate session/group, verifies the
helper's parentage and OS group/session, and registers a macOS kernel process
exit notification for that exact helper. It sends real `SIGKILL` to the worker
group while the direct-child worker is unreaped, then observes the helper
independently of the worker status and `killpg` return.

The fixed control bounds are readiness 1 second, exit notification 0.5 seconds
and cleanup 2 seconds. Worker capture has a 5-second deadline; the helper fixture
self-expires after 6 seconds. Its Python payload replaces `/bin/ps`; identity
files and cooperative release are fixture coordination. No runtime capture,
group signal or kernel exit observation is simulated. The older pre-exec
control patches the exec boundary to inspect reset state; it remains an adapter
control and is separate from this actual exec witness.

| Observation | RED | GREEN |
| --- | --- | --- |
| Worker PID = PGID = SID | 50518 | 7066 |
| Helper PID / PPID / PGID / SID | 50519 / 50518 / 50519 / 50519 | 7070 / 7066 / 7066 / 7066 |
| Helper exit notification after group KILL, within the fixed bound | None | Exact PID 7070, `KQ_NOTE_EXIT` |
| Worker cleanup | Exact child reaped, return code -9 | Exact child reaped, return code -9 |
| Fixture cleanup | Cooperative release then exit notification for 50519 | No additional helper cleanup needed after the observed exit |

The receipts are
[`RED`](red01/runs/worker-group-kill-real-helper-ee8cbb27155b4beeaea1e6d8836c1538.json)
and
[`GREEN`](green01/runs/worker-group-kill-real-helper-c710153c36284f7e98f1fa78e77beb7b.json).
The RED receipt shows why worker death alone cannot establish helper death.
The GREEN receipt independently observes helper exit after the group signal;
it does not provide a helper wait status or reap acknowledgement, because that
helper is no longer this controller's child after worker death.

## Preservation and scope

[`verification.json`](verification.json) records the frozen evidence audit,
run through `mise exec -- task diagnostic:resource-audit
AUDITOR=experiments/exec-reliability/evidence/worker-group-k66/verify-evidence.py`.
It verifies artifact hashes, twelve source snapshots per condition, stable
14-subject run manifests, exact expected failing cases, matching individual
case inventories and the two kernel-control receipts. Its own input digests
are retained before and after. All 55,603 earlier evidence files, including
ignored disk files, retain identical membership and bytes in
[`prior-evidence-before.json`](prior-evidence-before.json) and
[`prior-evidence-after.json`](prior-evidence-after.json).

The earlier [k62 report](../raw-helper-fork-k62/report.md) and its samples remain
unchanged; this report corrects its session/group account for the current source.
Status 70 still has no dedicated child-error channel and does not identify the
failed child phase. The full k64 charter remains unchanged and live: persistent
worker bootstrap, every-fork admission/reset, gated identity publication,
authenticated bounded channels, independent known-helper reap, shared
scheduling and the full adversarial control inventory still require their own
work. All aggregate ancestors remain open. No guest, installed application or
privilege was used. Ten-clone acceptance remains **0/10, unavailable**.
