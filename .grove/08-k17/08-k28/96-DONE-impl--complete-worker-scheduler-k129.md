# complete-worker-scheduler-k129

## Goal

Move persistent parent command dispatch into bounded request turns, so command
backpressure cannot hold the parent outside the shared driver until its deadline.

## Context

Read the k126 request-turns and k131 request-wakes reports. This is the next
private-wait seam. The entire original aggregate charter, historical failures,
remaining controls and final k28 review now belong to flat sibling
complete-worker-scheduler-k132, whose body preserves this leaf's original text.
The existing parent already owns the shared scope; another node adds no context.

## Done when

- Persistent commands remain capped at 4096 bytes and retain the same request
  envelope and original query deadline.
- One callback makes at most one nonblocking command write. Partial progress
  yields, backpressure schedules a clipped future wake, and terminal callbacks
  make no further command writes. Competing ready safety work precedes dispatch.
- Write failure and deadline expiry retain sticky worker failure and conservative
  creation ownership, with the existing fixed retirement allowance.
- New controls fail against the parent; frozen candidate controls and interleaved
  full parent/candidate suites retain individual outcomes and input digests.
- The bounded seam has a separate review-impl leaf before the aggregate resumes.

## Decisions (running log)

- 2026-11-15: Split flat at the command-dispatch boundary. k132 retains every
  original aggregate requirement verbatim, including concurrent owners, readiness
  wake policy, teardown overlap, historical failures and final k28 review. k28/k17
  remain open and ten-clone acceptance is still 0/10 unavailable.
- 2026-11-15: Keep command bytes in the existing request callback. One write per
  invocation yields immediately after progress and polls again after at most 5 ms
  on backpressure, clipped to the original query cutoff. This matches the current
  driver's idle policy; concurrent readiness wake design remains k132.

- 2026-11-15: Frozen RED v2 rejects all five command modes at the private pre-driver select. The initial RED is retained with its synthetic-clock backpressure timeout. Parent command dispatch now attempts one <=4096-byte nonblocking write from the existing query callback; short progress yields immediately, backpressure returns a future wake clipped to the unchanged query cutoff. Query admission, sticky failure and fixed retirement remain the existing authorities.

- 2026-11-15: A sixth exact-clock control advances time to the query cutoff during the reap observation. It rejects the first candidate's ensuing command write (red-reap-cutoff). Final dispatch resamples cutoff immediately before its nonblocking write. Parent and candidate final snapshots share all tests/configuration; prepare records the alternate parent worker source explicitly.

- 2026-11-15: Initial full comparisons exposed 11 bypassed command-mutation controls: their _write hook predates dispatch turns. The controls now damage the actual parent os.write payload for both implementations and retain their original worker/helper absence checks. Frozen final comparisons share these tests and the Ruff strict=False correction. All original runs stay retained.
- 2026-11-15: Original parent-full-2 and candidate-full-2 both fail test_persistent_worker_expired_request_is_sticky (residual_remedy None). Candidate-full-1 also reproduces test_declared_shutdown_real_child_phases[blocking-2-deferred-reap] postcutoff probe. These are explicit unresolved aggregate obligations, not accepted by this bounded seam.

- 2026-11-15: candidate-final-1 additionally fails standalone test_review_group_snapshot_uses_real_worker (helper capture deadline exhausted). The failed receipt has no persistent request identity and the source takes worker=None; new command-dispatch branch is not executed. Cause remains unresolved in k132. Retain the predeclared comparison-audit rejection, rather than widening its two historical allowances after seeing results. parent-final-2 again fails expired-request residual_remedy.

- 2026-11-15: Final interleaved outcomes are parent1 881 passed, candidate1 886 passed/one standalone timeout, parent2 880 passed/one expired-request residual failure, candidate2 887 passed. Ruff/format/strict pyright pass in all four. All six new cases pass in both candidates; 97 targeted controls pass. Full gate stays rejected for candidate1, with original and completed-provenance rejection outputs retained. condition-audit.json verifies integrity and records aggregate_accepted=false; prior-after confirms 216731 earlier paths unchanged. See evidence/command-dispatch-k129/report.md. This completes only the narrowed dispatch seam, with review and every unresolved aggregate obligation left live.
