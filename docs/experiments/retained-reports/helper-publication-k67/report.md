> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Helper publication before exec (k67)

The per-query inspection worker previously exposed a helper identity only in its
terminal result. If the result or worker was lost, the controller could not retain
an independently known helper even when creation had already succeeded.

The raw helper now waits after inherited-state reset on a private pipe carrying a
32-byte token. Its existing capture deadline is immutable. The worker publishes
PID, PPID, PGID, per-query token and creation time before releasing the gate.
EOF, a wrong token, an expired deadline or failed publication prevents argv from
executing. There is no controller acknowledgement. The release uses one pipe write
below `PIPE_BUF`; the code cites the Python API at that call.

The controller validates the publication against the worker identity and dispatch
interval, rejects duplicates/conflicts and cross-checks a terminal result. The
control-channel cap counts consumed records as well as buffered bytes. A separate
ledger retains each published helper obligation until a matching reaped receipt.
It confers no signal authority. Worker reap, group signals, channel loss and
`ECHILD` do not resolve that ledger entry.

## Behavior controls and retained evidence

Nineteen cases were added to the existing 269-case local diagnostic suite. Real
forked helpers demonstrate publication before marker-producing exec, failed and
expired publication, gate EOF and wrong tokens. Real workers demonstrate matching
reap resolution, result loss after publication, and worker exit after publication
before gate release. Eleven real-worker cases corrupt the publication or terminal
identity: duplicate, token, PPID, PGID, Boolean PID, stale/future timestamp, deadline,
extra field, terminal PID and terminal creation time. Existing synthetic terminal
fixtures publish explicitly simulated identities; they are not kernel evidence.

| Frozen condition | Outcome |
| --- | --- |
| [k67-red-01](../k67-red-01/verdict.json) | Existing 269 cases pass; five new behavior cases fail. Static checks pass. |
| [k67-green-01](../k67-green-01/verdict.json) | 274 pass, two residual-remedy cases fail; three strict-type diagnostics. Failure evidence retained. |
| [k67-green-02](../k67-green-02/verdict.json) | All 276 cases and static checks pass. |
| [k67-green-03](../k67-green-03/verdict.json) | All 288 cases, lint, formatting and strict types pass. Pytest reports 30.90 seconds within the original 60-second ceiling. |
| [k67-audit-01](../k67-audit-01/result.json) | Source/evidence integrity and exact PID observations pass; two deliberately retained blocked-disposal mock markers make the resource result false. |
| [k67-audit-02](../k67-audit-02/result.json) | Integrity, 14 identity checks, exact PID observations and bounded-scope mock resource cleanup pass. |

Each self-check freezes its Python sources, manifest, lock, Taskfile, interpreter
and `/bin/ps` before and after the run, and retains checker streams, exit statuses,
JUnit case identities and artifact hashes. No measurement subject changed during
its run. The [final input copies](../k67-inputs-01/source-run.json) match GREEN03's
nonbinary subjects. Intermediate attempts retain hashes and raw outcomes, without
a claim that their historical source copies were preserved. Earlier failures are
retained without alteration.

The [resource auditor](../k67-audit-02/audit.py) checks all four conditions against
individual failed-case identities and checker statuses. It checks final input
copies and current source digests, and validates 14 final helper-identity receipts.
Its `/bin/ps` oracle has live, missing, mixed and invalid controls. It observes
2,290 recorded exact PIDs, excluding explicit synthetic values; all are absent.
The oracle uses a five-second bound. It sends no signals to observed identities.

The auditor enumerates 636 fixture scratch directories in the retained test
sessions. Two `diag-*` markers remain from tests that deliberately bound a failed
disposal. Only after the clean process observation, the second auditor matches
each marker to its exact run plan, acquisition argv, identity and bytes `owned`,
then removes those two files. Their original bytes, digests, paths and plans remain
in its result. Other scratch files remain. The original failed resource result and
auditor preparation/lint failures remain available beside the successful audit.

## Limits and remaining work

These later observations establish absence of the recorded exact PIDs at audit
time. They do not acknowledge helper reaps, repair unknown creation, prove absence
of unpublished helpers or constitute a full process census. The retained runtime
ledger remains conservative after worker/result loss. Empty PID witnesses and
synthetic exclusions are itemized by the auditor.

This increment uses the existing per-query worker. Persistent pre-resource startup,
the shared earliest-deadline scheduler, complete request/epoch validation, explicit
fork-error/no-child messaging, the full fault inventory and the aggregate review
remain in `complete-worker-scheduler-k69`. A separate review of k67 precedes that
continuation. All aggregate ancestors remain open. Guest, installed-app, privileged
fixture, stalled-storage and ten-clone acceptance claims remain unchanged:
**0/10, unavailable**.

Graph coverage for the runtime/tests/runner and auditor has matching metadata and
no recorded issue at generation `2026-11-14T13:47:14Z`. This is a best-effort signal,
not proof of completeness; evidence and documentation were read directly.
