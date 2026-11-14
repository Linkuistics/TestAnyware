# complete-worker-scheduler-k79

**Reviews:** complete-worker-scheduler-k75

## Goal

Independently challenge the incremental framing correction in the committed k75
artifact before the complete-worker-scheduler-k78 continuation relies on it.

## Context

Read the producer, docs/specs/diagnostic-process-inspection.md, the k71/k76/k77
cleanup-identity lineage, and experiments/exec-reliability/evidence/result-framing-k75/report.md.
The report links all five frozen conditions, source snapshots and independent
audit. The first full run failed and is retained; the final full check passes
357 tests plus lint, format and strict types. Current provider evidence was
absent when k75 finished, so no Creator is invented or borrowed from another run.

The new receiver bounds reads by the four-byte header and declared remainder
plus one detection byte. It rejects malformed result EOF before control EOF,
while terminal success still needs both clean channel ends. Early-error cleanup
must revoke no-child authority for contradictory partial data. Normal/cleanup
control identity retention and all original deadlines remain material.

This is a narrow framing artifact. k78 carries the full original aggregate
charter/history, including persistent pre-resource startup, immutable shared
scheduling and outstanding real controls. This review does not replace the
eventual Reviews: bounded-inspection-k28 review before k21.

## Done when

- Challenge header/body fragmentation, zero/boundary/oversized declarations,
  truncated EOF, trailing/duplicate frames and read/backpressure limits with
  direct source evidence and targeted real pipe/process controls where useful.
- Challenge every early error's creation accounting, especially fork-error
  publications plus partial control/terminal bytes; valid late identities must
  remain cleanup-only, and neither worker reap nor late terminal data may prove
  helper absence or grant helper reap acknowledgement.
- Verify unchanged query/shutdown deadlines, due cleanup ordering and no new
  control wait, terminal success before both EOFs, or per-turn cap widening.
- Audit the actual frozen condition/source mappings, named testcase comparison,
  failed samples and oracle controls. The exploratory red.xml lacks recoverable
  fixture receipts and is explicitly outside the five-condition PID census.
  Later exact-PID absence cannot repair historical uncertainty. Fixtures replace
  worker behavior but use real forks/pipes; do not treat malformed fixtures as
  real helper creation or privileged/setuid evidence.
- Keep any findings anchored to the producer commit and exact source/evidence
  locations; insert any resulting integrate work before k78. Do not discharge
  the aggregate persistent worker/shared scheduler contract. No guests, app
  installation, privileges or unrelated process signalling are authorized.

## Notes

Use the separate review-impl route for the adversarial decision challenge.
k28/k17 stay open and ten-clone acceptance remains 0/10 unavailable.
