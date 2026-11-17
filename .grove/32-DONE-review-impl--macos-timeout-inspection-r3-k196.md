# macos-timeout-inspection-r3-k196

**Reviews:** macos-timeout-inspection-k193

## Goal

Adversarially review the k193 r3-only run and its claims before the
instrumented leaf k195 builds on them.

## Context

- Read the producer commits for `macos-timeout-inspection-k193`, its decision
  log, this grove's root brief and the k193 section of
  `docs/experiments/exec-reliability-macos-timeout-inspection.md`.
- The tooling is `experiments/exec-reliability/timeout_k193.py`,
  `timeout_k193_check.py`, `timeout_k193_run.sh`, `test_timeout_k193.py`,
  the `check` parameters added to `timeout_k4_check.py` and the Taskfile
  entries `diagnostic:macos-timeout-inspection-r3{,-check,-tools}`.
- Evidence: `experiments/exec-reliability/evidence/k193-r3-macos-01` and
  `k193-r3-macos-01-outer`.

## Done when

- Each claim of the k193 section is checked against the retained evidence.
  The checks cover the plan identity with the original r3 sequence, the
  child absences, the cancel child survival and the clone disposal.
- The new `check` parameters are read for a change to the k4 judgement.
- The r3-only checker and its controls are read for failures that a
  passing control would hide.
- Findings are recorded in this leaf's commit. An integration leaf is cut
  only for findings worth acting on.

## Notes

Inspection only. Do not repair the tooling in this leaf.

## Decisions (running log)

- The report's early absence and cleanup-causation claims require correction.
  The original fixture deliberately masks `ps` failures, so a successful
  observation with no row proves only that no row was listed. The later owned
  cleanup independently establishes both PIDs were gone, but cannot establish
  their earlier exit times or which action ended PID 700. This is a material
  evidence distinction for k195, rather than a wording preference.
- The retained plan and raw receipts establish the intended r3 condition and
  the recorded complete, not-reproduced result. The shared checker's default
  k4 judgement is unchanged. No additional acceptance-blocking finding emerged
  from the bounded tooling and control inspection.
- Place one integration sibling immediately before k195. Its finite handoff
  is triage of this review and a report whose process claims match the retained
  evidence, so the causal inquiry starts from supported premises. The existing
  parent supplies all necessary context; no new node is needed.

## Findings

### R1 — Process absence and cleanup causation exceed the evidence

Material evidence defect in
`docs/experiments/exec-reliability-macos-timeout-inspection.md:252` and
`docs/experiments/exec-reliability-macos-timeout-inspection.md:255`, repeated in
`.grove/31-DONE-impl--macos-timeout-inspection-k193.md:65`.

The report promotes missing process rows to valid observations of absence and
says the fixture's guest cleanup ended PID 700. The unchanged original fixture
does not provide either proof:

- `experiments/exec-reliability/evidence/macos-timeout-inspection-k4/fixture-r3/guest.sh:81`
  runs `ps -p "$pid" -o pid=,ppid=,stat=,comm= || true`. A successful enclosing
  observation does not establish the success of this individual `ps` call.
- `experiments/exec-reliability/evidence/macos-timeout-inspection-k4/fixture-r3/guest.sh:89`
  masks the cleanup lookup failure, and its next line masks the `kill` failure.
  The cleanup receipt has no per-PID signal or exit confirmation.
- In run
  `experiments/exec-reliability/evidence/k193-r3-macos-01/d8bca6a5a87547899d23140a124e2ceb`,
  `r3-observe-timeout.stdout` lists PID 682 without a process row;
  `r3-observe-cancel.stdout` positively lists PID 700 as `sleep`, state `S`,
  parent 699; `r3-observe-after-cleanup.stdout` lists both labels without rows.
  The later `cleanup.stdout` independently reports `gone pid=682`,
  `gone pid=700` and `cleanup confirmed`. That establishes their later absence,
  without identifying their earlier exit times or the action that ended 700.

The report itself explains the original fixture limitation at line 122, and
`experiments/exec-reliability/timeout_k4_check.py:351` deliberately classifies
these r3 outcomes as `not-listed`. The k193 table already uses that correct
description. Preserve the raw records and the positive survival observation;
qualify the early observations and the cleanup inference consistently in the
report and producer log. This matters because k195 is specifically investigating
process exit versus agent exit observation. It does not invalidate the
not-reproduced stall result or the later cleanup and disposal accounting.

## Inspection evidence and limits

- Inspected producer tooling commit `ce913129c137` and evidence/report commit
  `94f28cf2`, the root contract, new probe, runner, checker, controls, shared
  checker diff and original r3 fixture. The 23 retained calls have the original
  order and arguments, including the pinned online URL/hash and cancel exit
  -15. Actual call arguments match the retained plan. Health precedes the guest
  work; no issue payload or `after-*` call precedes the r3 condition.
- Read the key raw receipts and all call outcomes. Their durations and typed
  results support the report table, positive cancel-child survival and absence
  of a reproduced inspection stall. The retained checker output reports
  `complete`, `not-reproduced`, no failed/missing calls and confirmed cleanup.
- Read and hashed the evidence files only: all 83 retained storage digest
  entries match their file lengths and SHA-256 values. The two outer tooling
  lists are byte-identical, and their 15 subjects match the reviewed source.
  Recorded before/after runtime subjects also match. The recorded source diff
  contains only the runner's outer records.
- Checked the disposal ledger and before/after VM and Tart inventories. The
  unique clone is disposed with no residuals and is absent afterward; app PID
  1488 is unchanged. Later owned cleanup confirms both recorded PIDs gone.
- The shared checker change parameterizes shape/observations while preserving
  default k4 judgement. Read the 52 k4 and 13 r3 controls and the producer's
  recorded verification claims. No test, build, lint, format, probe or checker
  command was executed in this review.
- The checker does not independently attest the full invocation/order; those
  were verified directly for this run. Propagating every outer inventory read
  failure would strengthen future automation. The retained inventories are
  valid here, so neither limitation warrants another repair obligation for
  this accepted run.
- Graph lookup returned no relevant symbols. Coverage metadata reported no
  recorded issue but did not track most candidate files, so the material
  conclusions above use direct source and retained evidence inspection.
