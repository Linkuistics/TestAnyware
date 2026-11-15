# Request wake review integration (k131)

The k130 review exposed a test-sensitivity gap: the request transport already
returned the intended wake times, but its real transport control did not check
them. k131 adds those assertions and a deterministic callback case. Production
code is unchanged. Six focused cases and the full 881-case diagnostic suite pass;
four single-site wake mutations are rejected. This establishes wake sensitivity
for this seam, not concurrent ownership or aggregate acceptance.

## Finding dispositions

| Finding | Classification | Resolution |
| --- | --- | --- |
| F1: unchecked request wakes | Real test issue; no demonstrated runtime defect | Assert ready/immediate and idle/+5 ms query wakes, clipped to the original query cutoff, and retirement wakes within 1 ms. Exercise exact cutoffs and terminal callbacks. Freeze and reject idle-spin and stale-KILL mutations, plus ready-delay and slow-retirement mutations. |
| F2: readiness observation latency | Visible trade-off | k129's opening handoff records that data arriving just after a zero-timeout poll may wait up to 5 ms for observation; the former blocking select could wake immediately. Concurrent-owner readiness wake decisions remain there. No control here merely restates that sleep interval. |
| F3: persistent command dispatch write | Real remaining blocking boundary, handed off | k129 now explicitly includes `_write(worker.command, command, query_deadline)` before `run_turns`. A command is capped at 4096 bytes, but backpressure can keep its 5-ms select waits running through the query deadline. No dispatch rewrite was made here. |

Findings were read from review commit `d6b2dea2` and checked against the active
callbacks in `worker_capture.py`. The graph's generation was
`2026-11-15T02:19:52Z`; coverage reported changed metadata for that runtime and
untracked metadata for `test_turns.py` and `turns.py`. Targeted source reads were
used rather than interpreting empty graph lookups as missing symbols.

## What the controls observe

The existing real standalone/persistent × success/stalled-creation cases still
exercise the actual pipes, fork, framing, capped reads, safety priority and
external exact-fixture cleanup audit. Their callback wrapper now brackets the
returned wakes with the monotonic clock. Ready query work is immediately due;
idle work is due in 5 ms, clipped to the original query cutoff. Retirement is
due within 1 ms, clipped to the immutable outer deadline. Existing repeated
terminal-call checks still reject further reads or polls.

The new exact-clock case calls the real `capture_helper` callbacks while
simulating only fork, wait, signal and channel readiness. It creates no child
and sends no real signal. Start 10 and deadline 11 yield a query cutoff of
10.95: idle returns 10.005, ready returns 10, and an idle call at 10.949 returns
10.95. Terminal query calls return None without I/O.

Retirement starts at 10.95. Its first wake is 10.951 and its original KILL time
is 10.9625. At 10.962 that earlier KILL time wins. After observing reap or ECHILD
before KILL, the test advances to 10.963 while cleanup remains open. The wake
must be 10.964, not the stale 10.9625. At 10.9995 it clips to 11; repeated calls
at 11 return None without I/O. Unknown helper creation remains explicit. These
simulated cases prove callback choices; they do not measure real scheduler
latency or concurrent owners.

## Retained results

Assertions were written before the red controls. Every condition has exclusive
frozen inputs and output; no failed attempt was replaced.

| Condition | Outcome | Intended cause |
| --- | --- | --- |
| `red-idle` / `inputs-idle-spin` | 6 failures | Returning now for an idle query violates the +5 ms lower bound. All four real cases fail; exact-clock traces show 10 instead of 10.005. |
| `red-stale` / `inputs-stale-kill` | 2 failures, 4 passes | Keeping `kill_at` after reap/ECHILD returns 10.9625 instead of 10.964 with cleanup open. |
| `controls-verified` / `inputs-verified` | 6 passes | Unmodified runtime satisfies all focused wake assertions. |
| `mutant-ready` / `inputs-ready-delay` | 6 failures | Adding 5 ms even when ready violates the immediate-wake upper bound; exact-clock trace shows 10.005 instead of 10. |
| `mutant-retirement` / `inputs-slow-retirement` | 4 failures, 2 passes | A +5 ms retirement poll violates +1 ms; exact-clock result is 10.955 instead of 10.951. |
| `candidate-full` / `inputs-verified` | 881 passes in 76.94 s | Complete diagnostic suite, no failures or skips. Ruff, format and strict Pyright also pass. |
| `static` / `inputs-verified` | All three checks pass | Separate retained Ruff, format and Pyright receipts. |

Every failed test, by name:

- `test_request_turns_yield_with_transport_priority_and_caps[False-False]`
  fails in red-idle and mutant-ready.
- `test_request_turns_yield_with_transport_priority_and_caps[False-True]`
  fails in red-idle and mutant-ready.
- `test_request_turns_yield_with_transport_priority_and_caps[True-False]`
  fails in red-idle, mutant-ready and mutant-retirement.
- `test_request_turns_yield_with_transport_priority_and_caps[True-True]`
  fails in red-idle, mutant-ready and mutant-retirement.
- `test_request_turns_wakes_avoid_idle_spin_and_stale_retirement[False]`
  fails in all four mutations.
- `test_request_turns_wakes_avoid_idle_spin_and_stale_retirement[True]`
  fails in all four mutations.

For the two query mutations, the exact-clock test's initial query assertion
unwinds into the production retirement `finally`, which then also fails its
phase-start expectation. JUnit retains both assertions in the traceback. The
four real query cases fail directly on their wake bounds; they do not rely on
that secondary fixture failure. Stale-KILL and slow-retirement fail directly on
the exact returned retirement wake.

Before freezing, the new audit tool's first typecheck found four unknown-type
diagnostics (`append`, `startswith`, its generator argument and `line`). Explicit
list/dictionary annotations fixed those tooling diagnostics. The final
`diagnostic:request-wakes-tools` run passes formatting, Ruff and Pyright. No
candidate runtime failure was omitted or relabeled.

## Provenance, bounds and reproduction

Use repository-root tasks through mise:

```sh
mise exec -- task diagnostic:request-wakes-prepare INPUTS=. OUT=evidence/request-wakes-k131/NEW-inputs
mise exec -- task diagnostic:request-wakes-check INPUTS=evidence/request-wakes-k131/NEW-inputs OUT=evidence/request-wakes-k131/NEW-output MODE=controls
```

Prepare defaults to unchanged inputs; `MUTANT=idle-spin`, `stale-kill`,
`ready-delay` or `slow-retirement` applies exactly one unique replacement.
`MODE=full` runs the complete diagnostic suite and static checks;
`MODE=static` runs static checks only. Always choose fresh directories. The
preservation and audit tasks also write exclusively; their retained outputs
belong to this run and must not be overwritten for reproduction.

Each condition digests all frozen Python sources, project manifest/lockfile,
frozen and root Taskfiles, runner, plan, deviations, prepare/preserve/audit tools,
diagnostic spec, interpreter and `/bin/dd`, `/bin/sh`, `/bin/sleep`, `/bin/ps`,
`/usr/bin/false`, `/usr/bin/true` before and after. Each before/after pair matches.
Receipts hash declarations, stdout/stderr, JUnit and fixture artifacts. Controls
have a 60 s checker bound, full tests 120 s and static checks 30 s; the independent
checker retains a 2 s TERM and 2 s KILL/reap fallback. No condition timed out.

All checker children were reaped. The checker independently revalidated every
owned fixture journal's exact PID/PGID/start/command and found each absent;
none needed a remedial signal. The deterministic test's fake PID creates no
OS ownership claim. No worker reap discharges unknown helper creation.

`condition-audit.json` checks all seven verdicts, artifact hashes, nonpass
test identities, the focused-to-full item mapping, unchanged frozen candidate
sources, and exactly one runtime-file mutation hunk per mutant. The prior-evidence
inventory compares 214670 paths and reports no changed, added or missing prior
path (`prior-before.json`, `prior-after.json`). All earlier samples, mutants and
receipts remain byte-for-byte intact.

## Remaining scope

k129 retains concurrent production owners, readiness wake design, persistent
dispatch, teardown overlap and the final k28 review. k28/k17 remain open. Earlier
k85/k106/k114/k116/k121/k122 timing or fixture failures remain retained and
unresolved by this passing replay. Real privileged ps KILL/UID EPERM controls
remain unavailable without separately authorized prerequisites. No privilege,
guest, installed-app or unrelated-process work occurred. Ten-clone acceptance
remains 0/10 unavailable; storage stall stays in k18.
