# k106 bootstrap review integration

All five k105 findings are accepted. F1–F4 are repaired and checked here; F5 is
handed to the still-live `complete-worker-scheduler-k104`. The final frozen full
condition, `full-final-4/`, passed 626 tests in 66.83 seconds, lint, format and
production/tool type checks. The current runtime and tests match those frozen
inputs. Earlier unsuccessful attempts remain intact below. This closes the
bootstrap integration slice, not bounded-inspection-k28 or the wider worker and
scheduler acceptance.

## Finding dispositions

| Finding | Resolution | Retained controls |
| --- | --- | --- |
| F1 | Store readiness `ready`, `expired`, `late`, or `failed`; preserve the valid-frame observation through failure handling. Add numerical readiness/shutdown lateness and shutdown deadline exhaustion. Strict measured overruns remain failures, including ordinary expiry. Update the specification. | Ordinary expiry, delayed wait-return, valid-frame equality and lateness, successful readiness, and fork failure. |
| F2 | Deliver an actual valid frame from a real child, then set the first parent observation clock sample to deadline minus 1ms, equality, or plus 10ms. Equality rejects readiness but has zero numerical lateness and a false strict overrun flag. | `test_declared_bootstrap_valid_frame_deadline[-0.001/0.0/0.01]`; accept-late, accept-equality and overwrite-sample mutants. |
| F3 | Assert both overrun truth values, every shutdown outcome, readiness outcomes, numerical lateness and exhaustion. | Real TERM/KILL, deferred reap, delayed observation and injected fork failure; both overrun directions and each outcome substitution. |
| F4 | Check readiness/TERM/KILL-reap equations independently against the declaration, with `rel=0, abs=1e-9`. Narrow observed KILL to 70–100ms and startup to less than 480ms for the 150/70/160ms declaration. | A 500ms readiness substitution, TERM minus 5ms/plus 40ms, and KILL-reap plus 20ms each fail. |
| F5 | Pre-readiness ownership has an infinite request cutoff and no prior request signals. Bootstrap cannot establish earlier finite request-cutoff or prior-signal preservation. | Explicit handoff in the k104 task; no bootstrap acceptance claim for those properties. |

The 30ms KILL scheduling allowance, 50ms readiness observation allowance and
100ms total startup allowance are checker allowances, not runtime authority.
Exact deadline equations also constrain those allowances. Deferred-reap lateness
must be positive and below 50ms; the injected 270ms wait-return stall must produce
at least 40ms lateness. No tolerance converts expiry into success or renews a
signal/reap deadline. Controlled parent clock samples test the branch and receipt
contract; they do not claim that the host scheduler delivers equality in real
time.

## Frozen mutation and repaired evidence

`prepare.py` copies the top-level Python sources, project configuration, lockfile
and Taskfile before execution. Each mutant replaces exactly one matching source
site in `worker_capture.py`; only that file differs from `attempt-2/inputs/`.
Every RED condition ran the same 34 bootstrap cases, exited 1 inside the checker
(Task reports 201), and rejected its mutation. Every failed case is present and
passing in `attempt-2/green/` (34/34). Inputs were not edited during a condition.

| Condition under `attempt-2/` | Failed cases / 34 |
| --- | ---: |
| red-readiness-half | 5 |
| red-late-accept | 2 |
| red-equality-accept | 1 |
| red-overwrite-sample | 2 |
| red-overrun-true | 3 |
| red-overrun-false | 2 |
| red-outcome-reaped | 2 |
| red-outcome-residual | 2 |
| red-outcome-not-created | 1 |
| red-readiness-outcome | 4 |
| red-readiness-overrun | 5 |
| red-readiness-lateness | 5 |
| red-shutdown-lateness | 2 |
| red-shutdown-expired | 2 |
| red-early-kill | 4 |
| red-late-kill | 4 |
| red-kill-reap-small | 4 |

The 17 mutants produced 50 assertion failures and 528 passes; repaired controls
produced 34 passes. Exact case identities, command exits and digest checks are in
`condition-audit.json`, with the original JUnit and raw stdout/stderr in each
condition. This is an in-session repair and verification, not an independent
adversarial review.

## Full suite and retained unsuccessful attempts

| Condition | Pytest | Ruff / format / pyright |
| --- | --- | --- |
| attempt-2/full | 626 passed, 64.85s | 0 / 0 / 1 |
| full-final | 625 passed, 1 failed, 71.25s | 1 / 0 / 0 |
| full-final-2 | 620 passed, 6 failed, 77.90s | 0 / 0 / 0 |
| full-final-3 | 625 passed, 1 failed, 74.81s | 0 / 0 / 0 |
| full-final-4 | 626 passed, 66.83s | 0 / 0 / 0 |

The first full type check exposed `batch.py`'s direct `prepare` import when checked
from the frozen subject directory. Loading the adjacent file with `runpy` fixes
that import. The next attempt exposed an import-order blank line, subsequently
removed. Runtime and test bytes did not change across the five full conditions.
`tooling-complete-2.stdout/stderr` separately retain the passing lint, format and
type checks of all five current tools, including the final audit script.

The later pytest failures were in unchanged existing controls, and none were in
the bootstrap controls:

- `full-final/`: `test_review_live_group_is_failure_even_with_expected_leader_exit[False]`
  expected the residual child in the membership list. The retained result instead
  records unknown membership after `helper capture deadline exhausted`, empty
  stdout, helper SIGKILL and reap. Termination still failed closed.
- `full-final-2/`: four late-created-identity cases raised `KeyError: 'after_term'`;
  two prebuffer controls raised `AssertionError: worker did not prebuffer the identity`.
- `full-final-3/`: `test_review_real_ps_failure_disables_later_queries_and_probes`
  received empty stderr rather than the expected `usage:` text.

These observations are handed to k104. The final pass establishes the full gate
for this frozen slice; it does not erase the failed replays or establish aggregate
repeatability. No existing timeout or unrelated test was loosened to obtain it.

Initial batch attempt 1 stopped on an expected RED because Task's exit was 201,
not the inner pytest exit 1. `inputs/`, `inputs-readiness-half/` and
`red-readiness-half/` retain that attempt (5 failures, 29 passes). The corrected
batch uses a fresh `attempt-2/` and checks both exit layers. Tool versions and
spec/Taskfile snapshots from the successive tooling repairs are retained in
`tooling-attempt-1/`, `tooling-attempt-2/` and `tooling-attempt-3/`.

The first audit traversed synthetic verdict fixtures inside test output, whose
original temporary subject paths were no longer authoritative. Its failed output
and source are retained as `audit.stdout/stderr` and `audit-attempt-1.py`. The
second audit incorrectly expected the no-child receipt even when a mutant failed
before that receipt was written; `audit-2.stdout/stderr` and `audit-attempt-2.py`
retain it. The final audit enumerates the 24 actual declared conditions, requires
all eight real-child cleanup receipts even for RED, and permits the missing
no-child receipt only when that specific case failed. For failures before the
receipt's audit PID assignment, it uses the same retained runtime owner PID.
`audit-3.stdout/stderr` retain its successful execution.

## Integrity and process ownership

`condition-audit.json` verifies 9,517 subject/artifact digests across 24 actual
conditions, unchanged before/after subjects, 17 single-file mutant copies, and
current runtime/test equality to the repaired frozen inputs. Historical tool
hashes resolve to the retained tooling snapshots. Each condition has its own
`declaration.json`, command bounds, raw streams, JUnit cases, artifact hashes and
reap status. Checkers have independent 60s control/120s full and 30s static limits,
followed by a 2s TERM and 2s KILL/reap checker cleanup window on timeout. None timed
out, and all 39 checker processes were reaped.

All 192 bootstrap real-child receipts confirm cleanup with the unfaulted exact
child wait interface. The final process inventory additionally checked 2,120
recorded workload/worker/helper PIDs from direct retained run results, plus the
checker/bootstrap PIDs; none were present. This is a bounded check of recorded
owned processes, not a claim that the machine contains no unrelated process.
The audit script records its own matching before/after source hash.

`prior-before.json` and `prior-after.json` cover 142,137 prior evidence paths,
including file bytes and symlink targets without following directory symlinks.
The after inventory reports no changed, removed or added prior path. This new
k106 evidence directory is excluded from that prior-evidence comparison. No
historical evidence byte changed.

Graph coverage at generation `2026-11-14T22:15:36Z` did not track the new artifact
and tooling paths, so material code statements used direct source verification.
No graph completeness or repository-wide absence claim is made.

## Reproduction and remaining scope

Run project commands through `mise exec -- task`:

- `diagnostic:bootstrap-review-prepare INPUTS=<source> OUT=<fresh> [MUTANT=<name>]`
- `diagnostic:bootstrap-review-check INPUTS=<frozen> OUT=<fresh> [MODE=full]`
- `diagnostic:bootstrap-review-tools`
- `diagnostic:bootstrap-review-audit` verifies this fixed 24-condition record.

The retained batch intentionally refuses to reuse its populated `attempt-2/`.
Use fresh paths to reproduce the sequence. Earlier-cutoff/history, worker-failure
and orderly-close declarations, the shared scheduler, aggregate inspection
acceptance, and the observed timing-sensitive controls remain with live k104.
The guest/ten-clone acceptance gate remains unavailable (0/10); no local test here
substitutes for it. No parent node is closed by k106.
