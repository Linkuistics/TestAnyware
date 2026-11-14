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

## Decisions (running log)

- 2026-11-15: The review read the k75 producer commit f537c5f by inspection only. It ran no test, build or lint command. The receiver reads, the creation-accounting guard, the unchanged deadlines and the frozen evidence mappings all held up.
- Two findings survive. Finding 1: the RED runs never showed the read-count and early-TERM properties failing, and the hostile-channel test lost an assertion. Finding 2: a zero-length declaration is accepted at the header, and no control covers zero, exact-cap or fragmented-oversized declarations.
- Cut `incremental-result-framing-k80` as the integrate leaf before k78, because k78 is the first live sibling after this review. The integrator classifies each finding before acting.
- No provider evidence was present, so no Creator line is written. No review leaf names k79.
