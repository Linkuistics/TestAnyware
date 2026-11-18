# linux-agent-cancellation-k236

**Integrates:** linux-agent-cancellation-k235


## Goal

Triage the committed findings of linux-agent-cancellation-k235 against the
Linux implementation and its agreed contract. Reconcile actionable findings
before the macOS cancellation and Host CLI control leaves consume this work.



## Context

- Read the findings from the review's own commit, not from a restatement here.
- Producer: linux-agent-cancellation-k220, commit `b41c7492`.
- Contract: `docs/specs/guest-command-lifecycle.md`, ADR-0024, ADR-0025 and
  the producer's completion criteria and notes.
- Retained producer evidence:
  `experiments/exec-reliability/evidence/linux-agent-cancellation-k220`.

## Done when

- Each finding has a source-verified disposition under the integration
  acceptance and material-risk threshold.
- Actionable findings are reconciled in the implementation or contract, and
  appropriate controls and verification establish the resulting behaviour.
- The Linux agent's existing cancellation, responsiveness, retention,
  shutdown, record and download contracts remain accounted for.
- Documents and retained evidence state the actual result and its limits.

## Notes

This is a flat sibling because it repairs the same Linux artifact under the
same brief. It sits next to its review so intervening implementation cannot
silently move the cited code. Preserve earlier failed evidence. Do not replace
a golden or change the globally installed Host CLI or app.

## Decisions (running log)

- The source of the agent at the start equals producer commit `b41c7492`. The
  review commit changed only `.grove/`, so its line numbers still apply.
- R1 is a real issue. `run_command` checks the cancellation once, before the
  capture directory and the capture files. No check follows before `Popen`.
  Repair: check again in `_own`, directly before `Popen`, after the capture
  files are open. A cancellation that arrives after that last check is a
  visible trade-off. The command then starts, and the owner terminates it with
  `beforeStart: false`. That record is true to the kernel. Holding the signal
  lock across `Popen` would close the gap. The registry calls `request` under
  its own lock, so that lock would then stall every registry call during a
  spawn.
- R2 is a real issue. `_seconds` returns positive infinity for an
  `OverflowError` of either sign. Repair: a negative value past the float range
  gives 0, as every other negative value does.
- R3 is a real issue. The owner sets `ownerLatenessMs` to 0 whenever the pidfd
  woke it, even when the deadline had passed. Section "Arbitration" requires
  the lateness of a late owner that sees an exit after the deadline. Repair:
  compute lateness from the deadline and cancellation events alone. When the
  owner is on time, no event has passed, so the lateness stays 0.
- Each repair gets an old-fail control in `agent:linux:test`. Each control runs
  first against the unchanged agent in the pinned container, and its receipt
  goes to
  `experiments/exec-reliability/evidence/linux-agent-cancellation-k236/`.
- Old-fail run on 2026-11-18, before any agent change: the three new controls
  ran in the pinned container against the agent of commit `23dc246586`. Each
  failed for the reason of its finding. R1 gave `beforeStart: false`. R2 gave
  `exited` in place of `timed_out`. R3 gave `ownerLatenessMs` 0 where the
  control needs 700 at least. The agent and test digests were equal before and
  after the run. The receipt is in `old-fail-01/`.
- The repairs are in `exec_owner.py`. `agent-protocol.md` now states three
  Linux rules. A cancellation before the spawn gives `beforeStart: true`. A
  later cancellation ends the started group. A negative timeout counts as 0.
  The website copy was already behind, as the producer recorded, so it stays.
- The first `task agent:linux:test` after the repairs failed one host control,
  `test_two_accessibility_requests_never_run_inside_at_spi_together`, with
  `assert 5 >= 6` on its call count. Its lock assertion `most == 1` held. The
  cause is in the producer's control: two health requests can share one probe
  check, because the probe clears one request flag for each check. The control
  now accepts one call fewer than its requests. That run's full output was not
  kept. This session saw only the failing assertion above.
- `task agent:linux:test` then passed on 2026-11-18. Ruff passed. The host
  gave 50 passed and 74 skipped. The pinned container gave 124 passed in
  70.64 s. The digests were equal before and after. The receipt is in
  `corrected-pass-01/`.
- Repeats: the cancellation and responsiveness files ran three more times in
  the container, with 34 passed each time. The responsiveness file ran 20 times
  on the host, and each run passed. The receipts are in `repeat-01/`.
- Limit: a record of a timed-out command still carries the request timeout
  unchanged as `timeoutSeconds`. A negative value past the float range is
  therefore a very long negative integer in the JSON. This was already true
  for every negative timeout, and no finding names it. The Host CLI sends a
  positive integer.
- No new review leaf. The repairs are local corrections under the agreed
  contract, not a redesign, so no independent challenge is earned again. No
  node closes, because the root still has live leaves.
