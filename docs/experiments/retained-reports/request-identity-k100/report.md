> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# k99 request-identity review integration

Both findings are accepted as test-strength issues and resolved. Runtime code and the inspection contract are unchanged.

## F1: worker token replay and first-run pinning

`test_request_identity_worker_second_command` completes one real request, then writes a hand-built command directly to the worker command descriptor. The replay case retains the first request token with a fresh deadline; the changed-run case uses a fresh token with another run ID. Each worker must exit 70 before reaching publication. A matching-run, fresh-token case must reach publication and exit cleanly. The publication hook only replaces the second publication with a marker: it creates no second helper and does not fabricate a reap receipt. Existing sequential controls still cover repeated real helper execution.

The first helper uses the production publication path. Each case retains its first receipt and command/worker audit, confirms the worker was reaped with an independent waitpid check, and confirms the first helper PID is absent. Removing either worker check independently admits the prohibited command and fails its corresponding new test.

## F2: conservative ledger after damaged control identity

The existing 11-damage matrix now asserts that rejected ready, intent, created and fork-error frames leave helper identity unset, unknown helper creation true, creation conflict true, and a nonempty residual remedy. The existing error/closed/no-publication assertions remain, as do known-helper obligations for damaged result/completion frames. Controls use real helper reap before delivering altered traffic; no synthetic death receipt is accepted. A mutant that falsely reports known creation fails all 44 strengthened control cases.

## Frozen verification

| Condition | Passed | Failed | Result |
| --- | ---: | ---: | --- |
| green-controls-1 | 85 | 0 | Pass |
| mutation-no-replay-check-1 | 84 | 1 | Replay case detects removed check |
| mutation-no-run-pin-1 | 84 | 1 | Changed-run case detects removed check |
| mutation-false-known-creation-1 | 41 | 44 | Damaged controls detect false known creation |
| green-full-1 | 582 | 0 | Full suite, lint, format and type checks pass |

Each condition uses the unchanged k96 runner through the root `diagnostic:request-identity-check` Taskfile recipe and `mise exec`. The inputs, mutation replacements and their hashes are retained in `inputs/`, `mutations.json` and `frozen-inputs.json`. Reproduce with absolute INPUTS and a new absolute OUT directory, MODE=controls or MODE=full. No condition was retried or overwritten.

Each declaration fixes the commands, 20 subject hashes and checker budgets before execution: 60 seconds for focused pytest, 120 for full pytest, 30 per static checker, then bounded TERM/KILL/reap allowances. Subjects include all nine diagnostic Python sources, configuration, lockfile, copied/root Taskfiles, runner, interpreter and external fixture tools. Every before/after subject hash agrees. Every checker was reaped without timeout. Ruff reports all checks passed and nine files already formatted; Pyright reports zero errors, warnings and information messages.

`verification.json` compares the focused cases by exact test identity across mutations and the full run, validates every retained runner artifact hash, checks frozen inputs against their initial digests, and confirms live source bytes equal frozen green. Its process audit itemizes 1,406 recorded PIDs from checker and parsed run JSON; all are absent afterward. Before/after process snapshots are retained. One intentionally malformed nested-readiness stress JSON cannot be parsed for PID extraction and is explicitly listed; it remains covered by the run artifact digest. The process audit supplements the tests' worker/helper reap and absence assertions; it is not a permission, runtime reliability or universal process-containment claim.

Nine compressed `prior-evidence-*.json.gz` inventories reconcile all 127,131 prior evidence files with no missing or changed bytes. Python/pytest cache directories were excluded from that inventory. `artifact-index.json` names the new retained evidence and its SHA-256 digests, excluding disposable caches and the index itself.

No spec or ADR change is required. complete-worker-scheduler-k98 and the parent aggregates remain open for shared scheduling, cutoff closure, overlapping shutdown and the final bounded-inspection review.
