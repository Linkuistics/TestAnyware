# Review of the first worker reap at an expired cutoff (complete-worker-scheduler-k111)

This review covers complete-worker-scheduler-k107, commit `11090fc306e7`. Line numbers refer to that commit. No later commit touches the reviewed source.

## Outcome

The review found no defect in the first-reap change. The change preserves every later cutoff, every signal restriction and every sticky timing failure. It found two low-severity findings. Both concern the reach of the proof, not the behaviour that the proof covers.

## Method and limits

The review ran no test, build, lint, format or mutation command. It ran read-only hash, diff, JSON-walk and JUnit-parsing commands. It read the source directly. The code graph predates the commit, so the review makes no graph completeness claim.

## Verified

- `_reap` at `worker_capture.py:124` increments `reap_attempts` at line 132 before it calls `os.waitpid`. A no-result, an errno and a late return all follow that increment. None of them can grant a second post-cutoff poll.
- `_signal` at line 147 keeps its unconditional cutoff check at line 152. The change grants no signal, no renewed phase deadline and no new `shutdown_deadline_monotonic` value.
- The only runtime edit is one dataclass field and one guard clause. Production code outside `_reap` and `_Owner` is unchanged.
- `reap_attempts` reaches receipts through `asdict(owner)`. No consumer in `controller.py`, `inspection.py`, `self_check.py`, `peer.py` or `preflight.py` reads the worker owner keys. The field is additive.
- The frozen `green-inputs/worker_capture.py` equals the shipped bytes. The frozen `red-inputs/worker_capture.py` equals the parent runtime. Every other frozen source, the lock file and the Taskfile are identical across all six conditions.
- Each mutant differs from the green input by one line, at `worker_capture.py:130` or `:132`.
- JUnit counts match the report. RED has 108 tests with 46 failures. GREEN has 108 with none. The full run has 754 with no failure, error or skip. The mutants fail 28, 12 and 42 of 42.
- `condition-audit.json` holds 94 exact-child audits for each of RED, GREEN and full, and 42 for each mutant. The total is 408, as the report states.
- `audit.py` differs from `audit-initial.py.txt` by four type annotations only.
- The 42 declared-phase receipts hold 30 late, 6 at-deadline and 6 on-time grace outcomes. A late grace outcome stays a failure.
- The diff outside the retained evidence touches the Taskfile, one specification report, the two checkers, `worker_capture.py` and Grove task files. The k110 diff adds a handoff and keeps the original charter, including the final review of bounded-inspection-k28.

## Findings

### F1: the attempt counter is per owner, so a polled owner gets no first reap at a later cutoff

`reap_attempts` counts every poll over the life of the owner. The guard at line 130 refuses a poll at or after the cutoff once the count is above zero. `capture_helper` polls at line 1084 on every loop turn before the cutoff. A persistent worker that served a request therefore has a high count at close.

A request that times out leaves `shutdown_deadline_monotonic` at the request deadline (line 1021). The reset to infinity at line 1331 runs only for a clean request. Suppose the child exits between its last pre-cutoff poll and that deadline. The final `_reap` at line 1257 and the close `_reap` at line 520 then both return without a poll. The exited child stays unreaped. That is the hazard that k107 set out to remove.

The report and the task text state this scope ("Only a never-attempted owner bypasses an expired cutoff"). The behaviour matches the written requirement. The reach is narrower than the title of the slice suggests.

No control covers the case. The `expired` mode at `test_worker_capture.py:2324` uses a fresh worker that serves no request, so it asserts `reap_attempts == 1`. The 42-case matrix at `test_shutdown.py:355` builds `_Owner` directly. Only the `live` outcome adds a second pre-cutoff poll, and that child stays live.

Failure scenario: a persistent worker serves a request and a late exit follows the last poll. Close at an expired cutoff leaves the child unreaped, with no residual finding raised for the missing poll.

Actionable fix: decide whether the counter belongs to the owner or to the shutdown cutoff. Either record the limit in the specification and the handoff, or reset or scope the counter when `shutdown_deadline_monotonic` is assigned. Add a real-`Worker` control for a polled owner, an exit after the last poll and an expired close.

### F2: no retained mutant targets the counter scope

The three mutants edit only line 130 and line 132. They prove that a first poll is owed, that a later poll is refused and that the attempt is recorded. None of them changes where the count lives. A change that scoped or reset `reap_attempts` per cutoff would fall outside the mutant set. The review cannot say whether the existing controls would catch it, because it did not run one.

Failure scenario: a later edit resets the count at line 501 or line 1021. The 108 focused cases and the mutant set give no retained signal either way.

Actionable fix: when F1 settles the scope, add one retained mutant for the chosen scope. It must fail on a control that uses a polled owner.

## Observations kept out of the findings

- `test_shutdown.py:403` and `test_worker_capture.py:2294` use `select.kqueue`, which exists on macOS and the BSDs only. Neither file carries a platform guard. The project runs these checks on macOS, so the review raises no finding.
- The outcomes `late-exited`, `late-live` and `late-eio` advance a simulated clock inside a wrapped `waitpid`. The `exited` outcomes consume a real child status, and the independent cleanup uses the original `waitpid`. The report labels this split correctly.
- The review first counted grace outcomes over every nested receipt, which gave 133, 50 and 8. The 42 `shutdown-*.json` receipts give 6, 30 and 6, which matches the report. The wider walk includes receipts from other tests.

## Scope

The shared earliest-deadline scheduler, the overlap with controller teardown and the final review of bounded-inspection-k28 stay with complete-worker-scheduler-k110. The 754-case pass does not show scheduler, overlap, privileged or ten-clone acceptance. k28 and k17 stay open.
