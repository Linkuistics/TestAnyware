# bounded-helper-capture-k50

**Reviews:** bounded-helper-capture-k49

## Goal
Reconcile the findings of review k49 on the helper capture and reap path before worker-scheduler-k48 changes the ownership boundary.

## Context
The findings are in the Decisions log of k49, anchored to commit 2d25827 (producer k47). Re-read the cited code before changing it, because positions may have moved. The cited file is `experiments/exec-reliability/inspection.py` unless a finding says otherwise.

## Done when
- Each k49 finding is fixed, or rejected with a reason in this leaf's decisions.
- Every exit from `capture_helper` after Popen returns, including an exception from stream closure, makes at least one final nonblocking reap attempt and returns a complete receipt.
- After the last wait, the capture observes a signalled helper once more. A late reap is then not a residual.
- Late Popen creation has a control that states what the helper does afterwards: signalled or left running.
- Retained evidence keeps the initially failed lint condition k47-check-01.

## Notes
Scope is this slice only. k48 keeps the worker/scheduler contract and synchronous creation. Do not credit simulated controls as privileged denial tests.

## Decisions (running log)

- Findings 1–3 are real issues derived from the reviewed source. Guard each pipe close independently, retain close diagnostics, finish bounded cleanup, and make a final WNOHANG observation after the last sleep, even when scheduling reaches the fixed cutoff. A late synchronous creation receives one exact-PID KILL while wait ownership still pins the identity, followed by a final nonblocking observation; it gets no new wait allowance. Record any deadline overrun as failure. This repairs cleanup without claiming creation isolation.
- Finding 4 is a real diagnostic/ownership issue for ECHILD: distinguish lost wait ownership from a live pinned residual, release the stale owner, preserve unknown exit status and the failed observation, and forbid further signals. Other wait errors retain ownership conservatively and close signal authority.
- Finding 5 is dead code: remove the controller's obsolete TimeoutExpired handler; capture receipts remain the source of helper diagnostics.
- Finding 6 states a test boundary rather than a runtime defect. Keep controller adapter tests as reaction controls, real helper tests as pipe/KILL/reap evidence, and deterministic timing/wait adapters explicitly labelled. No injected fault establishes genuine permission denial or an uninterruptible kernel task. Preserve k47-check-01 and every earlier condition unchanged.
- Retained RED `k50-red-01` reproduced the five new failure cases and also failed the existing `test_review_pinning_control_detects_real_early_reap[poll]`: its receipt observed a zombie after one nonblocking poll. The control must confirm the injected reap before testing released identity; bound that polling to one second and fail explicitly if no reap occurs. This repairs the instrument rather than changing the controller's pinning contract. Also keep the thread-liveness assertion with its original admission control after inserting the new tests.
- `k50-final-01` retained 135 passing tests and clean Ruff checks, but strict Pyright rejected the unparameterized list factory for `close_errors` (`reportUnknownVariableType`). Specify `list[str]` in the factory and retain this failed condition before a fresh full check.
- Final `k50-final-02` passes Ruff lint/format, strict Pyright and all 135 tests; pytest checker elapsed 26.675 s. Independent `k50-audit-01` passes frozen input/artifact/JUnit reconciliation across k47-check-01, k47-final-01 and all three k50 conditions. All 1,498 recorded real PIDs are absent; 417 retained-argv/session-enumerated fixture directories are reconciled, with preflight inventories copied unchanged. Fake PIDs 123456789 and 987654321 are excluded explicitly. Every evidence file is included in jj, using a command-local snapshot limit for receipts above 1 MiB. Earlier k47 evidence is unchanged.
- All Done-when items are met: each finding has a disposition above, close faults retain complete receipts and final reap observations, last-sleep and late-creation behavior have controls, and failed conditions remain retained. No ADR changes are needed for this bounded repair. Worker/scheduler and synchronous creation isolation remain live in k48; therefore this retirement closes no parent node.
