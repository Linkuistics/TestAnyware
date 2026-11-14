# complete-worker-scheduler-k52

## Goal

Deliver every remaining requirement of worker-scheduler-k48, its complete ancestor chain and docs/specs/diagnostic-process-inspection.md after isolated-helper-capture-k51. This is production runtime work plus adversarial real controls, not an integration placeholder.

## Context

Read k48's unchanged full charter and all ancestor briefs. k51 introduces an interim worker per query to isolate synchronous helper creation. It is not a persistent worker, gated helper publication protocol, or shared immutable scheduler. The worker-local capture engine still uses synchronous Popen; the parent now bounds its wait and conservatively retains an unknown helper-creation obligation when interrupted. Do not treat worker reap/group KILL as proof of helper death.

## Done when

- Replace per-query startup with one helper-owning direct child before resource acquisition, bounded readiness and all later preparation exits accounted. Enforce native/Python thread/SIGCHLD admission and child reset before EVERY worker/helper fork, fresh owners and descriptor allowlists, os._exit on every child path.
- Publish intent then gated PID/PPID/PGID/request identity completely before exec without controller ACK. Explicit fork error preserves errno/no child; silence is unknown creation, late publication cleanup-only. Track each known helper until reap acknowledgement independently of worker death/group KILL.
- Implement separate capped nonblocking control/result channels, <=1MiB frames, <=256KiB combined output, <=4096 rows and <=64KiB combined reads per scheduler turn. Strictly validate run/worker/request/workload/epoch and ordering dispatch <= start <= end <= receipt <= immutable deadline, clean exit/stdout/stderr, duplicates/conflicts/partial/overflow/backpressure, leader and PGID identities. Preserve strict table parsing and distinct observation/probe semantics.
- One safety-first earliest-deadline scheduler owns active workload/teardown/helper/worker shutdown, no private wait/join/drain/destructor loops. Preserve mandatory group KILL at T including exited leader, denied-signal epochs, PID-only suppression/no widening, normal no group cleanup signals, sole final query only after last signal and exit before K-min(100ms,A/4), no target signal/probe after cutoff/reap, exact-child WNOHANG only until K. All workload leaders, known helpers, unknown creation, workers and observed members retain distinct strong owners or explicit residuals.
- Worker failure stays sticky with no replacement/retry/queued request, later workloads unreached and public safety cleanup/dispose/absence still attempted. Worker failure TERM grace <=2s then KILL/reap <=2s overlaps teardown under original deadlines. Healthy close has one grace; already-retiring worker gets none renewed. Bootstrap <=1s with bounded startup failure shutdown.
- Deliver ALL real/adversarial controls from the spec and k48: beforefork/betweenfork-publication/preexec delays, real slow helper KILL plus wait-return hold, separately labeled simulated unreapability, inherited hooks/handlers/wakeup/evidence FD and worker TERM with live workload/no worker-origin workload signals, native/Python/SIGCHLD rejection at every fork, real fork error versus unknown creation, malformed/late/duplicate/epoch/partial/stderr/output/row/frame/backpressure/postcutoff messages, early reap/stale callbacks, near-T exit within 30ms/no early query, aftercutoff exit/live table/no retries, mutation cap/overrun controls, teardown overlapping worker TERM/KILL/reap, startup/later-preparation/healthy-close/already-retiring. Independent exact-identity audit outside faulted worker. Real privileged ps KILL/UID EPERM remain explicitly unavailable unless separately authorized fixture prerequisites exist; acquire no privilege and do not call injected errno proof.
- Extend every frozen subject/bound, runner/typecheck, immutable retained samples and report. Preserve all prior evidence byte-for-byte. No guests, installed app changes or unrelated processes; ten-clone acceptance remains 0/10 unavailable; storage stall is separate k18.
- As final producer work insert review-impl bounded-inspection before retained-evidence-k21, **Reviews:** bounded-inspection-k30, with complete runtime waits/destructors/signal/ownership/concurrency/inventory trace. No empty integration leaf; review reconciliation is earned by findings.

## Notes

Keep k48/k44/k42/k40/k38/k34/k30/k28/k17 open until their actual Done when is met. Further decomposition must ship independently useful green behavior and carry this entire remainder.
