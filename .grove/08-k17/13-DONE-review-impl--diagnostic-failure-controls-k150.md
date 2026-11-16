# diagnostic-failure-controls-k150

**Reviews:** diagnostic-failure-controls-k149

## Goal

Review the k149 runtime and test changes before k18 consumes the k17 process-control contract.

## Context

Review the commit that retires diagnostic-failure-controls-k149.
Read that leaf's done-when list and its running decision log in full.

Read the `_group_snapshot` grammar change and its comment in `experiments/exec-reliability/controller.py`.
Read the grammar rows in `experiments/exec-reliability/test_controller.py`.
Read the admission checks in `experiments/exec-reliability/test_worker_capture.py` and `experiments/exec-reliability/test_shutdown.py`.
Read the late-created identity fixture in `experiments/exec-reliability/test_shutdown.py`.

Read the k149 section and the k17 reconciliation in `docs/experiments/exec-reliability-diagnostic-foundation.md`.
Use `report.md`, `verdicts.json` and the retained runs in `experiments/exec-reliability/evidence/failure-controls-k149`.
An internal subagent review is recorded in the `review` section of `verdicts.json`. It does not replace this review.

## Done when

- The review states whether the `?E` grammar admits a member that can still run user code.
- The review states whether each admission assertion detects a late, stale or missing admission.
- The review states whether the late-created identity fixture still detects a dropped identity.
- The review states whether each historical class in `verdicts.json` follows from its retained controls.
- The review states whether the two real controls prove what the k17 reconciliation claims.
- Each finding has a severity, a file and line, and a failure scenario.
- When findings need action, an `integrate-review-impl` leaf exists for them.
- No live guest, app or acceptance claim follows from this review.

## Notes

k17 closes when this review retires with no open integration leaf. k18 owns stalled storage.
The UID-denial control needs a human owner with `sudo`. Do not rerun it without one.

## Decisions (running log)

- Reviewed producer commit `4fdb3bb85060a444c0bf7224181cdf298873a55e`
  against its contract, current source and retained evidence. This review ran no
  tests, builds, linters, formatters or diagnostic workloads, and made no
  production or test changes.
- Graph generation `2026-11-15T02:19:52Z` predates the changes. Coverage reports
  changed runtime/test metadata and untracked k149 controls. Direct source reads
  supply the behavior evidence; shifted graph snippets were not used as current
  source. The graph's inbound `_group_snapshot` edge identifies `_final_snapshot`.
- The runtime/test change and both real controls are supported within the limits
  below. Three medium-severity findings prevent accepting the historical audit
  and its whole-k17 reconciliation without integration triage.
- Retirement leaves k17 open through the integration sibling. No parent closes,
  no ADR decision changes and no live guest/app/acceptance result follows.
- Integration `diagnostic-failure-controls-k151` is the next flat sibling and
  consumes these findings from this review's commit.

## Inspection findings

### R1 — P2: The readiness control does not establish the historical cause

Location: `experiments/exec-reliability/evidence/failure-controls-k149/verdicts.json:79`;
also `docs/experiments/exec-reliability-diagnostic-foundation.md:2100`.

Scenario: k18 consumes the assertion that k143 explains the original k145/k132
interrupted failures, although the retained comparison does not discriminate
their cause. `scope-readiness-red/scope-readiness.json` has three failing
never-ready cases, all with empty `child_stdout`, and nine timed-ready cases
that exit 0. By contrast, historical k145 run
`experiments/exec-reliability/evidence/caller-cleanup-k145/candidate-v2-full/runs/179b4702a70e48fdbd1e6dcd9bde8e06/live-exit.json`
exits -9 and its sibling stdout contains PID `34536`. Its elapsed time is about
462 ms, far below the frozen peer's four-second readiness-loop limit. In
`experiments/exec-reliability/evidence/caller-cleanup-k145/candidate-v2/peer.py:50`,
that PID is printed only after leaving the readiness loop. Thus the original
leader progressed beyond that loop before KILL; its readiness time and subsequent
progress are unrecorded. Late readiness may still contribute, but the new
never-ready reproduction is a different observable condition. In particular,
neither that reproduction nor the green peer proves the categorical historical
mechanism in the verdict.

The producer contract explicitly says the missing original readiness timestamp
and source difference alone do not establish causation. Retain the unresolved
historical attribution unless discriminating evidence supports it; do not
promote this possible mechanism into a completed causal audit.

### R2 — P2: Missing child identity is reported as positive absence

Location: `experiments/exec-reliability/evidence/failure-controls-k149/scope_readiness.py:127`.

Scenario: a readiness control loses the child's stdout identity, or its `ps`
query fails, yet the report treats cleanup as audited. The expression
`not child_text.isdigit() or (bool(audit) and audit[-1] == "")` returns true
without any query when identity is missing. All three red never-ready cases
actually record `child_stdout: ""`, `child_audit: []`, `child_absent: true`.
The query path also treats any nonzero `ps` return code with empty stdout as
absence, discarding status/stderr rather than distinguishing inspection failure.
This is not evidence that a child survived; it is an unsupported positive
cleanup claim. The k149 contract requires exact-child/resource cleanup audits,
and `report.md` says all these children are absent. An independently retained
identity and successful absence observation, or an explicit unknown cleanup
result, is necessary before relying on that claim.

### R3 — P2: Unexplained bound misses are promoted to completed causal acceptance

Location: `experiments/exec-reliability/evidence/failure-controls-k149/verdicts.json:245`;
also `docs/experiments/exec-reliability-diagnostic-foundation.md:2132`.

Scenario: a fixture or runtime defect produces an occasional hang or deadline
miss, later attempts pass, and k17 closes as though scheduling had been proven
to be the cause. `checker-ceiling-k77` has no controls and its entire mechanism
is that a later run passed in 54.39 seconds. `prebuffer-k106` also has no
controls and explicitly unidentified cause. The exited-leader control does not
reproduce the historical stall; the helper-latency control's 340 queries never
reach the historical deadline. These observations establish bound misses and
some correct fail-closed outcomes, but do not establish the asserted
`unavoidable_missed_scheduling_bound` cause for each historical observation.
The handoff nevertheless says every k17 condition is met.

This crosses the producer's explicit acceptance rule that later passing samples
alone are not a causal explanation. Preserve the distinction between observed
bound exhaustion, demonstrated mechanism and unknown cause. Resolve or visibly
triage the remaining acceptance gap against the agreed contract before closing
k17; passing totals and unchanged bounds cannot discharge it by themselves.

### Required runtime and test determinations

| Question | Inspection result |
| --- | --- |
| Can the new `?E` grammar admit executable user code? | No such state is established by the change. Apple's [ps state formatter](https://raw.githubusercontent.com/apple-oss-distributions/adv_cmds/main/ps/print.c) emits `E` for `P_WEXIT`; [exit](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/kern/kern_exit.c) calls `sig_lock_to_exit` before setting `P_LEXIT`, and [that function](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/kern/kern_sig.c) holds and waits for the other threads. The exiting thread continues kernel teardown. The grammar requires `E` with `?`, rejects `?S`, `?Z` and `?EZ`, and mixed `?E` plus `S` remains live. Treating all `E` members as non-executable is supported by these sources. This is an exit classification, not proof of reap or process-table absence. |
| Does each admission assertion reject late or missing admission? | Yes, for the recorded `_reap`/`_signal` syscall entries: `test_worker_capture.py:155` and `test_shutdown.py:324` require a nonmissing admission strictly before the cutoff. Signal and reap entries are both covered; strict zip pairs shutdown probes with admissions. The expired shutdown mode intentionally retains the existing owed-first-reap exception. |
| Does each admission assertion reject stale admission? | Both files additionally require execution minus admission in `[0, 0.05)` seconds. A stale value at least 50 ms old fails; an arbitrary smaller stale value is not distinguished from a legitimate scheduling gap. That is the explicit test tolerance, not a stronger clock-freshness guarantee. Retained one-second stale-clock mutants fail 88/94 reap and 70/94 signal cases. The final-reap adapter branches on admission, preventing execution delay from changing the simulated outcome. |
| Does the late-identity fixture still reject a dropped identity? | Yes. The shared witness replaces vulnerable post-TERM file I/O, while `test_worker_capture.py:2123` independently requires the exact expected PID sets in both the receipt and worker obligations. Shutdown requires an actual control read; overflow variants require the identity and overflow in one read. The retained drop mutant fails 14/27 cases; committed inputs pass 27/27. |
| Do the retained passing suites establish those runs? | The committed JUnit files record 94 timing, 27 identity and 975 full-suite tests, with zero failures/errors/skips. These are retained results, not this review's rerun or explanations for older failures. |

### Historical classes, individually assessed

| Verdict ID | Does the claimed class follow? |
| --- | --- |
| `timing-stale-term-k121` | Supported as an admission-versus-entry fixture defect by the frozen guard, controlled 500-microsecond delay and cutoff mutant. The original receipt records entry, not a measured admission timestamp. |
| `timing-denied-kill-k139` | Same bounded conclusion: source and admission control support the fixture error; original admission was not recorded. |
| `timing-deferred-reap-k116` | Same conclusion, including the 4.944 ms entry overage. It is not a demonstrated late admission. |
| `timing-deferred-reap-k129` | Same conclusion for the 215.5-microsecond entry overage. |
| `interrupted-live-group-k145` | A real old-peer defect is demonstrated; attribution of both original observations remains unproven. R1. Cleanup evidence in the red controls also has R2. |
| `exited-leader-grace-k132` | The 158.3/167.8 ms grace misses are established. Their unavoidable scheduling cause is not. R3. |
| `late-created-identity` | Supported by partial original witness files, the forced-stall old failure, repaired stall controls and retained identity-drop failures. |
| `prebuffer-k106` | Fixture progress exceeded its 150 ms wait; unavoidable scheduling causation is not established by an empty control list. R3. |
| `exiting-state-k110` | Supported by the actual `?E` receipt and real red/green leaders. The runtime comment correctly narrows kqueue ESRCH to `proc_find` failure; the report/verdict's older phrase “from exit start” is stale explanatory text. |
| `helper-capture-deadline` | Original timeouts and uncertain/failed outcomes are retained. Current low latency supports neither historical inevitability nor a single cause for all eight observation groups. R3. |
| `empty-pid-k85` | A startup stall reproduces the exact empty-PID error and demonstrates the fixture's 0.3-second dependency. It does not measure the old startup schedule or establish inevitability. R3's attribution limit applies. |
| `lost-result-frame-k114` | Supported: removing the EOF repair reproduces the exact incomplete-frame message; final EOF-order controls pass. |
| `close-before-ready-k104` | The obsolete minimum-delay assertion is a fixture error; the current behavior does not owe the removed 50 ms delay. No new k149 causal control is claimed. |
| `mutation-site-k114` | A control naming an absent source site is a fixture error; it does not demonstrate a runtime failure. |
| `expired-request-none` | Supported by k137's retained exited/stopped/race controls, old unconditional-assertion failure and ownership mutants. The k137 report retains its broader unrelated timing failures. |
| `setup-tooling` | The listed missing files, quoting, fake-clock and witness setup failures are consistent with tooling/fixture errors. k149 aggregates earlier repairs rather than adding per-observation controls; no new runtime guarantee follows from this row. |
| `checker-ceiling-k77` | The 60-second cap miss is established. A later 54.39-second pass alone does not establish its cause. R3. |

### Real controls and reconciliation

Both real controls substantiate their narrow process-control claims. All ten
setuid-`ps` green trials identify the owned unreaped child with real UID 501 and
effective UID 0, record live kqueue observations before the real KILL, and
record exact reap with signal 9 before the deadline and no retained ownership
or children. All eleven checks pass per trial. The three suppress-KILL controls
fail the two KILL checks while retaining the other nine checks.

The foreign-owner receipt identifies PID/PGID `43433`, parent `43429`, and all
three UIDs as nobody. Sender UID 501 gets real EPERM for both signals in each
scope. The group result preserves missing-table `permission_unknown`, the PID
result preserves `signal_error`, and neither claims success. The fixture's
identity remains unchanged. The separate authorised owner records exact
SIGKILL/waitpid, subsequent absence and no children; all sixteen sender checks
are true. No privileged control was rerun here.

Those controls close the former real-control prerequisites. They do not resolve
R1–R3 or justify the handoff's whole-k17 completion statement. Integration must
triage those findings before k17 closes. k18 still owns stalled storage, and
ten-clone acceptance remains **0/10, unavailable**.
