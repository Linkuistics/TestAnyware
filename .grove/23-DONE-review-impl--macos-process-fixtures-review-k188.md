# macos-process-fixtures-review-k188

**Reviews:** macos-process-fixtures-k185

## Goal
Adversarially review the owned process fixtures of the macOS agent tests and
their survival control, before leaf k186 runs `agent:macos:test` on the
shared host.

## Context
- Read the producer commit for `macos-process-fixtures-k185`, its decision
  log, this grove's root brief and the k185 section of
  `docs/experiments/exec-reliability-macos-completion.md`.
- The fixture is
  `agents/macos/Tests/TestAnywareAgentTests/OwnedProcessFixture.swift`. The
  converted tests are in `ProcessRunnerTests.swift` in the same directory.
- The control is
  `experiments/exec-reliability/evidence/macos-process-fixtures-k185/fixtures.py`.
  The Taskfile tasks are `diagnostic:macos-process-fixtures` and
  `diagnostic:macos-process-fixtures-tools`. Raw runs are in the same
  directory.
- The producer did not change `ProcessRunner.swift`. The suite fails in some
  runs with the k3 completed-then-timed-out shape, in both arms.

## Done when
- Challenge whether the fixture can signal a process that the test did not
  start. Cover PID reuse and a stale or partial PID file. Also cover a child
  that exits before tracking and a teardown after a thrown test.
- Challenge whether the control can pass without the property. Confirm that
  the broad pattern selects the control. Confirm that the fault arm runs the
  baseline test file. Confirm that the refusal guard stops the fault arm when
  an unrelated process matches.
- Challenge the claim that the intermittent suite failures do not come from
  the fixtures, using the raw runs of both arms.
- Record each finding with its evidence. Cut an integration leaf only for
  findings worth acting on.

## Notes

## Review outcome

The normal-path receipts support the reported control results. The ownership
and refusal guarantees still have material gaps, and the diagnostic can report
success without exercising the fixture. Integration is needed before k186
relies on these fixtures on the shared host.

Reviewed producer commit `560ef30ad5cf4b4f31c581ec8898d01efc803150`
against its charter, the root brief, ADR-0001, ADR-0004, current source and the
retained evidence. This was an inspection-only review: no test, build, lint or
format command ran, and no production or test code changed. The failure
sequences below are derived from source; they were not induced on the host.

## Findings

### R1 — High: acquiring a start time from a reused PID can adopt an unrelated process

`agents/macos/Tests/TestAnywareAgentTests/OwnedProcessFixture.swift:139–162`
reads only a PID from the file, then learns the start time from whichever
process holds that PID at observation time. The check at line 157 establishes
only that the observed process started no more than one second before fixture
creation. It does not establish that the test created it.

If the original child exits before `track` observes it and its PID is reused
by a newly started unrelated process, an unrelated `sleep` passes every check.
A different command is also appended to `tracked` at line 162 before the
command check throws at line 164, so teardown can signal that replacement too.
The PID can also change owners while the five-second command poll is running.
Rechecking the adopted PID/start pair in `stop` faithfully targets the wrong
process. This is distinct from the documented check-to-`kill` race.

The unique directory protects the file's provenance, but the file does not
bind the original child's start time or retained ownership to the PID. A stale
PID for a process older than the allowance is rejected; a recently reused PID
is not. Repair needs an ownership binding established while the launched child
is still known to be owned, and refusal to adopt a replacement. The report's
unconditional ownership claim also needs to reflect the resulting guarantee.

### R2 — High: the broad fault arm's refusal snapshot does not protect the shared host

`experiments/exec-reliability/evidence/macos-process-fixtures-k185/fixtures.py:247–258`
checks for unrelated matches once, before copying and building the broad
package. That build alone permits 900 seconds. Each iteration takes another
snapshot at line 276 but never refuses when that snapshot contains an unrelated
match. A matching process started during the build or between runs is therefore
passed to the restored, unrestricted `pkill -f 'sleep 30'` cleanup. The baseline
can stop it. Further, `matching_pids` at lines 108–119 treats a nonzero `pgrep`
error with empty stdout as an empty selection rather than a failed safety check.

The initial guard correctly refuses a match visible at that one instant. It
does not support the report and Taskfile claim that the fault arm refuses while
unrelated matches exist. Another pre-run snapshot would shorten the window but
would still leave a scan-to-signal race. The fault mechanism needs an enforced
scope containing only owned victims, or an isolated environment, together with
failure handling that refuses when process inspection fails. Repeating this
fault arm on the shared host is not safe evidence gathering as currently written.

### R3 — Medium: the control verdict can pass without executing the fixture tests

`experiments/exec-reliability/evidence/macos-process-fixtures-k185/fixtures.py:292–318`
parses the suite result but bases its verdict solely on control survival and
pattern selection. It does not require either fixture test to finish, inspect
`hit_bound`, or require the owned cleanup receipts. For example, if the task
exits with an error before XCTest starts, the selected control survives, no new
matching child remains, and the owned diagnostic returns success. A suite
stopped at the outer bound before reaching the fixture has the same problem.
`--runs 0` also succeeds through the empty `all` at line 316.

Keep the unrelated intermittent suite failures visible, but distinguish them
from a valid exercise of the property. A successful control result must require
a positive run count, completed relevant cases and appropriate cleanup evidence;
missing exercise or an outer-bound stop must not count as survival evidence.
This does not invalidate the retained 40 runs: their raw logs contain both
fixture cases and their expected normal-path outcomes.

### R4 — Medium: cancellation during tracking leaves a launched child outside teardown

`agents/macos/Tests/TestAnywareAgentTests/OwnedProcessFixture.swift:133–177`
does not register a child until after the polling loop. If the task is cancelled
while waiting for the PID file or the shell-to-`sleep` transition, the throwing
`Task.sleep` at line 148 exits `track` before the append at line 162. The test
has already launched its child. The registered XCTest teardown runs, but
`cleanUp` iterates only `tracked` and then deletes the PID directory; it neither
stops nor reports that unregistered child. The child can remain for the balance
of its 30-second sleep.

Teardown after successful tracking, including a later thrown test, is correctly
registered. The gap is failure during ownership acquisition. Repair needs to
retain responsibility for a launched child across that failure without adopting
unrelated processes as in R1, and retain evidence when cleanup cannot complete.

### R5 — Medium: controller exceptions bypass cleanup of the owned control

`experiments/exec-reliability/evidence/macos-process-fixtures-k185/fixtures.py:277–311`
starts the control before process inspection, suite execution and log parsing.
Any exception in those steps bypasses `stop_control` at line 294. The enclosing
`finally` removes only the broad package. For example, a process-inspection
timeout or an error starting the suite leaves the script's `sleep 300` running
after the diagnostic aborts, without a cleanup receipt. This is an owned child
and can be cleaned through its retained `Popen` handle.

The control needs bounded cleanup and reaping on every exit after launch,
including failed waits and failed evidence collection. Preserve the original
error and any cleanup failure rather than losing the adverse evidence.

## Evidence assessment

All paths in this section are beneath
`experiments/exec-reliability/evidence/macos-process-fixtures-k185/`.
Read all 40 `run.json` files and their raw `suite.log` files. Every raw log's
SHA-256 matches its recorded value. Every run has all six XCTest case results;
both fixture cases passed in every run, and none hit the controller's outer
bound. Every control was selected by the broad pattern. Each owned run records
two `stoppedByTerm` outcomes and no newly matching process after cleanup.

| Evidence directory | Runs | Control survived | Failed suites | Failed cases |
| --- | ---: | ---: | ---: | --- |
| `owned-01` | 5 | 5 | 3 | stdout once, large output twice |
| `owned-02` | 15 | 15 | 4 | stdout once, large output three times |
| `broad-01` | 5 | 0 | 1 | large output once |
| `broad-02` | 15 | 0 | 3 | stdout once, large output twice |

The control is `sleep 300`, selected because `sleep 30` is a substring.
The broad preparation writes the baseline test bytes into the copied package,
removes the owned fixture, builds its tests, and runs `swift test --skip-build`
there. All plans name baseline
`990641750f0735fcb400e93fcc0974800f26f28b`. Independently reading that revision's
test file gives SHA-256
`07fac70ad07ba5aa45f0ae2ffaade2b8159a768ff809c91b1ffd5db3d5fd137c`,
matching all four plans; it contains both restored broad cleanup sites.
The broad raw logs identify tests in the temporary copied package. No retained
run demonstrates an unrelated-match refusal or an acquisition-failure path.

The PID-file reader ignores an empty file or one without a terminating newline
and retries within its bound. A valid newline-terminated integer is latched;
there is no later reread. A child that is already gone, with no replacement at
its PID, is refused at lines 153–154 of the fixture. Those checks are useful but
do not close R1. The XCTest teardown registration itself precedes launch, so
a throw after successful tracking still cleans the registered child; R4
identifies the narrower gap before registration in the ownership ledger.

The intermittent failures are reproduced in both arms. For example,
`owned-01/run-01/suite.log:19–21` records stdout exit -1 and a timeout assertion
after 5.396 seconds, before the fixture cases start at line 22.
`broad-02/run-13/suite.log:7–10` shows the same assertions after 5.348 seconds,
also before either fixture case. Across the raw runs stdout failed twice in
owned and once in broad; large output failed five times in owned and three in
broad. The large-output case follows the fixture cases, so its ordering alone
does not exclude a fixture contribution.

The 7/20 versus 4/20 comparison and reported Fisher p = 0.48 do not establish
equivalence or exclude every fixture contribution. They do establish that the
new fixture code is unnecessary for this failure shape, and the earlier stdout
failures cannot be caused by fixture code in the same suite invocation. The
report appropriately leaves the timeout cause unestablished and the suite red;
no additional causal repair finding is raised. No dispatch-pool observation or
new runtime experiment was performed in this review.

Graph verification used project
`Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14`,
generation `2026-11-15T02:19:52Z`, Tier 2. The new fixture and controller are
not tracked in that generation; the tests and Taskfile have changed metadata.
Coverage was checked for all six source/report/task paths relied on. Direct
source reads supplied current evidence where the graph was stale or lacked
symbols. Existing runner calls were traced and checked against source; generic
system-call edges resolving to unrelated languages were not treated as evidence.

## Decisions (running log)

Record the source-derived safety and validity findings for integration; preserve
the existing raw runs and their adverse suite results. The integration is a
flat sibling before the next live macOS experiment because it shares the root
brief and directly unblocks safe reliance on the fixture. It consumes this
review by handle and triages the findings rather than inheriting them as a
mandatory repair list. No architecture or production wait decision changed.
