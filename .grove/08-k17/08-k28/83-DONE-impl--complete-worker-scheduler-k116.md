# complete-worker-scheduler-k116

## Goal

Make timely valid helper fork-failure evidence independent of control/result
channel EOF ordering in the one-shot capture transport, without granting late,
partial, contradictory or open-channel evidence no-child authority.

## Context

Read the parent bounded-inspection brief, process-inspection spec, and
experiments/exec-reliability/evidence/termination-review-k118/report.md (F4).
The main capture loop rejects empty result EOF immediately; cleanup drains only
control and fails to record its EOF. A timely fork failure may therefore become
unknown solely because result EOF precedes control EOF. This is a hypothesis
until deterministic RED evidence confirms it; it does not explain every retained
suite failure. The complete inherited charter and running history now live in
complete-worker-scheduler-k119 under the same parent.

## Done when

- A deterministic real worker/pipe test forces each EOF ordering after timely
  valid fork-failure publication and proves the baseline defect before repair.
- Clean closure preserves the actual fork errno, error status, absent helper
  and reaped worker; malformed, late, partial, trailing, contradictory and open
  channels remain conservative. Deadlines/read caps are not renewed or widened.
- Retain immutable RED/GREEN/negative controls and interleaved full-suite
  baseline/candidate samples with genuinely changed worker bytes, before/after
  hashes, checker bounds and exact owned-child outcomes. Report all failed
  samples by name; no passing replay erases a failure or proves all retained
  failures fixed. Root Taskfile provides reusable recipes; static checks pass.
- Evidence report documents the mechanism, scope and unresolved scheduler and
  repeatability obligations. Prior evidence remains byte-for-byte unchanged.
- A separate review-impl leaf owns independent challenge of this artifact.
  Entire aggregate charter stays live in k119; no guests or installed app work.

## Decisions (running log)

- 2026-11-15: Chose a flat sibling split because channel closure is an
  independently testable runtime repair and the shared scheduler needs a fresh
  session. k119 retains the entire original charter/history, every remaining
  acceptance condition and final k28 review. Narrowed goal and criteria together;
  k28/k17 remain open. This slice does not claim persistent-worker or scheduler
  completion.

- 2026-11-15: Frozen `red-order` forces both real EOF orders after timely
  fork-failure publication. Control-first passes; result-first fails with worker
  exit 70, errno 35 in the admitted failure but no receipt errno, and unknown
  creation. The narrow repair records control EOF in the existing bounded
  cleanup read. No parser, timing/cap or late-message rule changes. The full
  comparison plan freezes two interleaved parent/candidate pairs; k119 retains
  the actual shared-scheduler comparisons and all aggregate obligations.

- 2026-11-15: The first full baseline has the expected result-first EOF failure
  and a separate `test_declared_shutdown_real_child_phases[blocking-2-deferred-reap]`
  failure. Its retained exact-child receipt shows one probe 4.944 ms after the
  original cutoff; independent fixture cleanup reaps the child (status 9).
  This remains unresolved in k119; the EOF repair does not touch shutdown polls.
  The second baseline fails only the EOF case. Preserve the declared four-run
  comparison without retries or relaxed ceilings.

- 2026-11-15: Completed the declared comparison: both candidates pass 854/854;
  baselines fail the forced result-first case, plus the separately retained
  first-baseline late shutdown probe. All static checks pass. Final evidence
  audit verifies exact test identities, unchanged measured inputs/artifacts,
  fourteen EOF receipts, 336 exact-child shutdown cleanup receipts and 181,726
  unchanged earlier evidence paths. Final five-tool Ruff/format/strict Pyright
  checks pass. The durable report and diagnostic foundation retain limits and
  k119's full aggregate scope. Independent challenge is sequenced next before
  further scheduler edits; no ancestor is closed and no new design ADR is needed
  for recording an already observed EOF.
