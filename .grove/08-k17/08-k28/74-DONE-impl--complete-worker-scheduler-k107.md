# complete-worker-scheduler-k107

## Goal

Attempt the first exact-child nonblocking worker reap even when the original
shutdown cutoff is already expired. Later polls remain bounded by that same
cutoff; no signal authority or grace is renewed.

## Context

The full original aggregate charter and running history are preserved in flat
sibling complete-worker-scheduler-k110. Read the bounded-inspection spec and
k96/k101/k104/k109 reports and their reviews before changing their boundaries.
The shared scheduler, concurrent teardown/shutdown, remaining controls and final
bounded-inspection-k28 review all remain k110's work. k28/k17 stay open and
ten-clone acceptance remains 0/10, unavailable.

## Done when

- An owner receives one first exact-child WNOHANG attempt even at/after its fixed
  shutdown cutoff. A prior attempt prevents any later poll at/after that cutoff.
- Successful late reap releases only that exact ownership and retains failed
  timing. Live/error outcomes preserve their distinct residual/ownership-lost
  receipts. Repeated close/finalizer/stale callbacks cannot renew authority.
- Frozen RED/GREEN, mutation and real-child controls cover before/equality/after
  cutoff, exited/live children, ECHILD/other errno, late returns and stale reentry;
  independently audit exact identities outside injected clocks/waits. Preserve
  earlier evidence byte-for-byte; run full suite and configured static checks.
- Document the delivered slice and limitations; cut its specific review-impl
  sibling. Do not claim aggregate scheduler or overlap acceptance.

## Notes

No guests, installed app changes, privilege acquisition, or old evidence edits.
Actual privileged ps KILL and UID EPERM remain unavailable; simulated errno does
not establish them. Storage stalls remain separate k18 work.

## Decisions (running log)

- 2026-11-15: Flat sibling split carries the complete original charter and history
  into complete-worker-scheduler-k110. This independently useful slice repairs
  expired worker ownership accounting only; shared scheduling and concurrent
  teardown still need the full continuation and final k28 review.
- 2026-11-15: The owner now records reap attempts immediately before exact-child
  WNOHANG. Only a never-attempted owner bypasses an expired cutoff; no-result,
  errors and delayed returns consume the exception. Existing signal guards,
  close timelines and sticky timing failures remain binding.
- 2026-11-15: Delivered the bounded slice with 108 focused controls and the
  754-case full suite passing, plus Ruff/format/strict Pyright. RED retains 46
  failures; suppress-first/allow-later/omit-attempt mutants fail 28/12/42 new
  cases. The report and source/evidence audits are in
  experiments/exec-reliability/evidence/worker-first-reap-k107/. All 408 audited
  exact-child fixtures were reaped, and all 163,383 prior evidence paths agree.
  Thirty ordinary full-suite grace receipts remain late failures; a passing
  checker does not establish timing success or repair historical flaky samples.
- 2026-11-15: Current GROVE_PROVIDER_EVIDENCE is absent. Per retire guidance, the
  new review carries no Creator line; no legacy receipt or provider is invented.
  Aggregate k110 and ancestors stay open; no parent cascade is due.
