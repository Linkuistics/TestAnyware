# k142 expired-request review integration

The repaired control rejects false residual evidence on project Python 3.12,
checks the separate `capture_helper` receipt remedy, and exercises real ECHILD
ownership loss. Runtime modules, project interpreter requirements and the lockfile
are unchanged. This integrates k141's control findings; k140 still owns production
concurrency, actual teardown overlap and aggregate acceptance.

## Findings and decisions

**F1 — real sensitivity gap, repaired.** The retained
`baseline-false-residual/` sample runs the original controls and runner on Python
3.12: two pass, one skips, and the wrong always-present worker remedy is accepted.
The new exited schedule closes the real worker's command pipe, then observes its
exact PID in zombie state using `/bin/ps`. Each observation has a 0.5-second
timeout inside a one-second polling window. The unreaped direct child pins PID
identity. Preparation does not consume status; the runtime's first actual
`waitpid(pid, WNOHANG)` must collect it. No `waitid` path or capability skip is used.
The frozen `false-residual` mutant now fails the worker remedy assertion on 3.12
and isolated 3.15. A failed preparation remains a failed sample.

**F2 — visible trade-off, retained.** Natural exit remains unprepared and the first
real WNOHANG result determines the oracle. Waiting for a known exited state would
turn it into the separately named prepared exited schedule. The nonmutant
`candidate-controls-py312/`, `candidate-full-py312/` and
`candidate-controls-py315/` journals all record `[0, 0]`. The natural-collected
branch remains unobserved. The deterministic exited schedule covers collected
behavior, but does not establish natural-collected coverage or a production timing
guarantee. k140 must retain this limit.

**F3 — receipt sensitivity and missing-journal defects repaired; ECHILD semantics
made explicit.** Separate missing/false receipt-remedy mutants reach the
`capture_helper` assertion after the worker evidence assertion passes. The
ownership-lost schedule independently reaps a genuinely exited child before the
runtime's real wait. Runtime receives ECHILD, records `ownership_lost`, removes the
owner from signal authority, leaves `reaped` false, and retains both conservative
reconciliation remedies. Three additional frozen mutants independently reject
lost ownership suppression, invented successful reap, and retained owner registry
membership. Repeated requests, repeated close, and stale reap/signal callbacks
still acquire no new wait or signal authority.

The old `exact_child_absent: true` field was a recorded assertion, not an external
absence measurement. Completion journals now record the actual ChildProcessError
errno, exact PID, flags, operation and observation time. Its scope is **no waitable
child of the test process**, not global PID absence. A separate bounded external
`ps` observation establishes PID absence only at that observation. A present PID
or timeout remains unresolved; PID reuse is not classified and this audit never
signals an expired-request fixture.

An early start journal is written before `worker.start` and updated with its exact
PID afterward. The checker requires exactly one consistent start and completion
journal for each of four schedules, even if no journal exists. Missing, duplicate,
invalid or inconsistent records retain an unresolved remedy. The three file-based
fault controls (zero journals, start only, invalid completion) failed with the
original audit in `audit-red.txt`, and pass with the repaired audit in
`audit-green.txt` and `audit-final.txt`. These are audit fault controls, not a claim
that a killed checker can always recover an unknown PID.

## Retained verification

Every output directory is fresh. Full suites ran serially, parent then candidate.
Each checker records commands and subjects before running, subjects afterward,
artifact SHA-256 digests, bounded checker termination/reap, and independent fixture
audits. Controls have a 60-second checker budget, full pytest 120 seconds, statics
30 seconds; timeout cleanup allows TERM then KILL/reap for two seconds each.
These are checker bounds, not production cleanup acceptance.

| Sample | Retained result |
| --- | --- |
| `baseline-false-residual` | Original 3.12 false green: 2 passed, 1 skipped |
| `parent-full-py312` | 922 passed, 1 skipped; Ruff, format and Pyright passed |
| `candidate-full-py312` | 924 passed; Ruff/format passed; Pyright failed; overall verdict remains false |
| `candidate-static-py312` | Fresh Ruff, format and Pyright checks passed after correcting static inputs |
| `candidate-controls-py315` | All 4 controls passed; all 4 expired fixture audits resolved |
| `candidate-controls-py312` | Developmental 4-control pass; retained separately from final tool revisions |

The candidate full run explicitly passed `test_audit.py` to Pyright from the frozen
source directory, where its local `run_condition` import was unresolved. Its seven
type errors are retained in that sample. The correction removes only that test
from explicit Pyright inputs, following the repository's strict implementation
scope. It remains in Ruff, format and hashed subjects. Runtime and frozen test
inputs did not change; the fresh static-only run verifies the correction without
replacing the failed full receipt. Initial Ruff development errors are recorded in
`observed-deviations.json` as well.

`verify.py` validates sample artifact digests and stable before/after subjects,
compares every common JUnit case, checks the exact mutant failure schedule sets,
requires complete resolved fixture audits, compares baseline/candidate runtime
sources and configs, and checks the prior-evidence manifests. Its retained
`verification.json` confirms 920 common cases passed in both full suites, all 924
candidate cases passed, and 276,908 prior evidence paths were unchanged. The three
old control cases became four, accounting for the test-count change. Per-run
subject stability does not mean tool revisions were identical across samples;
commands/hashes retain that distinction and earlier developmental results are not
silently promoted to final-tool receipts.

All ten primary frozen mutants use Python 3.12 and run all four schedules. Each
exits pytest 1 at the expected assertions, with all four fixture audits resolved
and no checker timeout. Six supplemental mutants repeat on isolated 3.15 with the
same rejection sets. No 3.15 full suite was run in this leaf.

| Frozen mutant | Failing schedules | Rejecting assertion |
| --- | --- | --- |
| `no-first-reap` | all four | first real wait count |
| `repeat-reap` | stopped, natural | wait count after repeated/stale operations |
| `no-residual` | stopped, natural, ownership-lost | worker remedy |
| `false-residual` | exited | worker remedy |
| `expired-signal` | stopped, natural | forbidden runtime fork/signal |
| `no-receipt-residual` | stopped, natural, ownership-lost | receipt remedy |
| `false-receipt-residual` | exited | receipt remedy |
| `no-ownership-lost` | ownership-lost | ownership_lost state |
| `false-echild-reaped` | ownership-lost | actual collection state |
| `retain-lost-owner` | ownership-lost | owner registry membership |

The supplemental six are `false-residual`, both receipt mutants, and all three
ownership-loss mutants. Inputs are retained in `mutant-<name>-inputs/`; results
are `mutant-<name>-py312/` and, where applicable, `mutant-<name>-py315/`.

## Reproduction

From the repository root, choose unused names for every output. Commands below
resolve INPUTS and OUT from `experiments/exec-reliability`:

```sh
mise exec -- task diagnostic:expired-review-prepare INPUTS=. OUT=evidence/expired-request-review-k142/replay-inputs
mise exec -- task diagnostic:expired-review-check INPUTS=evidence/expired-request-review-k142/replay-inputs OUT=evidence/expired-request-review-k142/replay-controls MODE=controls
mise exec -- task diagnostic:expired-review-check INPUTS=evidence/expired-request-review-k142/replay-inputs OUT=evidence/expired-request-review-k142/replay-full MODE=full
mise exec -- task diagnostic:expired-review-prepare INPUTS=. OUT=evidence/expired-request-review-k142/replay-receipt-inputs MUTANT=false-receipt-residual
mise exec -- task diagnostic:expired-review-check INPUTS=evidence/expired-request-review-k142/replay-receipt-inputs OUT=evidence/expired-request-review-k142/replay-receipt MODE=controls
mise exec -- task diagnostic:expired-review-audit-controls
mise exec -- task diagnostic:expired-review-verify OUT=evidence/expired-request-review-k142/verification-replay.json
```

Add `PYTHON=3.15` for isolated supplemental checks. The receipt mutant command is
expected to return failure. Keep new samples and their failures; do not overwrite
the retained manifests or any old directory. `verify.py` is specific to this
retained comparison, rather than a discovery tool for arbitrary future samples.

## k140 obligations remain

Read the original k137 report and k141 review alongside this report. All prior
evidence, including failed and uncertain samples, remains unchanged. The original
3.15 parent failures (blocked-cleanup disposal, expected interruption continuation,
group-snapshot capture and competing-turn fixture ps timeout), candidate failures
(group-kill-before-inspection hang/ignore-term and zombie/EPERM classification),
overlapped runs and four original audit timeouts are still unexplained obligations
of k140. Later serial passes and supplemental absence checks do not erase them.

This leaf supplies no concurrent production-owner acceptance, actual teardown
overlap, or final aggregate review. k140 retains the production scheduler and final
k28 review. k28 and k17 remain open; guest acceptance remains 0/10 unavailable.
