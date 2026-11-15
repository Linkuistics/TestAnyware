# complete-worker-scheduler-k118

**Integrates:** complete-worker-scheduler-k117

## Goal

Triage the findings of complete-worker-scheduler-k117 against the k114
implementation and apply the real ones.

## Context

Read the findings from the k117 commit and its leaf file. Grade each against
the source and the spec. The k114 slice stays narrow: k116 owns the shared
scheduler, teardown overlap and the final k28 review.

## Done when

Each finding has a classification (unclear contract, real issue, visible
trade-off or noise) with evidence. Reproduce F1 and F2 with a red control
before any production change. Real fixes keep every prior sample and receipt
unchanged.

## Notes

No guests, privilege, installed changes or unrelated processes.

## Decisions (running log)

- 2026-11-15: F1 real issue: retained four RED cases show a healthy observer
  incorrectly marked failed at/equal to cutoff. Expired admission now returns
  without an observer error; live leader still cannot prove completion.
- 2026-11-15: F2 real test gap, with a correction to the review: its literal
  mutant fails eight old controls. A later-turn-only duplicate observation
  passes all 20 old controls and fails two strengthened controls. Both RED
  reproductions preceded any production edit. Retain both mutation variants.
- 2026-11-15: F3 real issue: two RED cases reproduce retries after OSError.
  Stop after the first observation error, retain its cause and continue mandatory
  safety signaling within the original deadlines. No new retry policy or budget.
- 2026-11-15: F4 real evidence limitation. k116 explicitly owns close-ordering
  investigation and retained full-suite parent/candidate repeats; the isolated
  same-code pairs establish no causal claim. The original failed sample remains.
- 2026-11-15: F5 real audit gap. New tooling checks the historical 13/3/2 exact
  failure sets, each condition's exact-child count and all five pairs' parent/
  candidate controller hashes. Historical tooling and receipts stay unchanged.
- 2026-11-15: F6 unclear inventory contract, clarified. New preservation excludes cache
  directories and explicitly measures this checkout's prior evidence inventory.
  The old 176,131 claim includes 14,289 ignored cache files; it is not a claim
  about clone contents. The retained old inventory is not rewritten.
- 2026-11-15: F7 real fixture/test issues. Fake-clock controls own yielding;
  remove the real child's scheduler-latency assertion without increasing grace
  or checker budgets. Record exact fixture identities before readiness waits;
  the external checker revalidates identity and cleans up only those fixtures.
  The first two-second injection stopped during collection with no fixture.
  A ten-second injection reached the fixture and retains its exact KILL/absence
  audit. Ordinary checker budgets remain 60/120/30 seconds; no unrelated process
  or installed setting is touched.

## Verification

Read `experiments/exec-reliability/evidence/termination-review-k118/report.md`
for the finding classifications, RED-before-production evidence and exact run
limits. Frozen final controls pass 26/26; the full suite passes 852/852 with
Ruff, formatting and Pyright clean. The later-turn duplicate mutant fails six
final controls, including the original strengthened pair. A separate timeout
injection proves exact-fixture cleanup; the earlier collection-only timeout is
retained and makes no such claim.

The new audit verifies historical failure identities/counts and paired hashes;
179,670 cache-excluded prior evidence paths are unchanged in this checkout.
The final runtime/config matches the frozen inputs; final tool validation is
retained separately after audit expectations were corrected. k116 retains F4
repeatability/close-ordering work, shared scheduler/teardown overlap and final
k28 review. k28/k17 remain OPEN and guest acceptance remains 0/10, unavailable.
