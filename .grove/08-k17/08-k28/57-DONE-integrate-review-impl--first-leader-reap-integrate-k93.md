# first-leader-reap-integrate-k93

**Integrates:** first-leader-reap-k92

## Goal

Triage the findings of the k92 review of the k88 first workload-leader reap boundary. Apply each real one. Keep the one-line k88 loop change unless a finding proves it wrong. Keep the k91 aggregate charter unchanged.

## Context

Read the findings from the k92 commit, not from this body. Reviewed commit: 6b678ed (complete-worker-scheduler-k88). Source: `experiments/exec-reliability/controller.py` (`_reap_leader`), `test_controller.py` first-reap cases, `Taskfile.yml` `diagnostic:leader-reap-*` tasks. Evidence: `experiments/exec-reliability/evidence/leader-reap-k88/`. Contract: `docs/specs/diagnostic-process-inspection.md`.

## Done when

- Classify each finding as a contract stated unclearly, a real issue, a trade-off or noise. Record the class in the decision log.
- For each real runtime or evidence finding, add a red control first. Then change the artifact. Then retain green evidence in a new directory.
- Preserve all earlier evidence bytes. Run the full diagnostic self-check and retain it, failures included.
- Route anything that belongs to the aggregate scheduler to complete-worker-scheduler-k91 by name. Do not fix it here.

## Decisions (running log)

- Created by the k92 review. Classification is pending.

- F1 is a real receipt issue: the first-attempt loop is correct, but receipts do not distinguish an attempted WNOHANG `(0, 0)` from no attempt. Add durable attempt/result evidence without another clock read or a change to K; describe an unreaped leader at the fixed deadline accurately.
- F2 is a real evidence issue: the prior manifest mixes tracked history with ignored local installation/cache files and omits tracked files. Preserve it unchanged; create a separate exact jj-tracked inventory with relative paths, and portable resolution for historical absolute subjects.
- F3 is a real audit issue: an unrelated archive can satisfy a changed subject, and the missing-record negative control is insufficient. Replace the runnable audit in a new evidence directory with identity-bound historical exceptions and explicit corruption controls; retain the old audit bytes.
- F4 is a real coverage issue: add exactly-K success, ready/ECHILD through `_call`, and no-later-signal/probe checks; retain an inclusive-boundary mutant rejected by those tests. Simulated withheld waits/ECHILD remain explicitly simulated; actual kernel stalls are not induced. Shared scheduler and stale callback coverage remain owned by complete-worker-scheduler-k91, whose charter stays unchanged.
- F5 is a real discoverability issue: place all three leader-reap task recipes together and direct them at the repaired evidence tools. No contract redesign or aggregate completion claim is needed.

- Final evidence policy: retain interim red-2/green-1/exclusive-1 but do not certify their changed runner/Taskfile subjects, whose exact bytes were not archived before editing. Repeat the same 25 case identities using frozen final inputs in red-3/exclusive-2/green-2; run the actual diagnostic:self-check in full-1. The final auditor must distinguish these observations from fully certified samples.
- Full diagnostic:self-check full-1 passed all 469 cases plus lint/format/strict types; focused green-2 passed 25. Final red-3 failed the same 15 receipt cases, and exclusive-2 failed exactly the two success-at-K cases. audit-final-1 exposed an auditor expectation error (historical full-1 uses self_check checker names/order); audit-red-2 exposed the legacy harness cwd requirement. Retain both failures, archive their exact helper bytes in tools-1 before correction, and bind each earlier subject to its own archive by sample and identity.
- audit-final-2 passed exact 92,812-entry inventory, all historical and certified current samples, and unchanged before/after hashes. Bounded legacy audit-red-3 hit its unchanged 20-second ceiling and was TERM-reaped; it is a retained timeout, not fresh evidence of acceptance. Earlier audit-red-1 remains the observed original-auditor acceptance of both injected corruptions. audit-controls-1 passed 12/13, including both final-entrypoint rejection controls; its relocation assertion incorrectly treated the checkout-local .venv executable as an external host dependency. Archive helpers in tools-2 and correct the test to check both relocated venv and resolved external interpreter identities.
- audit-controls-2 retained a 20-second timeout in the unrelated-archive final-entrypoint case after the other 12 controls passed, including the corrected relocation case. A concurrent final audit overlapped the tail of that run. Keep the failure visible; run one isolated repetition at the same 20-second ceiling after the final audit finishes, with unchanged case identities and no tool edits.
- The isolated audit-controls-3 also exhausted the fixed 20-second ceiling in the final unrelated-archive rejection. This is an auditor ordering issue: an invalid subject waits behind the whole 92,812-entry inventory hash. Reject malformed subject identities before expensive hashing, then repeat identity checks within the full before/after certification path on successful runs. Preserve the timeout samples and tools-3 bytes; keep every ceiling and all 13 control identities unchanged.
- Settled: audit-controls-4 passed all 13 unchanged control identities in 2.08 pytest seconds (2.31 outer seconds); all subjects match current bytes and the checker was reaped. audit-final-4 passed the 92,812-entry exact tracked inventory, historical/current identities and hashes, and before/after equality. Tools pass Ruff/format/strict Pyright; auxiliary controls pass Ruff/format. Full report: experiments/exec-reliability/evidence/leader-reap-k93/report.md, including all setup failures, uncertified interim observations, helper revisions and retained timeouts. The manifest was snapshotted with a one-command 25,000,000-byte new-file override; no global configuration changed. All F1–F5 are integrated. Aggregate work remains complete-worker-scheduler-k91; no ancestor closes.
