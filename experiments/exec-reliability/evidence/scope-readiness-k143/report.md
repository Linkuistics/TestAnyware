# TERM before child readiness

The scope-parent fixture now honours TERM while it waits for its child to declare readiness. The existing controller grace remains 150 ms.

Previously, the readiness loop ignored the stopping flag for up to four seconds. A held child proves this defect without depending on normal child startup timing. The parent publishes its exact child PID before the test sends TERM. The child never declares readiness. The parent must exit successfully within the existing grace.

The control covers both PID-only TERM and process-group TERM. An independent test owner releases the child and audits both exact PIDs after each case. The retained audits include signal time, parent return code, forced cleanup and final absence observations.

The fixture publishes the child PID through a temporary file and atomic replace. This prevents the test from reading an incomplete PID declaration. Source inspection found this instrumentation race after the first passing gate. No observed failure establishes that race. The original samples remain intact.

## Frozen comparison

All conditions use locked, isolated Python 3.12. The checker budgets remain 60 seconds for controls, 120 seconds for the full suite and 30 seconds for static checks. Its TERM and KILL/reap budgets remain two seconds each. The fixture preparation and cleanup bounds also remain unchanged.

| Condition | Selected controls | Full suite | Audit-tool controls | Static checks |
| --- | --- | --- | --- | --- |
| red-controls | 2 failed, 19 passed | — | — | — |
| candidate-controls | 21 passed | — | — | — |
| candidate-full | — | 941 passed, 83.85 s | 12 passed | Passed |
| red-v2-controls | 2 failed, 19 passed | — | — | — |
| candidate-v2-controls | 21 passed | — | — | — |
| candidate-v2-full | — | 941 passed, 87.60 s | 12 passed | Passed |

Both red conditions fail the two new cases with `subprocess.TimeoutExpired` at `parent.wait(timeout=0.15)`. The final red condition restores only the old loop in the otherwise final fixture. Both red conditions require forced parent cleanup. Their independent audits find both identities absent afterwards.

The final selected controls record successful parent exits after 0.043936542 seconds for PID-only TERM and 0.010750958 seconds for group TERM. Neither case requires forced cleanup. Their independent audits find both identities absent. These observations establish the controlled regression. They do not establish a general timing guarantee.

Per-case JUnit comparisons show the same 21 selected cases in both red conditions and both candidate conditions. Only the two new cases change from failure to success. Both full conditions contain the same 941 passing cases. Every condition records identical subject hashes before and after measurement.

The final full gate also passes Ruff, formatting, strict Pyright and all 12 checker controls. Its checker and fixture audits pass. The preservation inventory covers 289,810 earlier evidence paths and reports no changes. Its before-inventory SHA-256 is `1b866aafbb86b9e7c2e615126b8886d9091f0f7071eded373814e741d4e49443`.

## Provenance and reproduction

Each condition directory retains its verdict, subject hashes, checker receipts, logs, JUnit output and fixture evidence. The frozen input directories retain every root diagnostic Python source, the manifest, lockfile and Taskfile. `comparison-plan.json` declares condition order and fixed bounds. `observed-deviations.json` records the atomic publication correction. The v1 copies retain the earlier plan and deviations. `prepare.v1.txt` retains the original preparation script as Python source provenance.

The preparation, checker and audit controls derive from `../caller-cleanup-k145/`. The preservation tool derives from `../caller-scheduling-k140/preserve.py`. The preparation tool adds the `ignore-scope-stop` mutation. The checker supports the declared case selection. Prior evidence tools remain unchanged.

The repository Taskfile provides these commands:

```sh
mise exec -- task diagnostic:scope-tools
mise exec -- task diagnostic:scope-preserve MODE=before
mise exec -- task diagnostic:scope-prepare INPUTS=. OUT=evidence/scope-readiness-k143/new-inputs
mise exec -- task diagnostic:scope-prepare INPUTS=. OUT=evidence/scope-readiness-k143/new-red-inputs MUTANT=ignore-scope-stop
mise exec -- task diagnostic:scope-check INPUTS=evidence/scope-readiness-k143/new-red-inputs OUT=evidence/scope-readiness-k143/new-red-controls MODE=controls 'SELECT=request_turns_yield_with_transport_priority_and_caps or caller_driver or scope_parent_term_before_child_readiness'
mise exec -- task diagnostic:scope-check INPUTS=evidence/scope-readiness-k143/new-inputs OUT=evidence/scope-readiness-k143/new-controls MODE=controls 'SELECT=request_turns_yield_with_transport_priority_and_caps or caller_driver or scope_parent_term_before_child_readiness'
mise exec -- task diagnostic:scope-check INPUTS=evidence/scope-readiness-k143/new-inputs OUT=evidence/scope-readiness-k143/new-full MODE=full
mise exec -- task diagnostic:scope-preserve MODE=after
```

Preparation and measurement require unused output directories. Preservation uses exclusive files, so its existing records also refuse replacement. The retained records are the evidence for this delivery.

## Remaining work

This change does not establish the cause of k145's historical `assert -9 == 0` failure. That receipt lacks a child-readiness declaration. The historical failure and every earlier unresolved failure remain open.

The concurrent production scheduler, overlapping teardown, complete aggregate controls and final independent k28 review remain in complete-worker-scheduler-k146. The entire original charter and history continue there verbatim. k28 and k17 remain open. Guest acceptance remains 0/10 unavailable.
