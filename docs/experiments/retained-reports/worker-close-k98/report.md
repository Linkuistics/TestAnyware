> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Persistent worker close escalation — k98

Persistent worker close now allows one fixed 50ms orderly EOF grace, then attempts TERM before KILL. Failure and existing retirement skip that grace. Existing TERM/KILL attempts and the earlier owner deadline remain authoritative; repeated close is inert. This delivers the narrowed complete-worker-scheduler-k98 boundary only. The complete original aggregate charter and history remain in complete-worker-scheduler-k101 under bounded-inspection-k28.

## Runtime boundary

`Worker.close` in `experiments/exec-reliability/worker_capture.py` keeps its original four-second maximum allowance, capped by any earlier owner deadline. TERM history, including failed attempts, anchors the two-second escalation budget. KILL is scheduled at the earlier of that budget and the midpoint of the remaining allowance, reserving time for reap. A prior KILL is not repeated. Existing `_signal`/`_reap` cutoff guards and distinct helper/workload owners are unchanged. This arithmetic uses existing APIs; no framework-version-dependent API was introduced.

The close loop remains synchronous. Shared earliest-deadline scheduling, overlap with controller teardown, complete epoch closure, first expired reap behavior and the remaining aggregate controls still belong to k101. k28 and k17 remain open. No guests, installed application changes, privilege acquisition or signals to unrelated processes were used. Ten-clone acceptance remains 0/10 unavailable; storage work remains k18.

## Frozen measurements

Each sample uses a unique directory, frozen Python sources, manifest, lockfile and Taskfile. `run_condition.py` hashes those subjects, itself, the root Taskfile, interpreter and executable helpers before and after the sample. Its declaration fixes command order and bounds before execution. All failures are retained. Full pytest has a 120-second bound, focused pytest 60 seconds, and each static checker 30 seconds; checker timeout escalation allows TERM two seconds then KILL/reap two seconds. No checker timed out; every checker was reaped.

| Sample | Result | Checker elapsed |
| --- | --- | --- |
| `red-1` / `red-inputs` | 3 failed, 14 passed, 571 deselected | 10.394s |
| `green-1` / `green-inputs` | 17 passed, 571 deselected | 8.109s |
| `full-1` / `full-inputs` | 588 passed | 58.038s |
| `full-1` Ruff | pass | 0.267s |
| `full-1` format | 10 files already formatted | 0.041s |
| `full-1` strict Pyright | 0 errors, 0 warnings | 1.374s |

RED failed on the unchanged runtime for TERM-responsive exit (KILL instead of TERM), missing TERM before KILL, and failure to reap an already-retiring TERM-ignoring worker within its earlier deadline. GREEN changed the runtime and passed the same 17 controls. Full pytest includes those same seven close modes and ten existing cutoff controls. GREEN and full Python subjects match the shipped sources. Subject hashes are unchanged within every sample. The runner was subsequently formatted and extended to include itself in static checks before `full-1`; each sample retains its own runner digest and exact command declaration.

The seven close modes exercise orderly EOF, TERM-responsive, TERM-ignoring, sticky failure, previous TERM, previous KILL, and expired authority. They check signal order, phase timing, the earlier deadline, idempotence, residual retention and exact child status. In GREEN, healthy TERM was attempted after 50–52ms; ignored TERM escalated to KILL near 2s; an already-retiring owner with 400ms remaining escalated near 200ms; orderly exit took about 1.5ms without a signal. These are observed timings, not new deadline allowances. Denied-signal history is handled by the runtime but this slice does not add a real EPERM fixture.

## Ownership and preservation

The seven close controls and ten cutoff controls each retain an independent exact-child reap audit in RED, GREEN and full: 51 acknowledgements, all reaped. A later read-only process snapshot found none of those 51 PIDs present. `audit-summary.json` names every receipt and records the scope. Expired runtime authority intentionally leaves an explicit residual before the independent fixture cleanup; cleanup is not attributed to runtime success. The cutoff controls simulate deferred parent wait observations, not kernel unreapability.

`preservation.json` compares the parent commit `3d030cb6` content tree against the working tree: all 117652 prior evidence paths remain unchanged. Positive controls found the changed runtime and new k98 evidence paths. Six oversized full-suite receipts were explicitly snapshotted with a command-local Jujutsu size limit; no evidence was omitted because of the default limit. Retained artifact hashes in all three verdicts were checked after measurement.

## Reproduction

Run from the repository root, choosing a fresh OUT directory:

```sh
mise exec -- task diagnostic:worker-close-check INPUTS=evidence/worker-close-k98/full-inputs OUT=evidence/worker-close-k98/recheck-full MODE=full
mise exec -- task diagnostic:worker-close-check INPUTS=evidence/worker-close-k98/green-inputs OUT=evidence/worker-close-k98/recheck-controls MODE=controls
```

The Taskfile is the reusable entry point; `diagnostic:format` also accepts additional Python paths after `--`. Independent review of this worker-close slice is sequenced before k101. It must challenge escalation timing, earlier authority and ownership against the source and retained evidence. The future complete-runtime review of k28 remains required by k101.
