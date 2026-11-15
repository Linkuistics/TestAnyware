# complete-worker-scheduler-k143

## k140 caller scheduling boundary

k140 is narrowed to caller-controlled helper query, retirement and worker close scheduling. It does not establish concurrent production ownership or teardown overlap. This leaf retains every aggregate requirement and historical failure below. Keep k28/k17 open; acceptance stays 0/10 unavailable. Read the k140 report and its independent review before relying on that seam. The k140 delivery is experiments/exec-reliability/evidence/caller-scheduling-k140/report.md: optional synchronous drive callback across capture query/retirement/close and Worker.close; final 931 full cases plus six audit-tool cases/static checks pass, four routing mutants reject. All failures are retained. Parent full additionally failed test_late_created_identity_rejects_invalid_fields[future] (923 passes); do not claim this repaired because candidate passed. Preserve this failure alongside every original historical failure. A separate review-impl of k140 must precede relying on this seam. Production still has no controller overlap; all original aggregate done conditions remain here. The complete original charter and history follows verbatim.

# complete-worker-scheduler-k140

## k142 review integration handoff

Read `experiments/exec-reliability/evidence/expired-request-review-k142/report.md` before relying on the expired-request controls. Python 3.12 now rejects false worker remedies with real non-consuming zombie preparation; separate receipt-remedy and real ECHILD/ownership-loss mutants reject as well. Four explicitly expected start/completion journal pairs prevent missing-journal false greens. Runtime sources are unchanged. The natural-collected branch remains unobserved: every nonmutant natural wait was `[0, 0]`; deterministic exited coverage does not establish that branch or production timing suitability. Serial full suites retain 920 common passing cases (parent 922 passed/1 skipped; candidate 924 passed); the candidate full receipt's static-import failure remains false, with a separate passing static correction. All ten primary 3.12 mutants and six supplemental 3.15 mutants reject with resolved fixture audits. Prior 276,908 evidence paths are unchanged. Every historical failure, overlap and audit uncertainty below remains binding here, together with concurrent production ownership, actual teardown overlap and final k28 review. k28/k17 stay open; guest acceptance remains 0/10 unavailable.

## k137 control repair handoff

Read `experiments/exec-reliability/evidence/expired-request-controls-k137/report.md` and the k137 independent review before relying on the repaired ownership oracle. It distinguishes real collected/retained workers and closes repeated authority; runtime sources are unchanged. All original charter text remains verbatim below. New Python3.15 parent-full failures: blocked-cleanup disposal, expected interruption continuation, group-snapshot helper capture, competing-turn fixture ps timeout. Candidate-full failures: group-kill-before-inspection hang/ignore-term, zombie/EPERM classification. Retain all original receipts; later passing serialPython3.12 pair does not explain or repair them. Initial reader/static faults, process overlap and four audit timeouts remain visible; separate32fixture absence checks succeeded. No aggregate acceptance is established.

The k137 slice repairs the expired-request ownership control only. Read its report and scheduled review before relying on that control. Concurrent production scheduling, actual teardown overlap, every historical failure and every aggregate obligation remain here. The entire original k137 charter follows verbatim. k28/k17 stay open; acceptance remains 0/10 unavailable.

# complete-worker-scheduler-k137

k136 integrates the k135 readiness review. Advisory sampling/select errors now
fall back to the existing bounded sleep, preserving retirement KILL/reap/receipt;
owner callback errors still propagate. Post-write and post-EOF controls reject
all three closure-guard mutants without changing the existing registrations.
Read `experiments/exec-reliability/evidence/readiness-review-k136/report.md` for
the retained RED, first assertion-rounding failure, four rejecting mutants,
two 903-test/static passes and frozen reviewed-driver comparison. All prior
evidence is preserved. This aggregate still owns concurrent production owners,
actual teardown overlap, every remaining control and historical failure, and
final independent k28 review. k28/k17 stay open; guest acceptance remains
0/10 unavailable. The original charter and handoffs below remain binding.

k131 integrates the k130 request-turn review. Wake assertions now cover idle,
ready, cutoff and terminal query callbacks, and retirement after reap or ECHILD
with cleanup still open past the original KILL time. Read
`experiments/exec-reliability/evidence/request-wakes-k131/report.md` for the
frozen rejecting wake mutants and their limits. Production transport is unchanged.

Two reviewed boundaries remain this aggregate's work. The zero-timeout query
poll followed by the driver's up-to-5-ms idle sleep can delay observation of data
that arrives just after the poll by up to 5 ms; the former blocking select could
wake immediately on readiness. Decide how concurrent owners wake on readiness
while preserving the original cutoffs and historical timing failures. No control
here treats the known sleep interval as proof of aggregate latency suitability.
Persistent dispatch also calls `_write(worker.command, command, query_deadline)`
before `run_turns` in `worker_capture.py`. Although a command is capped at 4096
bytes, backpressure can keep that write in 5-ms select waits until the query
deadline. Include it in the remaining private-wait inventory and shared scheduler
work; k131 does not turn it. All prior aggregate requirements and handoffs below
remain binding, including concurrent ownership, teardown overlap and final k28
review. k28 and k17 remain open.

k126 handoff: parent-side query and failure-retirement transport now use bounded
shared-driver callbacks with ordinary/safety registrations. Read
`experiments/exec-reliability/evidence/request-turns-k126/report.md` and its
independent review before changing those boundaries. Ninety focused controls,
five rejecting mutants and two interleaved full pairs cover this seam. The
production owners still run sequentially; all concurrent scheduling, teardown
overlap and other aggregate requirements below remain binding. Original charter
and historical handoffs remain unchanged below.

k128 review integration adds `turns.py` to implicit measurement subjects and
proves the initial-due clock bypass with a rejecting mutant. See
`experiments/exec-reliability/evidence/driver-review-k128/report.md`. Safety flags
on the four singleton compatibility consumers remain unobservable; when owners
share the production scheduler, verify their registrations through competing
ready owners. k128 establishes neither shared scheduling nor teardown overlap.

k124 handoff: common turn driver centralizes sleeping in four compatibility consumers. Production owners remain sequential; persistent transport cleanup, actual concurrent teardown and every aggregate acceptance item below remain open. Read evidence/common-driver-k124/report.md and the k124 review before connecting owners. Entire original charter/history follows unchanged.

k107 handoff: the first worker WNOHANG attempt now occurs even at an expired
cutoff, and owner-level attempt history forbids subsequent expired polls. Read
`experiments/exec-reliability/evidence/worker-first-reap-k107/report.md` and its
scheduled review before changing that boundary. The 754-case suite/static
checks passed for that bounded slice; shared scheduling, teardown overlap and
all remaining aggregate requirements below are still open. The original
charter and historical log below are preserved in full.

k109 reconciles the k108 shutdown-phase review in
`experiments/exec-reliability/evidence/shutdown-phases-k109/report.md`. Binding
finite-reserve and aged-TERM controls each run three times and reject the three
surviving regressions; exact-clock controls reject 3 ms grace/KILL delays.
Phase timing now distinguishes on-time, equality and late completion, and a
strictly late grace phase makes failure sticky. Ordinary real escalations can
still be late under OS scheduling; passing the checker verifies their failure
receipts rather than establishing hard real-time success. The final frozen
712-case suite and static checks pass. This leaf retains its entire aggregate
charter, shared scheduling, overlap, expired-cutoff first reap and final review.

Before changing bootstrap or claiming the aggregate, read the k101 bootstrap report at `experiments/exec-reliability/evidence/bootstrap-bounds-k101/report.md` and its scheduled independent review. k101 supplies Plan/direct-Worker readiness and failed-bootstrap phase declarations only. This continuation still owns worker-failure and orderly-close declarations, the shared scheduler, overlap, all remaining controls and the final k28 review. The original charter below is retained in full.

Before changing the aggregate runtime, read the narrowed k96 producer, its scheduled independent review and `experiments/exec-reliability/evidence/request-identity-k96/report.md`. k96 supplies the persistent request envelope and controller scope only. The full original aggregate charter below remains binding, including shared scheduling, epoch closure, shutdown overlap, remaining controls and the final review of bounded-inspection-k28. Do not treat the identity slice's passing tests as aggregate acceptance.

## Goal

Deliver every remaining requirement of bounded-inspection-k28 (including all promoted k30/k34/k38/k40/k42/k44/k48 contracts), its ancestor chain and docs/specs/diagnostic-process-inspection.md after isolated-helper-capture-k51. This is production runtime work plus adversarial real controls, not an integration placeholder.

## Context

The delivered late-identity slice is documented in experiments/exec-reliability/evidence/late-helper-identity-k71/report.md. Read its scheduled review and any ensuing reconciliation before relying on cleanup-only parsing. The full suite and identity audit establish that slice only; the complete aggregate contract below remains binding.

Read k28's full charter and all ancestor briefs. The former k48/continuation charters and decisions remain byte-for-byte in [Grove brief history](../../../docs/planning/grove-history/README.md); use that index when their historical evidence is relevant. k51 introduces an interim worker per query to isolate synchronous helper creation. It is not a persistent worker, gated helper publication protocol, or shared immutable scheduler. At k51 the worker-local capture engine still used synchronous Popen. k62 replaces that helper boundary with raw fork/reset/exec and local exact-child ownership; the per-query worker protocol still conservatively retains an unknown helper-creation obligation when interrupted. Do not treat worker reap/group KILL as proof of helper death.

## Done when

- Replace per-query startup with one helper-owning direct child before resource acquisition, bounded readiness and all later preparation exits accounted. Enforce native/Python thread/SIGCHLD admission and child reset before EVERY worker/helper fork, fresh owners and descriptor allowlists, os._exit on every child path.
- Publish intent then gated PID/PPID/PGID/request identity completely before exec without controller ACK. Explicit fork error preserves errno/no child; silence is unknown creation, late publication cleanup-only. Track each known helper until reap acknowledgement independently of worker death/group KILL.
- Implement separate capped nonblocking control/result channels, <=1MiB frames, <=256KiB combined output, <=4096 rows and <=64KiB combined reads per scheduler turn. Strictly validate run/worker/request/workload/epoch and ordering dispatch <= start <= end <= receipt <= immutable deadline, clean exit/stdout/stderr, duplicates/conflicts/partial/overflow/backpressure, leader and PGID identities. Preserve strict table parsing and distinct observation/probe semantics.
- One safety-first earliest-deadline scheduler owns active workload/teardown/helper/worker shutdown, no private wait/join/drain/destructor loops. Preserve mandatory group KILL at T including exited leader, denied-signal epochs, PID-only suppression/no widening, normal no group cleanup signals, sole final query only after last signal and exit before K-min(100ms,A/4), no target signal/probe after cutoff/reap, exact-child WNOHANG only until K. All workload leaders, known helpers, unknown creation, workers and observed members retain distinct strong owners or explicit residuals.
- Worker failure stays sticky with no replacement/retry/queued request, later workloads unreached and public safety cleanup/dispose/absence still attempted. Worker failure TERM grace <=2s then KILL/reap <=2s overlaps teardown under original deadlines. Healthy close has one grace; already-retiring worker gets none renewed. Bootstrap <=1s with bounded startup failure shutdown.
- Deliver ALL real/adversarial controls from the spec and k48: beforefork/betweenfork-publication/preexec delays, real slow helper KILL plus wait-return hold, separately labeled simulated unreapability, inherited hooks/handlers/wakeup/evidence FD and worker TERM with live workload/no worker-origin workload signals, native/Python/SIGCHLD rejection at every fork, real fork error versus unknown creation, malformed/late/duplicate/epoch/partial/stderr/output/row/frame/backpressure/postcutoff messages, early reap/stale callbacks, near-T exit within 30ms/no early query, aftercutoff exit/live table/no retries, mutation cap/overrun controls, teardown overlapping worker TERM/KILL/reap, startup/later-preparation/healthy-close/already-retiring. Independent exact-identity audit outside faulted worker. Real privileged ps KILL/UID EPERM remain explicitly unavailable unless separately authorized fixture prerequisites exist; acquire no privilege and do not call injected errno proof.
- Extend every frozen subject/bound, runner/typecheck, immutable retained samples and report. Preserve all prior evidence byte-for-byte. No guests, installed app changes or unrelated processes; ten-clone acceptance remains 0/10 unavailable; storage stall is separate k18.
- As final producer work insert review-impl bounded-inspection before retained-evidence-k21, **Reviews:** bounded-inspection-k28, with complete runtime waits/destructors/signal/ownership/concurrency/inventory trace. No empty integration leaf; review reconciliation is earned by findings.

## Notes

Keep k28/k17 open until their actual Done when is met. The former k30/k34/k38/k40/k42/k44/k48 nodes are historical records, not certified complete; all of their unfinished contracts remain binding in k28 and this task. The future full-runtime review now targets k28 as the surviving aggregate producer. Further decomposition must ship independently useful green behavior and carry this entire remainder.

## Decisions (running log)

- 2026-11-14: Flat sibling decomposition keeps the full original k52 contract above here. k52 delivers only worker receipt timing validation in the actual per-query transport. Its review must reconcile before this producer relies on that boundary. Persistent bootstrap, gated identity publication, shared scheduler and every outstanding parent control remain this leaf's work; timing validation alone discharges none of those aggregate requirements.

- 2026-11-14: k55 was narrowed to reject nonpositive helper PIDs in the actual worker transport, with retained RED/GREEN controls. This leaf carries the entire original complete runtime contract above, including all outstanding controls and final aggregate review; PID validation is not helper identity authentication. Read the k55 review and any ensuing reconciliation before relying on that boundary.

- 2026-11-14: k58 delivers only terminal returncode/reap/ownership consistency in the interim transport. This flat sibling retains the entire original runtime contract, including remaining error/admission validation, all controls and aggregate review. Receipt consistency does not authenticate ownership. Read the k58 review before relying on this boundary.

- 2026-11-14: k60 delivers admission schema, error/errno and ownership-state consistency in the interim transport, including the k61 ECHILD finding. This flat sibling retains every aggregate requirement above; receipt consistency does not independently authenticate any helper. Read k60 and its review before relying on this boundary.

- 2026-11-14: k62 now delivers raw helper fork/reset and explicit local owners, with its own real controls. This sibling retains the entire aggregate contract above. Read k62 and its review before relying on that boundary. Raw fork does not supply gated publication, independent helper identity, persistent worker bootstrap or the shared scheduler; all remain here.

- 2026-11-15: k64 ships only the native SIGCHLD SA_NOCLDWAIT admission repair and its retained real controls. This flat sibling preserves the complete original aggregate charter above. Read the k64 artifact and ensuing review before relying on that boundary. Persistent pre-resource bootstrap, gated identity publication, immutable shared scheduling and every outstanding fault/control and aggregate review obligation remain here; all aggregate ancestors remain open.

- 2026-11-15: Flat sibling split: k67 delivers only gated helper identity publication and controller retention through a matching reap receipt. This leaf retains every requirement above, including explicit fork-error/no-child messaging, persistent bootstrap, full identity epochs, shared scheduler, all outstanding controls and final aggregate review. Read k67 and its scheduled review before relying on that boundary; all aggregate ancestors stay open.

- 2026-11-15: k69 delivers only explicit helper fork-error publication and conservative no-child accounting. Read its review before relying on that distinction. All aggregate runtime requirements and controls above remain here; no aggregate ancestor is complete.

- 2026-11-15: k71 is narrowed to cleanup-only late helper identity retention. This flat sibling owns the complete aggregate contract above, all remaining controls, and the final k28 review. Read k71 and its scheduled review before relying on that boundary. No persistent worker or shared scheduler is established by late identity retention.

- 2026-11-15: k75 delivers only incremental result framing in the interim transport. This sibling retains the ENTIRE aggregate charter and every remaining control above. Read k75 and its scheduled review before relying on the transport boundary; no persistent worker/shared scheduler is established.

- 2026-11-15: Flat sibling split: k78 delivers only admission/deadline revalidation after preparation immediately before worker/helper fork. This leaf retains the ENTIRE aggregate charter above, every outstanding control and final aggregate review. Read k78 and its scheduled review before relying on this boundary. All aggregate ancestors remain open.

- 2026-11-15: Flat sibling split: k81 adds only separately recorded signal-0 probe intervals, retaining the validated table-query timestamps separately. Read k81 and its scheduled review before relying on that evidence boundary. This sibling retains the ENTIRE original aggregate charter above, all controls and final k28 review; k28/k17 remain open.

- 2026-11-15: k83 ships only fixed worker shutdown-deadline guards at signal/reap entry points. This flat sibling retains the ENTIRE aggregate charter above, every outstanding control and final k28 review. Read k83 and its scheduled review before relying on that boundary. k28/k17 remain open.

- 2026-11-15: Flat sibling split: k85 now delivers only workload termination signal/exit-observation deadline admission, with frozen RED/GREEN controls. This leaf owns the ENTIRE aggregate charter above, including active-workload waits, probe/reap boundaries, persistent worker, shared scheduler, every outstanding control and final k28 review. Read k85 and its scheduled review before relying on this narrow boundary. k28/k17 remain open.

- k85 bounded slice: workload `_terminate` now fixes outer K before first TERM, samples signal/exit-observation admission at equality, and rejects late observer returns. Retained evidence/report: experiments/exec-reliability/evidence/termination-cutoff-k85/report.md. This does not complete shared scheduling, persistent ownership, `_wait`, group snapshot/probe/reap admission or any aggregate controls. full-1 retains one unknown permission table and four empty-PID real fixture failures; full-2 passes unchanged runtime/tests after a frozen-checker metadata path fix. Their intermittent mechanism remains unresolved; preserve every failed attempt and do not claim repair from the passing replay.

- k90 integrates k89: signal admission now closes at K-R, and receipts use the admission clock sample. See experiments/exec-reliability/evidence/termination-cutoff-k90/report.md. The spec requirement to attempt the first exact-child nonblocking reap even after K is already expired remains this aggregate leaf’s work; k90 does not change `_reap_leader`. All charter items and final review above remain binding. Earlier intermittent fixture failures are still unresolved.

- 2026-11-15: k88 is narrowed to the first exact-child nonblocking reap at an expired deadline. This flat sibling retains the ENTIRE original aggregate charter above, including every unfinished control and final k28 review. Read its scheduled review/reconciliation before relying on that slice. k28 and k17 remain open.

- 2026-11-15: k91 now owns only the active workload exit-wait deadline admission and completion repair. Read its scheduled review before relying on it. This sibling retains the entire aggregate charter, including replacing private waits with the shared scheduler; a corrected polling loop does not deliver that scheduler. All aggregate ancestors remain open.

- 2026-11-15: k94 is narrowed to persistent pre-resource worker startup, sequential request reuse and sticky lifecycle shutdown. This continuation retains the ENTIRE aggregate charter above, including the shared scheduler, full identity epochs, concurrent teardown/shutdown, every outstanding control and final k28 review. Read k94 and its scheduled review before relying on that boundary; k28/k17 remain open.

- 2026-11-15: k96 is narrowed to persistent request identity envelopes across command/control/result channels and production workload/epoch/phase propagation, with frozen RED/GREEN real transport controls. This flat sibling retains the ENTIRE aggregate charter above, including the shared scheduler, concurrent teardown/shutdown, epoch closure at cutoff and every outstanding control/final k28 review. Read k96 and its scheduled review before relying on that boundary. Standalone test-only one-shot transport remains outside that identity slice. k28/k17 remain open.

- 2026-11-15: Flat sibling split: k98 delivers only orderly-close/TERM/KILL worker escalation with no renewed retirement deadline or duplicate earlier signals. This leaf retains the ENTIRE original aggregate charter above, every remaining control and final k28 review. Read k98 and its scheduled review before relying on this boundary. Shared scheduling and concurrent teardown remain here; k28/k17 remain open.

- 2026-11-15: k98 narrows worker close to one 50ms orderly EOF grace, then TERM/KILL/reap with preserved earlier deadlines and attempts. Read complete-worker-scheduler-k98, its scheduled review/reconciliation and experiments/exec-reliability/evidence/worker-close-k98/report.md before relying on that boundary. Its synchronous close loop is not shared scheduling or concurrent teardown; every aggregate requirement above remains this leaf’s responsibility.

- 2026-11-15: Flat sibling split: k101 ships declared, validated bootstrap readiness and bootstrap-failure TERM/KILL-reap bounds, wired from Plan to the worker with absolute phase receipts and real owned-child controls. This leaf retains the ENTIRE aggregate charter above, including declarations for worker failure/orderly close, shared scheduling, every unfinished control and final k28 review. Read k101 and its scheduled review before relying on that slice. k28/k17 remain open.

- 2026-11-15: k106 accepts k105 F5: bootstrap starts with an infinite owner shutdown deadline and no prior request signals, so its controls cannot prove preservation of earlier finite cutoffs or signal history. Independently demonstrate those properties for worker failure/orderly close, including denied attempts, under this leaf's remaining phase declarations and shared scheduling work. Read experiments/exec-reliability/evidence/bootstrap-bounds-k106/report.md for the reconciled bootstrap-only evidence; every aggregate requirement above remains open.

- k106 verification handoff (2026-11-15): the unchanged 626-case suite passed in `evidence/bootstrap-bounds-k106/attempt-2/full/`. Later replays retained existing timing-sensitive failures: `full-final/` live-group False control got an unknown group because ps capture exhausted its deadline (helper reaped; termination still failed closed); `full-final-2/` had four missing `after_term` observations and two prebuffer timeouts in late-created-identity controls. Bootstrap controls passed in both replays. Aggregate acceptance must account for these retained failures and their timing/setup assumptions; k106 does not claim a deterministic whole-suite pass across replays.

- k106 verification handoff, continued: `full-final-3/` also retained `test_review_real_ps_failure_disables_later_queries_and_probes` failing because stderr was empty instead of containing `usage:`. Final unchanged-input `full-final-4/` passed 626/626 and all static checks. This pass does not erase the three failed replays; see the k106 report for exact cases and raw result paths. k104 still owns aggregate repeatability and all its original obligations.

- 2026-11-15: k104 is narrowed to declared worker-failure/orderly-close bounds and phase receipts. This continuation retains the full aggregate charter and history above, shared earliest-deadline scheduling and shutdown/teardown overlap, every outstanding control and final k28 review. Read the k104 report and scheduled review before relying on that slice; keep k28/k17 open.

- 2026-11-15: k107 is narrowed to the first exact-child worker reap even at an expired cutoff. This sibling retains the ENTIRE aggregate charter above, shared scheduling, concurrent teardown/shutdown, every outstanding control and final k28 review. Read k107 and its scheduled review before relying on that boundary. All aggregate ancestors remain open.

- k112 integrates k111 F1/F2: the worker first-reap exception is explicitly owner-lifetime, including all request polls. A real worker serves a request, exits after its last poll, then reaches expired close: close leaves failed residual ownership without another poll, even though status is waitable. Neither request nor close resets history. This is a documented limitation for this aggregate's shared scheduler work, not proof of final-cutoff reaping for previously polled owners. Preserve the same-cutoff no-later-poll rule in any redesign. See experiments/exec-reliability/evidence/worker-reap-scope-k112/report.md for the control and close-reset mutant. The entire charter, repeatability concerns, remaining controls and final k28 review above remain open.


- 2026-11-15: Flat sibling split: k110 supplies incrementally schedulable worker shutdown through begin_close/advance_close, also driven by the existing synchronous close consumer. Read its scheduled k113 review and evidence before relying on this boundary. This leaf retains the ENTIRE aggregate charter and history above: remove compatibility/private wait loops in the shared scheduler, overlap worker retirement with safety teardown, preserve original cutoffs/owner-lifetime first-reap history, deliver every outstanding control, account for retained intermittent failures, and insert the final review of bounded-inspection-k28. k28/k17 remain open.
- 2026-11-15 k110 retained full replay: ordering ignore-term sees real ps
  row `30416 30416 ?E`, records unsupported process state/unknown, then reaps
  leader and cleanly closes worker. Diagnose this process-table repeatability
  limitation with the other retained unknown-inspection cases; no parser
  relaxation or aggregate completion is established by k110.

- k115 integrates all five k113 findings. Read
  experiments/exec-reliability/evidence/incremental-review-k115/report.md before
  relying on shutdown evidence or wake scheduling. Intermediate evidence no
  longer claims terminal shutdown authority; overrun turns retain the sleep
  guard. Direct begin/advance clock controls and drain/wake mutations cover
  immutable deadlines, overdue first-turn TERM/KILL, exact wakes and exhausted
  first-reap authority. A starved caller receives no renewed TERM grace.
  The declared ten isolated parent and ten k110 ordering/ignore-term samples all
  pass; the earlier `?E` sample is still unexplained and remains this leaf's
  repeatability obligation. The full 826-case suite and 238 review controls
  pass for this slice. Every aggregate obligation above and final k28 review
  remain open; ten-clone acceptance stays 0/10, unavailable.

- 2026-11-15: k114 is narrowed to nonblocking workload termination signal/exit turns. This flat sibling retains the ENTIRE aggregate charter and history above, all private wait removal/shared scheduling, concurrent teardown/worker retirement, remaining controls and final k28 review. Read k114 and its scheduled review before relying on the turn boundary. k28/k17 stay open.

- k114 bounded seam: `_Termination.advance()` now exposes workload signal/exit
  turns; `_terminate` still sleeps as a compatibility consumer and performs the
  single synchronous final query. This does not deliver shared scheduling or
  overlap. Read the k114 report and scheduled review before relying on it.
  Its first full run also retained transport control
  `test_review_worker_fork_failure_evidence[lost-result]`: worker 21679 reaped
  with code 70, valid fork-failure report and no conflict, but both channel EOFs
  were not established, so unknown creation remains conservative. Preserve and
  account for this sample along with the earlier repeatability cases. Entire
  aggregate charter above, final k28 review, and 0/10 unavailable remain open.

- 2026-11-15: k118 integrates k117 F1–F7; read
  experiments/exec-reliability/evidence/termination-review-k118/report.md before
  relying on the termination seam. Cutoff exhaustion no longer mislabels a live
  leader as an observer failure. The first observer error is sticky and stops
  further observations without suppressing the bounded safety signal. Per-turn
  observation counts and exact-fixture timeout cleanup now have retained controls.
  The review's literal double-observe mutant already failed eight old controls;
  a later-turn-only variant passed all 20 old controls and fails the strengthened
  pair. This is a test gap, not a demonstrated duplicate call in production.
- k117 F4 remains aggregate work here: the five isolated baseline/candidate
  pairs execute identical worker_capture/inspection bytes and omit suite load.
  They cannot explain the retained green-full lost-result failure (worker exit
  70, errno 35 and incomplete worker frame length), nor prove k114 caused it.
  Inspect worker-exit/fork-failure-channel close ordering and run interleaved
  full-suite parent/candidate repeats, preserving every sample and input hash.
  Scope the repeats to the actual changed scheduler implementation when it
  exists; ten clean isolated samples are not causal evidence. Account for
  existing timing/setup failures by name. k118 does not certify repeatability,
  shared scheduling, teardown overlap or guest acceptance; k28/k17 remain OPEN,
  with guest acceptance 0/10 and unavailable.

- 2026-11-15: k116 is narrowed to the fork-failure channel EOF-order repair.
  This flat sibling retains the ENTIRE original aggregate charter and every
  outstanding control above, including shared scheduling, overlapping teardown,
  all retained timing/setup failures, interleaved full-suite parent/candidate
  repeats against the actual changed scheduler, and final k28 review. Read
  k116's evidence/fork-channel-k116/report.md and scheduled review before
  relying on its boundary. A repaired one-shot channel EOF order is neither
  persistent-worker failure recovery nor shared scheduler acceptance. k28/k17
  remain open and guest acceptance remains 0/10 unavailable.

- 2026-11-15: The k116 full baseline retained an additional shutdown failure:
  `test_declared_shutdown_real_child_phases[blocking-2-deferred-reap]` at
  test_shutdown.py:298 (`assert all(item < end for item in probes)`). Read
  experiments/exec-reliability/evidence/fork-channel-k116/parent-full-1/runs/shutdown-blocking-deferred-reap-1794716286099350000.json:
  sample 2 has one probe 0.004944042069837451 seconds after cutoff
  333565.29822554096. The fixture audit reaped the exact child with status 9.
  The k116 EOF bookkeeping repair changes no shutdown polling; later clean
  samples cannot resolve this timing failure. Preserve it in the aggregate
  investigation and actual shared-scheduler comparisons.

- 2026-11-15: Flat sibling split: k119 delivers only incrementally schedulable workload-leader reaping and its compatibility driver. Read its evidence and scheduled review before relying on that seam. This leaf retains the ENTIRE original aggregate charter and every unresolved obligation above: active workload/inspection waits, shared earliest-deadline scheduling, overlapping worker retirement and resource teardown, owner lifetimes, all controls, historical timing/setup/transport failures, actual scheduler interleaved full-suite comparisons and final k28 review. The parent context already expresses this scope; a deeper node would add no joint context. k28/k17 remain open, guest acceptance 0/10 unavailable.

- 2026-11-15: Flat sibling split: k122 exposes incremental active-workload exit wait turns with a synchronous compatibility consumer. Read its retained workload-turns-k122 report and scheduled review before relying on that seam. This leaf owns the ENTIRE aggregate charter above, real shared scheduling, transport failure recovery, teardown overlap, every unfinished control and final k28 review; k28/k17 remain open.

- 2026-11-15 k122 result: `_WorkloadWait` turns and the production compatibility consumer pass 18 focused controls, six rejected mutations and 866 full-suite tests/static checks. First candidate float-sleep failure retained; corrected candidate passes. All 60 new child journals and two external fixture journals show absence; 193,709 prior evidence paths unchanged. See experiments/exec-reliability/evidence/workload-turns-k122/report.md. Shared scheduler, persistent failure cleanup/close and actual teardown overlap remain k124; no aggregate acceptance or historical-failure repair. Independent seam review is inserted before continuation.

- 2026-11-15: k126 narrows to request-transport wait and failure-cleanup turns consumed by the shared driver. This continuation retains the ENTIRE aggregate charter and history above, including production competing owners, worker retirement overlapping resource teardown, remaining private waits/bootstrap, all controls, historical failures, interleaved full-suite comparisons and final k28 review. Read the k126 report and its scheduled review before relying on its boundary. k28/k17 remain open; guest acceptance remains 0/10 unavailable.

## k129 command-dispatch handoff

Read experiments/exec-reliability/evidence/command-dispatch-k129/report.md. Parent dispatch now lives in the request callback (one nonblocking write, clipped backpressure wake, post-reap cutoff sample). Existing command mutation tests now intercept actual os.write traffic. All original aggregate requirements above remain binding.

The initial interleaved full comparisons retain these additional unresolved failures: parent-full-2 and candidate-full-2 test_worker_capture::test_persistent_worker_expired_request_is_sticky at test_worker_capture.py:2751, residual_remedy None; candidate-full-1 test_shutdown::test_declared_shutdown_real_child_phases[blocking-2-deferred-reap] at test_shutdown.py:298, postcutoff wait probe. Do not erase these when a later isolated run passes. Final comparisons and per-item audit are in the report. k129 does not discharge readiness wake policy, concurrent owners, actual teardown overlap, historical failure explanations, or final k28 review.

Final k129 pair 1 adds an unresolved failure outside persistent command dispatch: candidate-final-1 test_worker_capture::test_review_group_snapshot_uses_real_worker at test_worker_capture.py:1086, helper capture deadline exhausted. Retained worker-group-snapshot-1794730372595455000.json records standalone request_identity/session_token null, /bin/ps helper PID77581, capture cutoff347651.573231041, KILL347651.573951541, reap347651.579634833, exact fixture PID77579 independently killed/reaped. Source _group_snapshot(worker=None) calls standalone capture_helper; its command buffer stays empty. This path evidence does not explain the helper stall or discharge the failure. Preserve it and investigate alongside the earlier ps/transport failures. The final comparison audit must retain its rejection; do not expand the predeclared historical allowance to make it green.

- k132 split: this flat sibling retains the entire aggregate above. k132 supplies pipe-readiness wakeups only; production concurrent ownership, teardown overlap, complete wait inventory, historical-failure reconciliation and final k28 review remain here. Flat placement shares k28 context without adding a new subgoal.

## k132 measured failure carry-forward

The readiness slice retains `experiments/exec-reliability/evidence/readiness-wakes-k132/parent-full-2/pytest.stdout` and exact raw receipts for three parent failures: `test_review_exited_leader_does_not_hide_a_surviving_group_member[False]` returns -9 instead of 0; `[True]` records leader exit 349937.987481333 after KILL 349937.980860666; `test_review_live_group_is_failure_even_with_expected_leader_exit[True]` returns -9 instead of 0. All independently recorded exact fixtures report absence. These are additional unresolved observations, not explanations of earlier failures. A clean later full suite does not repair them.

The first candidate full suite passes 896 tests but fails strict Pyright pending-list inference at `turns.py:62` (five errors). An explicit pending-list type fixes the inference; original frozen bytes and failures remain retained. Consult the final readiness report and condition audit for corrected-source comparisons. k135 independently reviews the readiness seam before this aggregate continuation. The entire original charter and final bounded-inspection-k28 review remain binding.

- k132 parent-final-2 reproduces `test_persistent_worker_expired_request_is_sticky` at test_worker_capture.py:2751: `worker.evidence()["residual_remedy"]` is None. This matches the retained k129 family and remains unexplained; all checker and recorded fixture audits complete. The entire readiness corpus and report retain this new sample alongside the original three parent timing failures.

- k132 final handoff: optional common-driver readiness and production query/retirement interests are implemented; actual concurrent production ownership and teardown overlap remain unimplemented. The report experiments/exec-reliability/evidence/readiness-wakes-k132/report.md and condition-audit.json retain four interleaved comparisons, all six mutation pairs, nine new cases, 106 focused passes twice and two final 896-case candidate passes. Corrected parent/candidate statics pass; original inference and repaired instrument-import failures are retained. Every frozen hash/artifact verifies, all 94 recorded fixtures are absent and 232,416 prior paths are unchanged. Overall/final comparison gates remain rejected by retained failures; no aggregate acceptance. Readiness review k135 comes first and must feed actionable findings into this aggregate. The full Goal/Done charter above remains binding, including actual concurrency/teardown, all controls and final independent k28 review before k21.

- 2026-11-15: flat split chosen: this session converts worker startup readiness from its private select loop into bounded common-driver turns, preserving identity, frame and immutable deadline checks. A sibling retains the entire aggregate charter and history because the shared scheduler and teardown overlap need separate sessions but share k28 context. k28/k17 remain open. Independent startup review will precede the continuation; no in-session adversarial reviewer duplicates that scheduled read.

## Startup slice handoff from k134

k134 narrows only persistent worker startup readiness to bounded common-driver callbacks. The full charter above is retained, including all unresolved historical failures. Synchronous Worker.start still drives its own turn set, so this does not establish production concurrency or teardown overlap. Read evidence/startup-turns-k134/report.md after its review before relying on the new seam. k28/k17 and final full-runtime bounded-inspection-k28 review remain open.

## k139 startup control integration observation

The test-only startup review integration retains another unresolved shutdown-deadline sample: `experiments/exec-reliability/evidence/startup-frame-controls-k139/candidate-full-1/pytest.stdout`, `test_declared_shutdown_real_child_phases[blocking-0-denied-kill]`, `test_shutdown.py:298`. Exact receipt `experiments/exec-reliability/evidence/startup-frame-controls-k139/candidate-full-1/runs/shutdown-blocking-denied-kill-1794738607774520000.json` records a WNOHANG entry 0.0003471660311333835 seconds after shutdown end 355886.63276249997. The fixture independently reaped the exact child. Runtime source is unchanged; 920 other cases and all statics pass. Preserve this observation beside the existing postcutoff wait family; it is not an explanation or repair of earlier samples. The full comparison gate remains rejected even if the second preplanned sample passes. k139 changes frame-control sensitivity only; the entire aggregate charter remains here.
