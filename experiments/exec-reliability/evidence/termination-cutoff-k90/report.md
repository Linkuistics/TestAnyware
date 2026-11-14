# Termination cutoff review integration (k90)

This integrates the four findings in the k89 review commit
5539d5c00ad5a0adb503e988249d295d7fc02081, against the k85 producer
f5a081e. The governing cutoff is K-R, with R=min(100ms,A/4), from
`docs/specs/diagnostic-process-inspection.md` lines 201–245.

## Dispositions

1. **Real runtime issue:** signal admission used K, allowing KILL in the
   reserved reap tail. `_terminate.send` now compares its admission sample with
   `inspection_deadline` (K-R), including equality. New controls cover both PID
   and group scopes at K-R and inside the reserved tail. Late observer-return
   expectations now require no KILL after authority closes.
2. **Real evidence wording issue:** k85's six mutations cover three deadline
   comparisons (signal admission, observer entry and observer return) and their
   equality variants. They do not independently remove early deadline
   initialization, the observed-exit short circuit or each sticky
   `observer_ok=False` assignment. The current foundation document now says
   exactly that; the historical k85 report and evidence bytes are preserved.
3. **Real receipt issue:** `signals[].monotonic` now records the same clock read
   used by signal admission. The stepping-clock control detects the old extra
   clock sample in both scopes. This remains sampled authority: OS preemption
   between the clock read and syscall is outside this guarantee.
4. **Real runner issue:** a k90 successor uses argparse and accepts exactly
   `controls` or `full`. Missing, unknown and extra modes exit 2 before output
   creation. The reusable Taskfile command now selects this successor. The k85
   runner stays unchanged.

The original K, earlier PID cap, observed-exit short circuit, sticky observation
failure, observer entry/return checks and termination success predicate remain
unchanged. No contract clarification or substantial redesign was needed.

## Retained measurements

All paths below are relative to this directory. Runtime red controls were
written, frozen and executed before editing runtime code. Each condition has
its own declaration, raw streams, JUnit, verdict, before/after subject hashes
and retained diagnostic receipts. Inputs were never edited during a condition.

| Condition | Outcome | Meaning |
| --- | --- | --- |
| `red-1`, `red-inputs` | 8 failed, 12 passed | Original runtime: six late-signal/late-return failures and two admission-timestamp failures |
| `green-1`, `green-inputs` | 20 passed | Both runtime fixes |
| `outer-deadline-1`, `outer-deadline-inputs` | 6 failed, 14 passed | Restore only signal comparison against K |
| `early-receipt-1`, `early-receipt-inputs` | 2 failed, 18 passed | Restore only the separate earlier receipt sample |
| `green-2`, `green-inputs` | 20 passed | Final runner, unchanged fixed runtime/tests |
| `mode-red-1` | 3 failed | Old mode handling silently accepts missing, misspelled and extra modes |
| `mode-green-1` | 3 passed | Successor exits 2 and creates no output for each invalid mode |
| `full-1` | 454 passed | Full diagnostic self-check; lint, formatting and strict types also pass |

The cutoff and timestamp defect-restoration failure sets are disjoint; their
union equals the original eight failures. The six original k85 comparison
mutants were not rerun or broadened into independent evidence for other guards.
`run_condition-green-1.py` preserves the exact runner bytes measured by green-1
before Ruff formatting; its recorded hash is verified. The final runner's
formatting and lint checks pass separately. The old mode runner is preserved as
`run_condition-before-mode-fix.py`. Mode-red child evidence originally written
under pytest's temporary directory is copied beneath mode-red-1 without changing
the captured streams. The two mode-only invocations retain JUnit and raw
streams but did not capture pre-dispatch test-source hashes; do not give those
runs the stronger input-identity claim made for the six measured conditions.

`audit.py` checks prior evidence hashes, retained subject/artifact hashes, exact
case sets and measured checker timeout/reap receipts. Its first failed version
and traceback are retained as `audit-before-taskfile-map.py`, `audit-red.stdout`
and `audit-red.stderr`: the auditor initially compared red-1's original root
Taskfile hash with the changed live Taskfile. The correction maps that subject
to the exact frozen `red-inputs/Taskfile.yml`; no measurement was overwritten.
The successful audit verifies all 101,691 enumerated prior evidence files
unchanged (cache directories excluded by the original manifest), and is retained
as `audit.json` and `audit-green.*`.

Reproduce with fresh OUT names:

```sh
mise exec -- task diagnostic:termination-cutoff-check INPUTS=experiments/exec-reliability/evidence/termination-cutoff-k90/green-inputs OUT=<fresh-directory>
mise exec -- task diagnostic:test -- evidence/termination-cutoff-k90/test_runner_modes.py
mise exec -- task diagnostic:self-check OUT=<fresh-directory>
```

The audit uses exclusive output creation, so copy it and the evidence together
or use a fresh audit output name when repeating it. Its assertions compare the
retained original absolute measurement paths; it is a local identity audit.

## Aggregate boundary

The mandatory first exact-child nonblocking reap attempt when K has already
expired remains work for **complete-worker-scheduler-k88**. `_reap_leader` was
not changed here. The persistent worker, shared scheduler, all remaining
probe/reap/ownership controls and mandatory final aggregate review remain in
that leaf with its charter unchanged. k28/k17 remain open. Earlier intermittent
full-suite permission/empty-PID fixture failures remain unresolved; this passing
sample does not prove their repair. The real direct-child control simulates a
delayed observer return, not an uninterruptible kernel operation. Its two-argument
fake snapshot does not exercise the real snapshot method's full signature.
No guests, installed app or privilege changes were made. Ten-clone acceptance
remains **0/10, unavailable**.
