> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Shared bounded-turn driver — k124

This slice routes four production compatibility consumers through `turns.run_turns`:
workload observation, workload termination, leader reap, and worker close. Each
consumer still runs synchronously. Production failure cleanup and safety teardown
remain sequential; this is not aggregate k28 acceptance.

The driver selects ready safety work before ordinary work, then earliest absolute
wake, then stable rotation at equal wakes. New owners are immediately due. It
sleeps only while no owner is ready and caps each sleep at 5 ms. Terminal owners
leave the queue; errors propagate without retry, signal, reap or finalization.
Owner callbacks retain all immutable deadlines and the lifetime first-reap rule.
A callback must yield after bounded work. Safety priority can delay ordinary work;
this is cooperative scheduling, not a hard real-time guarantee.

Keeping four loops was the smaller textual change, but leaves sleeping policy
scattered and cannot compose bounded owners. Moving signal/reap logic into the
driver would mix authority with scheduling. This slice instead shares the driver
while leaving existing owner methods unchanged. It uses existing Python primitives;
no newly version-dependent library behavior or API choice is introduced.

## Verification contract

The Taskfile `diagnostic:common-driver-*` entries prepare exclusive frozen inputs,
run independently bounded checks, inventory older evidence and audit results.
`comparison-plan.json` defines the interleaved comparison. Parent runtime means
commit `4cceab19b5058fba527ddb58a9d3b241c59b8380`'s controller and worker files with
the candidate driver, test and manifest overlay: original consumers keep their
loops, while the same 873 test identities run in both conditions. The direct
composition control uses the same driver in both; it does not establish production
concurrency. This comparison isolates changing the compatibility consumers.

The runner freezes every top-level Python source, manifest, lock, Taskfile, tool,
planning file, spec and declared binary before launch and hashes them afterward.
Checkers have 60 s focused / 120 s full / 30 s static bounds, followed by TERM 2 s
and KILL/reap 2 s on timeout. Exact fixture journals are inspected by a separate
parent process using PID, PGID, start and command identity before any cleanup.
The real composition control combines a failed worker, a healthy worker, and a
workload wait in one driver, then checks terminal reentry and explicit child reap.
Deterministic controls cover ready safety priority, overdue wake order, equal-wake
rotation, finite sleep caps, terminal removal and exception propagation.

## Retained failures and limits

- `red/` is a setup-only failure: the first runner launch lacked its planning
  files and never launched a checker. `red-inputs/` is retained.
- `red-2/` fails collection because the requested driver module is absent. This
  establishes test-first setup only, not behavioral sensitivity.
- `controls-1/` has 283 passes and two exact-clock first-reap assertion failures.
  The initial driver unnecessarily sampled the clock before dispatching an
  immediately due owner. It now skips that read when all pending owners are new;
  owner cutoff checks are unchanged. The failed input and sample remain retained.
- All five driver mutations are behavioral failures, separately retained. They
  remove safety priority, reverse overdue wake order, remove stable rotation,
  remove the sleep cap, or suppress owner errors.
- k114's lost-result errno35/incomplete-frame failure, k116's 4.944 ms late probe,
  and k121's 20.291 μs late WNOHANG remain unresolved historical observations.
  These local comparisons do not explain or repair those earlier samples.
- Graph generation `2026-11-15T02:19:52Z` records no coverage gaps, but changed
  runtime files are stale and new files are untracked. Relevant source was read
  directly; no graph-completeness or repository-wide behavioral claim is made.

The full aggregate charter and historical log are preserved in
`complete-worker-scheduler-k126`. Persistent late-control parsing, helper ownership,
failure/shutdown overlap with teardown, actual one-scheduler orchestration, full
fault inventory, privileged unavailable controls and the final k28 independent
review remain there. No k28/k17 ancestor closes on this slice.

## Results

| Condition | Tests | Outcome |
| --- | ---: | --- |
| Final focused controls | 285 | Pass |
| Parent full 1 → candidate full 1 | 873 each | Both pass |
| Parent full 2 → candidate full 2 | 873 each | Both pass |
| Five deliberate mutations | 6 each | All rejected by assertions |

Every full run also passes Ruff, format checking and strict Pyright. Both pairs
have exactly 873 common test identities, zero added/missing identities, and no
changed per-item outcome. `condition-audit.json` records the item comparisons,
all failed conditions and the evidence hashes. All measured subjects are unchanged
within each condition; final frozen sources match live runtime, tests, manifests
and Taskfile. Each full run has five independently audited fixture journals,
including the three new composition children; all are absent with zero external
cleanup signals. Other existing child controls retain their own before/after
receipts. All 195,920 older evidence paths are unchanged, including symlink targets
as recorded text. Earlier failures remain evidence, not repaired by these passes.

Independent review is scheduled immediately before the full-charter continuation.
No provider evidence was present in this session environment, so the review has
no invented Creator receipt.
