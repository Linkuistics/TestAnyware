# Fork-failure authority review integration (k121)

The k120 review exposed a control gap in k116, not a runtime defect. An empty
control channel whose writer remains alive must retain unknown helper creation.
The new real-pipe control rejects both unsafe authority mutations. Shipping
`worker_capture.py` is byte-for-byte unchanged from the reviewed version.

## Findings and decisions

- **F1 — real issue, repaired in tests.** The old 37-case selection accepts both
  result-EOF-only authority (M1) and EAGAIN-as-control-EOF (M3). Each expanded
  mutant run now fails exactly `test_fork_failure_empty_control_channel[open]`,
  with 38 other cases passing. The failed assertion is the false known-creation
  classification. Unchanged production passes all 39 focused cases.
- **F2 — visible trade-off, guard retained.** Removing the eager cleanup conflict
  guard (M2) still passes all 39 cases. Buffered tails and duplicate-publication
  checks overlap that guard. This finite sample does not prove equivalence for
  every capped read or persistent completion control. No demonstrated defect
  justifies removing conservative rejection.
- **F3 — unclear contract, clarified.** In the lost-terminal path, the explicit
  no-helper failure must be admitted before the query cutoff. Both actual EOFs
  may finish framing during the existing cleanup reserve, before the original
  deadline, without trailing/conflicting data. The spec now states this timing
  distinction. The failed observation stays failed; no late failure admission,
  extended deadline or inferred absence is permitted.

## Real transport and ownership

`test_fork_failure_empty_control_channel` uses the existing capture seam. The
helper fork raises EAGAIN and the real worker exits 70 instead of sending its
terminal result. A separate direct child of the controller inherits only the
control write endpoint. This holder survives worker exit outside its group, so
the kernel returns EAGAIN on the empty control pipe after result EOF. The test
neither synthesizes pipe bytes/EOF nor replaces time or reap results.

A controller-owned holder was chosen instead of an orphaned worker descendant:
it creates the same open endpoint while allowing exact-parent release and reap.
The holder blocks on stdin with a two-second self-expiry. Test cleanup closes
stdin, waits at most one second, then can kill only that owned child and wait
one more second. In all ten measured holder instances, release and exact-child
reap succeeded with exit 0; no fallback kill was needed. The open cases observed
no control EOF while the holder was alive. The cleanup-eof cases release stdin
only after the real query cutoff and observe real EOF before the unchanged
absolute deadline. The full sample observed that EOF 2.487 ms after cutoff.

## Retained finite comparison

[The original plan](comparison-plan.json) predates execution and still expects a
clean full run. [Observed deviations](observed-deviations.json) record failures
separately; the plan was not rewritten to make an unexpected failure pass.

| Condition | Pytest outcome | Meaning |
| --- | --- | --- |
| old-controls | 37 passed | Original focused controls |
| old-result-eof-only | 37 passed | M1 survives old controls |
| old-eagain-is-eof | 37 passed | M3 survives old controls |
| new-result-eof-only | 1 failed, 38 passed | New open-channel control rejects M1 |
| new-eagain-is-eof | 1 failed, 38 passed | New open-channel control rejects M3 |
| new-omit-eager-conflict | 39 passed | M2 still survives; guard retained |
| candidate-controls | 39 passed | Unchanged production, expanded controls |
| candidate-full | 1 failed, 855 passed | Shared deadline failure below; both new cases pass |

Every condition retains raw stdout/stderr, JUnit, runtime receipts, fixture
journals, declared commands/bounds, and before/after SHA-256 subjects in its own
directory. Frozen `inputs-*` preserve runtime, tests, config, lock and Taskfile.
The old controls were measured before adding the new test. Each subject set was
unchanged during its measurement. Focused pytest is bounded at 60 seconds, full
pytest at 120 seconds, and each static check at 30 seconds, with independent
checker TERM (2s), KILL/reap (2s) escalation. Every checker was reaped without
timeout. The full run also retained 84 shutdown fixture audits and the two
external termination-fixture absence audits. Those shutdown fixture audits all
report exact-child reaping; that does not cancel a deadline violation elsewhere.

### Full-run failures remain failures

The only pytest failure was
`test_worker_shutdown_deadline_closes_owner_operations[stale-term-pid]`.
Its [exact-child audit](candidate-full/runs/worker-deadline-audit-stale-term-False-1794718413279236000.json)
records deadline `335692.452171875` and one WNOHANG call at
`335692.452192166`, **20.291 microseconds late**. The independent audit reaped PID
40652 with status 15. The existing runtime checks time before invoking waitpid;
this sample records a later observation across that boundary. It establishes a
contract failure, without proving that these added controls caused it or that
it is equivalent to k116's separate 4.944 ms deferred-reap failure. Both remain
with k119, which owns shared scheduler/deadline work. No repeat full run was used
to replace this sample.

Ruff and format passed in the full sample. Pyright failed with eight diagnostics
because the evidence auditor imported sibling `prepare` from the frozen-input
working directory. The initial tooling is preserved in `tools-initial/`. The
fix uses an independent literal mutation oracle; its first static-only run
(`candidate-static/`) retained two further strict typing diagnostics for the
auditor's late-call list. `tools-import-fix/` preserves that intermediate tool
version. The list now has an explicit type. `candidate-static-final/` checks the
same frozen runtime/tests and final harness from the same working directory,
without rerunning pytest. It retains all three static check results separately.
The corrected tooling does not turn the full sample green.

## Preservation, audit and limits

`prior-before.json` and `prior-after.json` inventory all 189318 prior evidence
paths, excluding this new evidence root and the inherited cache exclusions.
The evidence audit validates those inventories, exact single-site mutants,
retained failure sets, source/config equality, before/after hashes, checker
bounds/reaping, new holder receipts and shared deadline audits. Its
`evidence_reconciled` verdict means the record is consistent; it explicitly
reports `full_suite_passed: false`.

This is one finite full sample and four expanded focused samples (including
two deliberate unsafe mutations). It is not a scheduler/storage/teardown proof,
a repeatability claim, or the ten-clone acceptance run. k119 and the k28/k17
aggregate charters remain open; ten-clone acceptance remains 0/10, unavailable.
No guest, privilege, installed configuration or unrelated process was touched.
Existing tool provision was sufficient; no `mise.toml` was needed.
