# macos-timeout-inspection-k192

**Reviews:** macos-timeout-inspection-k4

## Goal

Adversarially review the k4 post-timeout inspection diagnostic and its claims
before the remainder leaf k193 builds on them.

## Context

- Read the producer commits for `macos-timeout-inspection-k4`, its decision
  log, this grove's root brief and
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`.
- The tooling is `experiments/exec-reliability/timeout_k4.py`,
  `timeout_k4_check.py`, `timeout_k4_run.sh`, `test_timeout_k4.py` and the
  Taskfile entries `diagnostic:macos-timeout-inspection{,-check,-tools}`.
- Evidence: `experiments/exec-reliability/evidence/k4-timeout-macos-01*`
  (refused before acquisition) and `k4-timeout-macos-02*` (the unchanged clone).

## Done when

- Each claim of the report is checked against the retained evidence. The
  checks cover the stall of `after-09-table`, the timeout overheads, the
  kill of the issue child and the clone disposal.
- The checker revision made after run 02 is judged for post-hoc bias.
- The plan, the checker and the owned cleanup are read for failures that a
  passing control would hide.
- Findings are recorded in this leaf's commit. An integration leaf is cut only
  for findings worth acting on.

## Notes

Inspection only. Do not repair the tooling in this leaf.

## Findings

All five findings below meet the acceptance/material-risk threshold. They concern
the diagnostic's conclusions and the inputs inherited by k193, rather than a
production ownership or cancellation design.

1. **P2 — Restore the original r3 online invocation before treating this plan as
   the original fixture condition.**
   `experiments/exec-reliability/timeout_k4.py:185` invokes only
   `bash /tmp/vmfit-k15/guest.sh online`, with a 30 s guest budget. The unchanged
   fixture reads URL and hash arguments at
   `experiments/exec-reliability/evidence/macos-timeout-inspection-k4/fixture-r3/guest.sh:60`
   and `:62`. The retained original
   `/Users/antony/Development/AgenticOrchestration/Ravel-Strategies.gh-issue-1/experiments/tutorial-vm-fit/evidence/macos-03/clone-1/online.json`
   records both pinned arguments and a 120 s guest budget. Run 02's
   `experiments/exec-reliability/evidence/k4-timeout-macos-02/6de306c947a64319a961c2da30706979/r3-online.stdout`
   instead records exit 1 and
   `/tmp/vmfit-k15/guest.sh: line 60: $2: unbound variable`.
   This is an additional, already reached plan defect before cancellation; copying
   the fixture bytes did not preserve its invocation. Correct the plan's inputs
   and original bounds, record this failed sample, and check the invocation
   against the retained original before k193 claims the complete r3 condition.

2. **P2 — Judge guest outcomes, rather than accepting successful host transport.**
   `experiments/exec-reliability/timeout_k4_check.py:165` checks only receipt
   `ok` for most guest roles; `:196` records the expected exits 23/71 without
   comparing them. Ordinary commands accept any exit when `timed_out` is false;
   r3 calls generally do not even require `timed_out` to be false. Thus the actual
   online exit 1 above is absent from revision 2's failed samples. The healthy
   fixture at `experiments/exec-reliability/test_timeout_k4.py:31` also supplies
   exit 0 for both intentional nonzero cases, yet `:85` expects complete evidence.
   Health and transfer roles bypass the guest receipt check, and downloaded
   fixture bytes are not compared with the pinned expected bytes. A full run can
   therefore be labelled complete/not-reproduced without executing the expected
   workload successfully. Validate typed receipts and each role's expected
   numeric/timeout/health/transfer outcome, including original artifact bytes;
   add adverse controls for wrong outcomes rather than relying on renamed healthy
   host-call records.

3. **P2 — Failed inspections must leave child state unknown, including cleanup.**
   `experiments/exec-reliability/timeout_k4_check.py:113` defaults a printed PID
   with no following row to absent. At `:177`–`:190` that is changed to unknown
   only for a stall, even when host failure, cut streams or a malformed receipt
   have already made the sample invalid. A failed observation with a retained
   `child pid=N` prefix can therefore still report absence. The unchanged r3
   observation masks `ps` failure with `|| true` at
   `experiments/exec-reliability/evidence/macos-timeout-inspection-k4/fixture-r3/guest.sh:81`.
   The new owned cleanup has the same loss of evidence:
   `experiments/exec-reliability/timeout_k4.py:105` pipes `ps` through `sed`,
   treats empty/failed output as a skip, and `:115` prints cleanup complete
   unconditionally. Its KILL branch also does not confirm disappearance.
   Preserve inspection errors separately from a confirmed no-match, require
   valid complete evidence before reporting absence, and expose uncertain child
   cleanup instead of unconditional success. Preserve the original r3 fixture
   bytes when accounting for its limitations. Independent clone disposal remains
   required and is supported for run 02; it does not prove earlier child absence.

4. **P2 — A reproduced stall needs a valid local sample, even when the rest of
   the run is incomplete.**
   `experiments/exec-reliability/timeout_k4_check.py:247` returns reproduced
   whenever any sample sets `stalled`, regardless of that sample's receipt,
   stream or host failures. For example, an ordinary receipt with `ok: false`
   and `timed_out: true` both fails the receipt check and establishes reproduction;
   a failed observation over 30 s also establishes it without a valid guest
   outcome. Run-level provenance failures are reported but cannot invalidate
   reproduction either. This lets a damaged reading or a host-side deadline
   establish the guest phenomenon. Require valid local guest/provenance evidence
   and the relevant preceding intentional timeout before admitting a reproduced
   sample. Keep the useful distinction that an unrelated later failure need not
   erase a valid earlier reproduction. The actual after-09 receipt is a genuine
   guest timeout; this finding does not erase that observation or demand a
   globally complete run as its prerequisite.

5. **P2 — Separate observed output from unmeasured completion and termination.**
   `docs/experiments/exec-reliability-macos-timeout-inspection.md:32` concludes
   that the old timed-out observations have complete stdout because a different
   successful observation printed no row. The timed-out samples cannot establish
   whether their children were absent or whether more output would have followed.
   At `:117`, the listing's own final PID row similarly establishes substantial
   output, not that enumeration/output had finished; shell/ps exit was not timed.
   The timing-only claims at `:35` and `:118` should remain consistent-with
   interpretations while the binary provenance gap and phase timestamps remain
   unresolved. At `:108`, no process observed for PID 630 is attributed to an
   agent kill without a signal/exit record; at `:153`, empty cleanup command
   output is described as finding the PIDs gone despite the ambiguity in finding
   3. These stronger premises already reached the goal context of
   `macos-timeout-inspection-k193`. Qualify them across the report and that
   handoff: preserve the timeout, printed rows, later observations and elapsed
   times as facts; leave completion, kill attribution and exact wait phases
   unproved until the planned instrumentation measures them.

## Supported results and limits

Run 02's after-09-table guest receipt reports exit -1/timed_out true at 30.148 s
with 460 output rows; the earlier same-command receipt took 0.150 s with 459
rows. The three intentional guest timeouts took 2.242, 2.281 and 2.209 s;
their total host overheads are below 4 s, but do not measure the separate grace
and kill/drain/reap phases. Cancellation stopped the run at host exit -15;
the six later r3 calls remain unvisited. The owned cleanup recorded TERM of
PID 812. Disposal took 1.215 s, its separate inspection reported the owned clone
absent, and before/after VM inventories contain only the retained goldens.
Those records support clone disposal independently of guest cleanup.

Revision 2's expansion from known-child observations to ordinary guest commands
is disclosed, and both original and revised verdicts are retained. It supports
the narrower statement that a process-listing command timed out after the
intentional timeouts. It is a post-hoc expansion, not a predeclared reproduction
of the exact historical known-child observation or proof of a shared cause.
That distinction should survive into k193.

This was source and retained-evidence inspection only; no test, build, lint,
format command, live VM diagnostic or checker execution was performed. Graph
coverage generation 2026-11-15T02:19:52Z does not track the new k4 files and flags
older controller/Taskfile metadata changes; material claims use directly read
source and receipts, not graph completeness. No production or test files changed.

## Decisions (running log)

- The five findings warrant an adjacent integrate-review-impl leaf before k193.
  That session owns triage, any accepted repairs and verification; this review
  leaves the diagnostic and retained evidence unchanged.
