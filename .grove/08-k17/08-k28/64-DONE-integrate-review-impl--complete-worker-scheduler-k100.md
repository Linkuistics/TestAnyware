# complete-worker-scheduler-k100

**Integrates:** complete-worker-scheduler-k99

## Goal

Triage the findings of the k99 review of the k96 request identity envelope. Fix each finding you accept. Record each finding you reject with the reason.

## Context

Read the findings in `experiments/exec-reliability/evidence/request-identity-k99/report.md` and in the k99 commit. Re-derive the line positions from the tree as it now stands. The artifact is the k96 code and tests in `experiments/exec-reliability/` and its evidence in `evidence/request-identity-k96/`. The k28 brief chain and `docs/specs/diagnostic-process-inspection.md` bind the work.

Keep the slice narrow. Shared scheduling, cutoff closure of epochs, overlapping shutdown and the final review of bounded-inspection-k28 belong to complete-worker-scheduler-k98. The one-shot compatibility transport stays out of scope. Preserve every prior evidence byte.

## Done when

- Every k99 finding is accepted with a fix and a frozen verification record, or rejected with a reason in the Decisions log.
- Each accepted test change has a mutation that fails it, retained as frozen evidence.
- The full suite, lint, format and type checks pass, retained as frozen evidence. The run ends with no stray child process.
- Leave complete-worker-scheduler-k98 and the parent aggregate open.

## Decisions (running log)

- Accepted k99 F1 as a real test gap: `_service` pins the first run and rejects used tokens, but existing cases never challenge those checks with a second command. Add descriptor-level replay, changed-run and fresh-command controls; mutate each check independently in frozen copies.
- Accepted k99 F2 as a real test gap: damaged ready/intent/created/fork-failure frames must retain unknown creation, creation conflict and a residual remedy without accepting helper identity. Assert those states in the existing real-process matrix and freeze a mutation that falsely clears unknown creation. Runtime behavior and the contract need no change.
- Frozen `request-identity-k100/green-controls-1` passes all 85 identity cases. Independent `mutation-no-replay-check-1` and `mutation-no-run-pin-1` each fail exactly their new negative case (84 pass); the fresh second-command control passes both. `mutation-false-known-creation-1` fails exactly the 44 strengthened damaged-control cases (41 pass). All four runs have equal before/after subject hashes and reaped checkers. The second publication hook creates no helper; the first request uses a real reaped helper, independently checked absent.
- Frozen `green-full-1` passes 582 tests plus Ruff check, Ruff format check and Pyright (zero errors/warnings). `verification.json` reconciles every focused test by identity against mutations and full-suite results, validates all retained run artifact hashes, confirms frozen/live source equality and preserves 127,131 prior evidence files byte for byte (cache directories excluded). All 1,406 recorded process identities are absent in the post-run snapshot; the one deliberately malformed stress JSON is disclosed separately. No runtime source, spec, scheduling or shutdown change was needed; k98 and parent aggregates remain open.
