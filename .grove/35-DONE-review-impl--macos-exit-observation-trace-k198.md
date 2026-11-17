# macos-exit-observation-trace-k198

**Reviews:** macos-exit-observation-instrumented-k195

## Goal

Adversarially review the k195 trace seam, its controls, the traced guest run
and the report claims, before a later leaf relies on them.

## Context

- Read the producer commits for `macos-exit-observation-instrumented-k195`,
  its decision log, this grove's root brief and the k195 section of
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`.
- The seam is `agents/macos/Sources/TestAnywareAgent/ExitTrace.swift`, with
  the `EXIT_TRACE` blocks in `ProcessRunner.swift`. The host controls are
  `agents/macos/Tests/TestAnywareAgentTests/ExitTraceControlTests.swift`
  and `experiments/exec-reliability/evidence/macos-exit-observation-k195/controls.py`.
- The guest tooling is `experiments/exec-reliability/exit_k195.py`,
  `exit_k195_check.py`, `exit_k195_run.sh`, `test_exit_k195.py` and the
  Taskfile entries `diagnostic:macos-exit-observation{,-controls,-check,-tools}`.
- Evidence: `controls-01`, `run-01` and `run-01-outer` under
  `experiments/exec-reliability/evidence/macos-exit-observation-k195`.

## Done when

- The default build is checked for any behaviour change from the seam.
- The seam is read for an ordering that it can record wrongly or miss. The
  read covers PID reuse, a missed kqueue registration and the clock source.
- The checker and its controls are read for failures that a passing control
  would hide. The read covers the route judgement and the call matching.
- Each claim of the k195 section is checked against the retained evidence.
- Findings are recorded in this leaf's commit. An integration leaf is cut
  only for findings worth acting on.

## Notes

Inspection only. Do not repair the tooling in this leaf. Do not run a VM.

## Decisions (running log)

The default path retains the original `waitForExit`, termination and cleanup
operations behind the `#else` branches. All new agent code is conditional on
`EXIT_TRACE`. Source inspection found no default-path behaviour change. This
review runs no builds, tests or VM operations.

The observer timestamp is an upper bound on an exit, not the exit time itself:
`ExitTrace.loop` samples `now()` after `kevent` returns and uses that time for
the entire batch. The checker nevertheless treats a late or absent exit note
as proof of liveness. This is material to the diagnostic's central question
and earns integration. The findings below are derived from committed source and
retained records, without executing the diagnostic or its tests.

## Findings

### R1 — High: late observation is not proof of liveness at the deadline

`agents/macos/Sources/TestAnywareAgent/ExitTrace.swift:105–118` samples the
clock after `kevent` returns, then gives that timestamp to every event in the
batch. It does not receive a kernel exit timestamp. An exit can occur before
the deadline while the observer is descheduled, yet be delivered afterwards.
`experiments/exec-reliability/exit_k195_check.py:170–183` classifies precisely
that case as `command-alive`, even if registration failed or no exit was
recorded at all. `cause` at lines 245–248 then states that the guest command
was still running. This can turn the suspected missed-exit mechanism into the
opposite causal conclusion. Four passing controls do not challenge delayed or
failed observation.

Record the timestamp's actual meaning and require positive evidence for
liveness; late or missing notifications must retain uncertainty. The early
notification case establishes an upper bound on exit, with the deadline also
needing a defined anchor: `ProcessRunner.swift:185–189` timestamps after the
timed wait returns, not when its deadline expires. The two clocks described
at `ExitTrace.swift:6–9` are observer marks on the same clock. Also,
`exit_k195_check.py:176` compares sequence allocation order to infer waiter
starvation, although `ExitTrace.swift:154–158` takes the timestamp before
locking for the sequence. Preemption between those operations can reverse
sequence and timestamp order. Do not turn such an ambiguous boundary into a
proved cause. Preserve the existing sample as evidence of no recurrence.

### R2 — Medium: failed watches and PID reuse can silently lose identities

`agents/macos/Sources/TestAnywareAgent/ExitTrace.swift:65–91` inserts a PID in
`known` and assigns its owner before registration. A registration failure
emits `watch-missed` but never removes that PID. There will be no exit event
to remove a failed watch. A later command assigned the reused PID is skipped
by `known`, keeps the earlier owner and has no watch of its own. Even with a
successful watch, reuse before the queued exit is handled can skip the new
registration. Events carry no registration token (`udata` is nil), and
`handle` resolves them through the current PID map at lines 122–145.

Child coverage has a further explicit limit: `adopt` at lines 94–103 only
enumerates children still present in a fixed 256-entry snapshot. A child
that forks and exits before enumeration can leave neither a watch nor a
`watch-missed` record. The report's promise to trace each child is therefore
stronger than this mechanism. Make watch ownership distinguish process
incarnations, undo failed registrations, and expose incomplete descendant
coverage rather than treating zero missed-watch records as completeness.
This is required for the producer's per-command shell/child exit evidence;
it is not a request to change production process ownership.

### R3 — High: evidence completeness ignores receipts and trace identity

`experiments/exec-reliability/exit_k195_check.py:213–238` records
`receipt_failures` but determines completeness solely from whether each trace
call has a recognised deadline classification. Removing an ordinary workload
receipt while retaining its trace leaves `timed_out=None`, no unintended
timeout and `evidence=complete, stall=not-reproduced`. An invalid intentional
timeout receipt likewise does not gate the causal interpretation. This
violates the inherited requirement for valid local evidence after a valid
intentional timeout.

The route check at lines 89–117 checks the first trace record embedded in
the verification receipt, but `check` never binds the downloaded trace to
that record or agent. `_calls` at lines 127–135 groups only by call number,
which resets in every agent process. A trace from another agent, or appended
records from multiple agents, can thus be judged as the verified run. The
greedy prefix matcher at lines 200–226 also skips unmatched calls and can
assign a later repeated command to an earlier missing call. Require an
unambiguous correspondence between valid receipts and trace identities.

Malformed lines and loop errors are counted at lines 287–298 but do not gate
the result, and sequence gaps are not checked. Missing records are especially
material given R1. `main` at lines 311–317 returns zero even for route
`unavailable`, incomplete evidence or uncertain owned cleanup, provided VM
disposal succeeded; the runner uses that exit status at
`experiments/exec-reliability/exit_k195_run.sh:83–88`. Keep unavailability,
evidence failure and cleanup uncertainty visible through the aggregate
outcome instead of allowing the runner to report success from disposal alone.
The present tests cover good receipts, basic route mismatches and a wholly
absent trace, but not these damaged or mismatched evidence cases.

### R4 — Medium: the host/guest timing comparison contradicts retained controls

`docs/experiments/exec-reliability-macos-timeout-inspection.md:310` reports
the normal host exit at 2.3 ms and agent result at 2.4 ms. Lines 358–359
contrast a guest delay of 25–65 ms with a host delay below 0.1 ms. In
`experiments/exec-reliability/evidence/macos-exit-observation-k195/controls-01/result.json`,
the three normal runs instead show exit / waiter-return / wait-result at
1.7 / 65.5 / 65.6 ms, 1.8 / 64.7 / 64.7 ms and
1.7 / 64.0 / 64.0 ms. Their roughly 62–64 ms delay overlaps the guest
delay. The table's 2.3/2.4 ms normal row occurs in none of these runs.

Correct the comparison from the retained trace files and identify the run
behind each control number. This matters to the causal inquiry because the
report currently invents a host/guest difference. The absence of reproduced
stall remains supported. The default-build claim at lines 299–300 also needs
its stated baseline comparison or narrower wording: retained `default_seam`
records a single binary digest and zero trace symbols, but no comparison to
a pre-change binary. Source inspection supports conditional exclusion of the
seam; it does not itself prove byte identity.

## Retained evidence audit and limits

- The guest trace contains 253 valid JSON records, 253 distinct sequence
  numbers, a single agent PID 703, no recorded loop/watch error and at most
  12 sampled threads. Its matching receipts report only the two intended
  timeouts. The nine other workload exec calls succeed at the agent wait;
  the report's guest lag range is consistent with those records. The
  observed thread samples do not establish a continuous upper bound.
- The route receipts identify old PID 412, new PID 703 and installed digest
  `76e76a484edd59c80075ea40982412e4e1932dc926c433a40e9c3789a05c6bcf`. All downloaded
  trace records in this actual sample have that new PID, so R3 concerns
  checker enforcement rather than evidence of a route mismatch in run 01.
- In call 3, `terminate-sent` is at 2131.6265 ms; shell PID 735 exit
  notification is at 2131.8500 ms and child PID 737 notification at
  2131.8683 ms, status 15. The child notification is 0.2418 ms after the
  mark made before sending TERM. This proves the child exited by receipt
  of that notification, not an exact exit time or which sender caused it.
  The later descendant SIGKILL returns ESRCH. The retained own observation
  and cleanup agree on absence. The report's statement that the kernel
  measured the exact exit time needs R1's qualification.
- Retained health records report accessibility denied and CLI exit 4.
  The controller summary records the owned clone disposed with no residuals.
  The outer before/after input hashes are equal and match the reviewed
  checkout inputs; development-app PID 46272 is unchanged across the run.
  The restored installed-app PID 53925 is a producer log statement; the
  outer runner records end before that restoration and do not independently
  establish it.
- All four host controls are recorded as passing in three runs. This review
  reads those records; it makes no fresh execution claim. Their wide-margin
  forced orderings do not exercise delayed notification, failed watches,
  reused identities or incomplete evidence.
- Graph lookups found no current seam symbols. Coverage generation
  `2026-11-15T02:19:52Z` reports no recorded gap but freshness `not_tracked`
  for the new files/evidence and `metadata_changed` for ProcessRunner and
  Taskfile. Material conclusions use direct source and retained-record
  reads, not graph completeness. No graph reindex was performed.

The integration belongs beside this review before the first later live
sibling, `macos-exec-outcomes-k12`: it directly unblocks safe use of k195's
causal conclusions. Findings stay in this review commit; the integration
body carries the review handle for independent triage.
