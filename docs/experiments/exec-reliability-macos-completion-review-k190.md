# Live macOS completion review (k190)

> Historical report: paths and commands refer to the [archived checkout](README.md).
> The maintained harness is in [tests/exec-reliability](../../tests/exec-reliability/README.md).

The retained k186 run contains ten successful serial receipts and twelve
successful receipts from overlapping host clients. Its reusable verdict path
can nevertheless accept missing or incomplete evidence. Integration is needed
before later work relies on that path as a regression check.

Reviewed frozen-source commit `4b23ff10258b0d5e387482917c86b99547b5e006`
and evidence commit `c414438b40a8df487347cebb81abc2b0514ea0b5`, against the
k186 charter and the root evidence contract. This was inspection only. No
test, build, lint, format or live diagnostic command ran. The failure examples
below follow from source inspection; they were not executed.

## Findings

### F1 — P2: JSON null passes without any guest result fields

`experiments/exec-reliability/completion_k186_check.py:35` validates fields
only for a dictionary. The branch at line 40 rejects other decoded values
only when they are not `None`. Consequently, `judge(0, 0.1, b"null")` returns
`verdict: pass`: parsing succeeds, the field checks are skipped, and no failure
is added. This directly violates the checker's single-object receipt contract.
Require a decoded object and the required field values before accepting a
sample. The retained receipts are objects; this finding does not reclassify
any of them as a failure.

### F2 — P2: the checker accepts absent samples and ignores controller failure

`experiments/exec-reliability/completion_k186_check.py:55` discovers samples
only by iterating the calls and children that happen to be present. It checks
neither the frozen ten-serial/three-by-four shape nor unique names and child
indices. At line 78, zero samples and no reported unreached names produce exit
0. A concurrent round declaring four children but containing three successful
children also passes. Duplicate dictionary keys silently replace samples.

The same loop ignores `result.ok`, storage/inspection/subject failures, each
call's `ok`, stream completeness and the concurrent wrapper's return code.
Thus a run with all successful receipt payloads but a failed controller can
receive a successful standalone receipt check. Validate expected membership
and distinguish a valid successful run from missing exercise or failed host
evidence. Preserve the individual receipt results when rejecting the run.
All expected samples are present in the retained run and its controller is
healthy; the issue is the checker accepting cases outside that evidence.

### F3 — P2: concurrent capture ends at leader reap without proving pipe EOF

`experiments/exec-reliability/concurrent_k186.py:31` uses `_drain` for both
stdout and stderr. It treats `BlockingIOError` and an empty read alike and
returns bytes without recording whether EOF was observed. At line 96, reaping
the CLI stops further polling for that child; the final drain at line 113
still does not require EOF. Success at line 124 depends only on return codes.

A leader can write a valid success object and exit 0 while a descendant holds
the pipe open. The wrapper can return that prefix as successful before the
descendant's later output arrives. Malformed/truncated JSON is rejected by
the checker, but a valid JSON prefix of an unfinished stream is accepted.
This is the truncated-pipe case required by the review charter. Capture must
record EOF separately from temporary emptiness and reject or retain as unknown
an unfinished stream within a bounded capture policy. The retained concurrent
payloads contain the expected objects, but their inner pipe EOF is unobserved;
the controller's EOF for the wrapper's own stdout does not establish it.

### F4 — P2: the live task exits successfully after a failed probe or checker

`experiments/exec-reliability/completion_k186_run.sh:18` records the probe
exit without propagating it. Lines 24–26 likewise record the checker exit,
and missing `RUN` skips the checker. The final `cat` at line 28 normally
exits 0 in all these cases. Moreover,
`experiments/exec-reliability/completion_k186.py:155` prints a false outcome
without setting a failing process exit status. The Taskfile at
`Taskfile.yml:213` simply runs this shell script.

As a result, the documented regression invocation can succeed after a failed
receipt, a false controller verdict or an early probe exception. Keep the
evidence files and resource audit, then return a failing status unless the
required probe, controller verdict and check all succeeded. Retained
`probe.exit` and `check.exit` are both 0, so this is a reusable-gate defect,
not evidence of a hidden failure in that run.

### F5 — P2: the receipt checker and outer runner lack before/after hashes

The subject list at `experiments/exec-reliability/completion_k186.py:93`
includes `CHECKER`, but that constant is the k19 inventory checker
`vm_inventory.py`, not `completion_k186_check.py`. Neither the receipt checker
nor `completion_k186_run.sh` appears in the retained 21 subject entries.
The controller's additional subjects at
`experiments/exec-reliability/controller.py:1693` do not add them.

Both files determine the reported outcome, and the root contract explicitly
requires freezing checkers and tooling with before/after hashes. The frozen
source commit and pre-run dirty diff identify their starting source, but do
not establish that these omitted files stayed unchanged through execution and
checking. The receipt checker runs after the controller's final subject read,
so adding it only to that earlier window would still leave its actual check
outside the window. Cover these tools through their use, and document the
historical gap without inventing missing digests or replacing the old run.
All 21 recorded subject digests do match; no mutation is alleged.

### F6 — P2: failure-rate bounds assume independent, comparable requests

`docs/experiments/exec-reliability-macos-completion.md:383` presents 0.098
as the probability of 22 clean requests when each fails with probability 0.1,
and 0.127 as a one-sided 95% upper bound after 0/22 failures. These are the
binomial calculations `0.9**22` and `1 - 0.05**(1/22)`; the serial-only bound
0.259 is `1 - 0.05**(1/10)`.

The arithmetic is correct under independent trials with a common failure
probability. That model is not established for serial and concurrent requests
on one guest, especially when the candidate cause is shared dispatch-pool
state. Equal marginal failure probabilities alone do not imply the product
probability. Fully correlated requests with marginal failure probability 0.1
would all succeed with probability 0.9. State these bounds as conditional
illustrations, keep the serial/concurrent exposure distinction, and avoid an
unconditional empirical failure-rate bound. The existing conclusion that this
run establishes neither a repair nor an excluded cause remains appropriate.

## Evidence assessment and limits

The evidence root is
`experiments/exec-reliability/evidence/k186-completion-macos-01`; its controller
run is `b1256cae4068418dac994e03f0392f6b`. Outer records are in the sibling
`k186-completion-macos-01-outer` directory.

- Read all ten serial stdout receipts and all twelve child stdout payloads
  in the three concurrent records. Each is the expected JSON object, with
  `ok: true`, exit 0, `timed_out: false`, stdout `ordinary-ok` and empty
  stderr. Serial host times are 0.106–0.310 s. Concurrent times agree with
  the report. All child statuses are 0, reasons are `exit`, and signal lists
  are empty. Non-JSON fails parsing; an actually TERM/KILL-terminated child
  has a nonzero return code and fails. A deadline completion after 50 s also
  fails the elapsed check. These checks do not close F1–F3.
- Each round has four host CLI lifetimes with a common overlap of at least
  0.142, 0.258 and 0.145 s respectively. These timestamps do not establish
  overlap of guest handler execution or blocked guest workers. The report
  correctly leaves separately unobserved phases unknown.
- The current `waitForExit` at
  `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:114` dispatches
  two global blocks per initial wait. Four simultaneous requests therefore
  account for up to eight such blocks on the normal path of this source.
  `agents/macos/Sources/testanyware-agent/AgentServer.swift:448` awaits that
  runner. Eight is not a measured total of busy guest workers: unrelated
  work, recovery waits and the historical guest binary's unknown source
  remain outside that estimate. The system limit 64 is observed; actual
  agent worker occupancy is not. Skipping the conditional instrumented clone
  matches the declared no-failure/limit-far-above-nominal-demand rule, but
  supplies no evidence excluding transient starvation.
- `result.json` has `ok: true`, no unreached calls, successful reaping and
  termination fields, and EOF on both controller streams for every call.
  Its before/after subject maps are equal. The storage close receipt is
  orderly with last applied sequence 90, zero refused and zero failed;
  the result records 89 data frames enqueued and acknowledged without loss.
- The ledger and disposal/inspection receipts support one owned clone,
  disposal and no residual. Public VM inventories agree. Tart inventories
  have the same entries and properties except the source golden's `Accessed`
  timestamp: `2026-11-16T20:49:22Z` to `2026-11-16T23:24:17Z`. That change
  is disclosed in the report. App PID is 1488 before and after.
- The dirty diff contains only the outer `source-commit` record and the
  empty diff file being created. Guest identity records agent SHA-256
  `1282f4cf53fe2ede6c7e94a2687888b5fa4ada3e2a920d227db2715679b28a84`,
  macOS 27.0.1 and arm64. Matching the retained binary hash establishes the
  same binary, not its source. The installed bundle's different agent hash,
  changed host CLI/app identity, and failed guest agent-thread lookup are
  disclosed. Unrelated historical claims were not independently remeasured.
- The cause table remains open and the report explicitly claims no repair
  and no excluded candidate. F6 concerns the statistical model, not a claim
  that a clean run proves a repair.
- Tier 2 graph work used project
  `Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`,
  generation `2026-11-15T02:19:52Z`. Relevant production snippets and traces
  were inspected. Coverage checks mark the new tooling/report and all 88
  k186 evidence files untracked, and controller/Taskfile metadata changed.
  Direct source and raw-record reads supply the findings; graph completeness
  is not claimed. No reindex or workload execution was performed.
