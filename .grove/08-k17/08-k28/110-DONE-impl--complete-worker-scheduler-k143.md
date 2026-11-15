# complete-worker-scheduler-k143

## Goal

Repair the local scope-parent fixture so TERM ends its child-readiness wait within the existing 150 ms grace. A held-readiness control sends a real signal after the fixture declares its exact child PID. The control retains parent and child observations. Its independent owner cleans up and audits both identities.

## Done when

- The new control fails on the old readiness loop and passes after the loop observes the stopping flag. Controller bounds remain unchanged.
- A frozen full diagnostic suite and existing audit/static checks run. Every failure and every historical evidence byte remain retained.
- A durable report explains the proven race. It leaves k145's historical failure unresolved because its receipt lacks readiness evidence.
- Complete-worker-scheduler-k146 preserves the complete original aggregate charter and history verbatim. k28 and k17 remain open. Guest acceptance remains 0/10 unavailable.

## Decisions

- Flat split: this independently useful fixture repair has a real subprocess control and shares the existing k28 context. k146 owns every original aggregate requirement. Production overlap remains there.
- Root-cause hypothesis: scope-parent ignores TERM in its readiness loop. A slow child can consume the whole grace, then receive KILL. The control withholds readiness and declares the child PID before TERM. This does not establish k145's historical cause.
- The frozen red controls reproduced PID-only and group TERM exceeding 150 ms before child readiness. They recorded two failures and 19 passes. Independent audits found both exact identities absent after cleanup. The repair adds the stopping condition to the readiness loop. Every existing controller deadline remains unchanged.
- The initial candidate gate passed 21 controls, 941 full cases, 12 checker controls and static checks. Source inspection found a separate instrumentation race. The PID declaration became visible before its contents were complete. Atomic replacement now publishes the complete PID. The initial passing samples remain retained. Final measurement uses frozen sources and the same bounds.
- The final old-loop mutation rejects both real signal cases. The final candidate passes 21 controls, all 941 full cases, 12 checker controls and static checks. Per-case comparisons show identical case membership. Subject hashes remain unchanged during every condition. The preservation inventory reports no changes across 289,810 prior evidence paths.
- The durable report is experiments/exec-reliability/evidence/scope-readiness-k143/report.md. k145's historical failure remains unresolved. The final aggregate scheduler and independent k28 review remain with k146. This fixture repair needs no separate adversarial review.
