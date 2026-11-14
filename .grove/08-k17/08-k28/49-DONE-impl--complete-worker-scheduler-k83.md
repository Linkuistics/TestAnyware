# complete-worker-scheduler-k83

## Goal

Close worker signal and exact-child reap entry points at the original per-query shutdown deadline. Preserve an unreaped worker as a strong residual owner without later runtime signals or reaps.

## Context

The original aggregate charter, unchanged done criteria and prior decisions are carried in complete-worker-scheduler-k85. That flat sibling owns the ENTIRE persistent pre-resource worker/shared scheduler remainder and final bounded-inspection-k28 review. This independently useful interim-transport repair closes no aggregate ancestor.

## Done when

- Fix the deadline on the worker owner at creation. Every worker signal/reap entry point refuses calls at or after that deadline, including stale callbacks and final cleanup after the loop.
- Preserve PID-only signals before confirmed readiness, group signals after readiness, exact-child WNOHANG before cutoff, and strong residual ownership after cutoff. Do not change helper-local late-creation cleanup semantics.
- Retain RED/GREEN regression conditions using real owned worker children and an explicitly labeled deferred-wait-observation adapter. Independently reap each fixture after the runtime returns; preserve raw failed receipts and all prior evidence bytes.
- Run the full diagnostic self-check and report measured limits. Schedule a separate review-impl of this slice before k85 consumes it.

## Decisions (running log)

- 2026-11-15: Flat sibling split keeps this leaf as the first independently useful green slice because owner entry points currently permit an unconditional final WNOHANG after the loop deadline and stale signals on residual workers. complete-worker-scheduler-k85 owns every original aggregate requirement and control, unchanged. No persistent worker/shared scheduler claim follows from this repair.

- 2026-11-15: Frozen RED six-case matrix failed at the intended after-deadline OS-call assertion. Store the original shutdown deadline on each worker owner and refuse both _signal/_reap at >= cutoff. Keep helper-local semantics unchanged; retain strong residual owners. Samples cannot prevent OS preemption/syscall overrun, so elapsed overruns remain failures.

- 2026-11-15: Expanded identical RED/GREEN matrix to eight cases, including exact cutoff equality. All eight fail against the original runtime and pass against the changed runtime; full self-check passes 432 cases plus Ruff/strict pyright. Separate exact-child audits reap all GREEN fixtures; evidence reconciliation checks case identities and 89,102 prior file entries. Read [report](../../../experiments/exec-reliability/evidence/worker-deadline-k83/report.md) for raw evidence and inventory/clock limitations. This is producer verification; schedule independent review before k85 consumes the boundary.
