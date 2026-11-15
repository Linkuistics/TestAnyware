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
