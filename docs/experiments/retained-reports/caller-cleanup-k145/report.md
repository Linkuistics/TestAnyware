> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Caller safety-driver cleanup, k145

This integrates F1–F4 from the committed k144 review (`d1811ef7`) of the k140 caller-scheduling seam. A supplied driver can raise during retirement or close. Those exceptions now propagate after owned channel finalization and receipt finalization, while the original process cutoff and retained ownership still apply. The aggregate production scheduler and overlapping teardown remain in complete-worker-scheduler-k143. k28/k17 remain open; guest acceptance remains 0/10 unavailable.

## Findings and decisions

F1 is a real descriptor leak. `capture_helper` now nests unconditional finalization around retirement, worker metadata and persistent close. The retirement driver's exception still runs the final nonblocking reap, subject to the existing owner checks. `Worker.close` finalizes channels if its driver raises before shutdown completion. These paths add no fallback driver, signals, waits or renewed process allowance. Unresolved process obligations remain recorded.

Eight deterministic controls use real pipes across standalone retirement, persistent retirement, persistent close and direct close, each with RuntimeError and OSError. The process boundary uses a synthetic already-reaped owner; no child or signal is created. Each case requires the same exception object to reach the caller, owned descriptors to reject `fstat` with EBADF, peer descriptors to stay open, and applicable receipts/shutdown fields to finish without extending the cutoff. Test cleanup releases every allocated descriptor even when the assertion fails.

F2 is an unclear contract. Both entry-point docstrings and the k140 report now require a supplied driver to synchronously advance every Turn, including safety retirement/close Turns, to completion or raise. Returning with a pending Turn violates that precondition. An extra completion-probing Turn could perform work outside the caller's chosen schedule, so this integration adds neither a probe nor a private fallback.

F3 is a real precision error in the k140 explanation. The query-bypass mutant has three caller-escape failures and two secondary `assert (1 == 2)` failures at frozen `test_turns.py:447`: `test_request_turns_yield_with_transport_priority_and_caps[True-False-False]` and `[True-True-False]`. The k140 report now states those mechanisms; its retained mutant results are unchanged. `k140-report-before.md` retains the original explanation.

F4 is a real provenance omission. `runner-provenance.json` attributes the exact runner hashes to all ten original conditions using their retained before/after subject records and verdict digests:

- `ebb966a58a0e489462f608aae26b28010410ee7be27ea6b2fb9b21cf731bd6eb`: RED, parent-full, candidate-controls and candidate-v2-controls.
- `649f85d17b1967a5d04e7027a883f2369d6f42e99927c60c67d463d009ab552b`: candidate-v3-controls, candidate-v4-full and all four bypass mutants.

Each original condition has stable subjects within its run. Across those groups the checker changed; equal checker code is not claimed. Earlier runner bytes cannot be recovered from a digest alone. The current k140 runner and every retained condition remain unchanged. New checks use the separate k145 runner, which validates boolean flags, positive checker/worker identities, query/retirement phase ordering and outcomes, and interrupted-creation accounting. Twelve audit-tool controls cover missing, unmatched, malformed and inconsistent journals plus a valid completion. Journal accounting and independent process-absence observations remain distinct.

## Reproduction and retained measurements

Use the repository Taskfile through `mise exec -- task`. `diagnostic:caller-fix-prepare INPUTS=. OUT=<new-directory>` freezes all thirteen Python sources, manifest, lock and Taskfile; `MUTANT=skip-capture-close` or `skip-worker-close` requires exactly one matching replacement. `diagnostic:caller-fix-check INPUTS=<frozen-directory> OUT=<new-results> MODE=controls|full|static` uses isolated Python 3.12. `diagnostic:caller-fix-tools CLI_ARGS=--check` verifies the new tools. `comparison-plan.json` records the sequence and bounds: controls 60 seconds, full 120, static/tool checkers 30 each, timeout TERM 2 then KILL/reap 2. Results include exact commands, before/after subject hashes, XML item outcomes, stdout/stderr, fixture journals and artifact hashes. Each output directory has one writer.

The test-first RED sample retains eleven passing caller cases and eight real-descriptor failures against the original runtime. Each failure is `Failed: DID NOT RAISE OSError` at the assertion that the owned descriptor is closed. The first candidate-full sample passes all 939 runtime cases but fails the overall gate: one contradictory assertion in the audit control, five overlong string lines and ten strict-type diagnostics in the checker. Those results remain intact. The corrections affect that test assertion, checker typing and string layout; fresh v2 inputs/results follow them. `observed-deviations.json` records these corrections and the inventory compaction.

## Final measurements

`comparison.json` records item outcomes, verdict digests and checks of every recorded artifact digest. All five measured conditions have identical before/after subjects, reaped checkers, resolved fixture accounting and no checker timeout. Neither full condition passes its overall gate; preserve that distinction when using the runtime results.

| Condition | Runtime passed / failed | Overall result |
| --- | --- | --- |
| red-result | 11 / 8 | Expected descriptor failures against original runtime |
| candidate-full | 939 / 0 | Failed audit/static gate, retained before corrections |
| candidate-v2-full | 938 / 1 | Failed runtime gate; all audit/static checks pass |
| skip-capture-close-v2-result | 17 / 2 | Rejected at standalone descriptor assertions |
| skip-worker-close-v2-result | 13 / 6 | Rejected at persistent/direct worker descriptor assertions |

All eight new cleanup cases pass in both full conditions. Both exception types reject each relevant closure mutant with `Failed: DID NOT RAISE OSError` at the real descriptor assertion. The final full run also passes all twelve audit-tool cases, Ruff, formatting and strict Pyright. Its 31 fixture audits, four expired-request journal pairs and eight caller journal pairs are resolved; these accounting results do not erase the runtime failure.

The final runtime failure is `test_controller::test_review_live_group_is_failure_even_with_expected_leader_exit[True]`, at frozen `test_controller.py:1708`: `assert -9 == 0`. It is not a cleanup-control failure. Its cause is unresolved; neither a flaky-test diagnosis nor a repair is claimed. The first full sample passed that case, which does not explain the later failure. k143 retains this observation and the work needed to investigate it before aggregate acceptance. No rerun replaces the failing sample.

Against the historical k140 candidate-v4-full, each new full condition has 931 common cases, zero removed cases and eight added cleanup cases. The first full condition has no changed common outcome. The final full condition has one changed common outcome, the group-leader failure above; its other 930 common cases pass. The different checker versions and conditions prevent a causal attribution from these counts.

`diagnostic:caller-fix-preserve MODE=after` passes for 285,660 earlier evidence paths. `prior-after.json` retains the same path/content commitment as the original inventory, `747c775de1e6d5db0073b4e08816a028acd26d80f1d42ea23172eb448c3baf36`, with only the permitted k140 report hash changed. The inventory excludes the new k145 directory and interpreter/test/lint caches. The initial detailed inventory was compacted from its already captured records, rather than recaptured after edits; the length-prefixed sorted path/hash commitment preserves the check. Every retained sample, mutant, runner and receipt covered by that inventory remains unchanged.

## Scope and limits

The controls establish descriptor finalization for raising safety drivers at this synchronous seam. They do not establish production fairness, actual concurrent ownership, overlapping teardown or every scheduler invariant. The historical failures and final aggregate review remain binding in k143. Executable descriptor controls and single-site closure mutations directly test this repair; no duplicate adversarial reviewer of k144 ran here.

Graph project Users-antony-Development-AbstractingComputers-TestAnyware.gh-issue-14 was ready at generation 2026-11-15T02:19:52Z, but relevant paths had changed metadata or were untracked. Material claims use current source reads after coverage checks; no graph absence or completeness claim is made. No guest, installed application or unrelated process was changed.
