# Incremental shutdown review integration (k115)

The k113 review identified one misleading runtime evidence field and three
control/contract gaps in k110's incremental shutdown slice. They are reconciled
here. The earlier unsupported process-state sample remains unexplained.
Shared controller scheduling, concurrent cleanup, aggregate controls and final
k28 review remain with k114. k28/k17 stay open; ten-clone acceptance remains
**0/10, unavailable**.

## Findings and decisions

| Finding | Classification | Resolution |
| --- | --- | --- |
| F1: overrun mode bypassed the incremental sleep guard | Real control issue | The injected delayed `waitpid` uses the captured original sleep. Every incremental begin/advance turn now retains the sleep guard, including overrun mode. |
| F2: a late first turn can send TERM and KILL together | Visible trade-off; late-caller contract was unclear | Preserve the immutable phase deadlines. The spec now explicitly describes overdue escalation, lost effective TERM grace and strict failure accounting. No fresh grace or signal authority is granted. |
| F3: evidence claimed closed authority during shutdown | Real runtime issue | Emit the terminal residual remedy only after `shutdown_finished`. Started/finished timestamps and the terminal receipt distinguish progress from completion. |
| F4: exact-clock tests drove only the blocking consumer | Real control gap | Add direct begin/advance controls using the real owner/deadline guards, exact grace/final cutoffs and returned wake boundaries. |
| F5: prior full run failed on an unsupported ps state | Real unresolved repeatability issue | Ten isolated samples on each declared revision passed; the earlier failed sample remains retained and unexplained for k114. |

Only the terminal-remedy predicate changes runtime behavior. Neither the parser,
phase budgets, signal/reap authority nor timing allowances were relaxed.

## Retained conditions

Every run has an exclusive output directory containing declarations, raw checker
stdout/stderr, JUnit cases, hashes and verdicts. Failures remain present.

| Condition | Cases | Result |
| --- | ---: | --- |
| [initial red](red-evidence/verdict.json) | 29 | 29 fail: one expected remedy failure and 28 fixture float-equality errors |
| [corrected red](red-corrected/verdict.json) | 29 | Expected remedy failure; 28 boundary cases pass before the runtime fix |
| [focused green](green-controls/verdict.json) | 117 | All pass |
| [drain-loop mutation](drain-loop/verdict.json) | 3 | All reject sleeping inside the overrun incremental turn |
| [KILL-wake mutation](wake-kill/verdict.json) | 28 | Four expected failures at a wake crossing the grace deadline |
| [final-wake mutation](wake-end/verdict.json) | 28 | Eight expected failures crossing grace or final deadlines |
| [self-check](self-check/verdict.json) | 826 | All pass; Ruff lint/format and strict Pyright pass |
| [review controls](review-controls/verdict.json) | 238 | All pass |
| [ordering comparison](ordering-outcomes.json) | 10 parent + 10 k110 | All pass; no causal attribution established |

The first red fixture used decimal 0.07/0.16 phase durations and incorrectly
compared their floating-point sum to literal 100.23. Corrected controls use
exactly representable 0.125/0.25 durations, retaining the original failed run.
The 28 direct clock controls cover failure/orderly shutdown and both unused and
already-consumed first-reap authority. The first turn occurs at phase start,
just before KILL, exactly at KILL, 3 ms after KILL, just before the final cutoff,
exactly at the final cutoff or 3 ms after it. They verify one-or-zero exact-child
polls, signal history, clamped next wakes, immutable deadlines, strict phase
timing and terminal idempotence. OS calls and clock are simulated in these
controls; they do not claim real-child lifecycle evidence.

The existing real-child matrix remains 84 cases (14 modes, three samples,
blocking/incremental drivers). Delayed syscall return and deferred reap are
explicit simulations; denied-attempt histories are not real privilege evidence.
Fixture cleanup independently uses original OS functions and checks its named
child. The drain-loop mutant is the review's specific regression: a private
sleeping loop before the incremental turn. The wake mutations remove the KILL
clamp and all deadline clamps respectively.

## Ordering comparison and limits

[repeat_ordering.py](repeat_ordering.py) reconstructed all eleven Python files,
manifest, lockfile and Taskfile directly from parent `1e8cbf4d` and k110
`cbd2aace`. It recorded [both byte sets and the fixed ten-sample count](ordering-declaration.json)
before running any sample. Each sample is a separate bounded checker process
selecting only `test_review_group_kill_precedes_inspection[ignore-term]`.
All twenty passed. This comparison ran before the final full/review suites.

The historical k110 [failed full run](../incremental-shutdown-k110/green-full/verdict.json)
and its raw `30416 30416 ?E` table remain unchanged. The unsupported state was
observed before worker close. Passing isolated samples neither explain it nor
establish absence of regression or aggregate repeatability. There was no retry
until success, parser relaxation or exclusion of failed outcomes.

## Reproduction and integrity

Use `mise exec -- task`. Prepare a new frozen input directory with
`diagnostic:worker-reap-scope-prepare INPUTS=. OUT=evidence/incremental-review-k115/<new-inputs>`.
Use `diagnostic:worker-reap-scope-check INPUTS=<inputs> OUT=<new-output> MODE=controls SELECT=<expression>`.
The red expression is `incremental_shutdown`; focused green selects
`incremental_shutdown or declared_shutdown`. Mutation preparation is retained
in [prepare_mutations.py](prepare_mutations.py), which requires fresh output
paths; each mutation changes exactly one checked source site. Historical
comparison preparation and execution are retained in `repeat_ordering.py`.
Invoke those scripts through the project's locked `uv run` under `mise exec`.

Required live checks use `diagnostic:self-check OUT=<new-root-relative-output>`
and `diagnostic:review-controls OUT=<new-root-relative-output>`. Evidence tools
also pass Ruff lint/format and Pyright with the diagnostic project config.
Type annotations in the comparison script were completed after its samples;
the recorded historical input bytes and checker artifacts are unchanged.

[audit.py](audit.py) verifies all 28 conditions, checker reap/no-timeout status,
artifact hashes, fixture cleanup receipts, frozen/live source equality and the
prior evidence inventory. [preserve.py](preserve.py) inventories previous
evidence without following directory symlinks. The resulting
[condition audit](condition-audit.json) and [prior comparison](prior-after.json)
record their exact counts. The runner retains its declared 60-second focused,
120-second full-suite and 30-second static checker bounds, with bounded
TERM/KILL cleanup. No guests, privilege, installed changes or unrelated
processes were used. No aggregate completion is claimed.

The final audit verifies 3,219 read subjects, 224 named exact-child cleanup
receipts and all 172,739 previous evidence paths unchanged. All checker
processes were reaped without timeout.
