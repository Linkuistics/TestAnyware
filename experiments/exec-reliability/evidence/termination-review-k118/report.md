# k118 termination review integration

The termination owner now distinguishes an exhausted observation window from an
observer failure. A genuine observer error stops further observations and retains
its first cause. Mandatory safety signaling and the original deadlines remain in
force. All 852 tests pass on the frozen final runtime, with Ruff, format and
Pyright checks passing. This repairs the k114 seam; k116 still owns the aggregate
scheduler and teardown work.

The review source was the k117 commit
`a6260b88813a3afea44d1e37a56be9419cdccf94`, rather than this task's restatement.

## Finding decisions

| Finding | Classification and disposition | Evidence |
| --- | --- | --- |
| F1 | Real issue, repaired. Exhausted admission returns without setting an observer error. A live leader still cannot establish termination success. | Four RED cutoff cases in `red-cutoff-errors`; the same cases pass in `green-controls` and `full`. `_terminate` still requires a recorded leader exit, a healthy observer and the final group result. |
| F2 | Real test gap, repaired. Count observations on every tested turn, including later and terminal calls. Correct the review's literal mutation prediction. | Literal extra-call mutation fails 8/20 old controls (`old-double`). A later-turn-only mutation passes 20/20 old controls (`old-repeat`) and fails 2/4 strengthened deadline controls before the production edit (`red-repeat`). On the final 26 controls it fails six count assertions (`double-observe`), including the four new cutoff cases. No duplicate production observation was demonstrated. |
| F3 | Real issue, repaired. Stop after the first observer failure and retain its cause while preserving safety signaling. | Two RED OSError cases record four attempted observations instead of one. The repaired controls preserve the first cause, observe once and still send TERM/KILL. |
| F4 | Real evidence limitation, accepted visibly and handed to k116. The isolated pairs cannot support a causal comparison. | All five pairs have identical worker_capture/inspection bytes, and omit suite load. The original green-full lost-result failure remains unchanged. k116 must investigate channel close ordering and retain interleaved full-suite parent/candidate repeats against the implementation it changes. |
| F5 | Real audit gap, repaired in separate tooling. | `audit.py` checks the exact historical 13/3/2 failure sets, two child receipts per historical termination condition, and the pinned parent/candidate controller hashes in every pair. Old tooling remains unchanged. |
| F6 | Unclear scope of the inventory claim, clarified. The inventory describes a checkout, including ignored material when present. | The old 176,131-path inventory included 14,289 ignored cache files. New preservation excludes cache directories, measures this checkout's 179,670 prior evidence paths and verifies zero changes. Neither count claims clone contents. |
| F7 | Real fixture/test issues, repaired. Fake-clock controls own the yielding assertion. The real child keeps its signal/deadline/reap checks without a scheduler-latency assertion. | Exact fixture identity is published before the readiness wait. The external checker revalidates PID, PGID, start time and fixture command before exact-PID cleanup. `fixture-timeout-2` reaches the fixture and confirms KILL/absence after checker timeout. |

## Retained conditions

Every condition has a declaration, stdout/stderr, a verdict and before/after
subject hashes. Complete pytest runs also retain JUnit results. Expected RED and
mutation failures are retained as failures, not recast as green runs.

| Condition directory | Cases | Failed | Exact termination-child receipts |
| --- | ---: | ---: | ---: |
| `old-double` | 20 | 8 | 2 |
| `red-double` | 4 | 2 | 0 |
| `old-repeat` | 20 | 0 | 2 |
| `red-repeat` | 4 | 2 | 0 |
| `red-cutoff-errors` | 6 | 6 | 0 |
| `green-controls` | 26 | 0 | 2 |
| `double-observe` | 26 | 6 | 2 |
| `full` | 852 | 0 | 2 |

Both F1 and the effective F2 RED reproductions preceded the production change.
The first unconditional mutation and its rejection by the old controls are
retained too. Preparation mutations are visible in each frozen source directory.

The first fault injection, `fixture-timeout`, used two seconds and stopped during
collection: checker 98060 was reaped, but no fixture existed. It proves no fixture
cleanup. The retained ten-second `fixture-timeout-2` reached the fixture: checker
4511 was reaped after TERM, exact fixture 4577 was matched, killed and observed
absent. Normal budgets remain 60 seconds for controls, 120 for the full suite and
30 for each static checker. No increased budget concealed a stall.

The full suite completed in 83.01 seconds. Its two real termination fixtures
were reaped with -9 and observed absent. The independent fixture audit also
observed both absent. This is one passing suite sample, not a repeatability or
causal claim about the earlier transport failure.

`condition-audit.json` records the passing audit. During audit development two
expectations were corrected: the historical RED condition has two child receipts,
and the final mutant rejects six controls, not two. Neither correction changed
runtime/test inputs or any condition's receipts. The old failure identities and
all eight new complete-condition identities/counts are checked mechanically.

`inputs-final` matches the working runtime, tests, manifest, lockfile and Taskfile.
`final-source-sha256.json` records the final source/tool hashes. The full check
hashed each subject before and after and found no drift. Audit expectations were
finished after the full run; final tooling checks are separately retained in
`final-tools.stdout` and `final-tools.stderr`. The full verdict's tool hashes
remain its original hashes. `prior-before.json` and `prior-after.json` match on
179,670 cache-excluded prior paths, with zero changed/added/removed paths.

## Reproduction entry points

Use `mise exec -- task` from the repository root. The task names are
`diagnostic:termination-review-prepare`, `diagnostic:termination-review-check`,
`diagnostic:termination-review-preserve`, `diagnostic:termination-review-tools`
and `diagnostic:termination-review-audit`. Preparation/checking take INPUTS and
OUT relative to `experiments/exec-reliability`; checks also take MODE=controls or
MODE=full. MUTANT selects a preparation mutation; SELECT narrows controls;
BUDGET is reserved for timeout injection. Give each condition a fresh OUT.
Preservation before/after and the condition audit are one-shot artifacts; retain
existing outputs rather than overwriting them. Tool validation uses
`mise exec -- task diagnostic:termination-review-tools -- --check`.

## Remaining scope

k116 carries the full scheduler/worker retirement/teardown charter and the final
k28 review. Its handoff names the old lost-result failure (worker exit 70, errno
35, incomplete worker frame length), close-ordering investigation and retained
full-suite comparisons. Existing unexplained timing/setup cases also remain.
No guest or privileged operation was run. k28 and k17 remain OPEN; ten-clone guest
acceptance remains 0/10 and unavailable. This integration establishes no shared
scheduling, teardown overlap, repeatability or guest acceptance.
