# complete-worker-scheduler-k58

## Goal

Reject contradictory terminal helper status in the interim worker result decoder.

## Context

The k59 review found no findings in k55. The actual per-query transport already checks types, timing and positive PIDs but accepts status without reap, reap without status, and reap together with lost ownership or a reap error. This slice validates those combinations only. Synthetic helper publications exercise the real worker fork/reset/channels/decoder/shutdown; existing genuine helper controls remain required.

## Done when

- Returncode is present exactly when helper reap is reported; reaped helpers report neither reap_errno nor ownership_lost.
- Retain test-first regression evidence for all four contradictory combinations and accepted coherent success, nonzero status, unreaped error and lost-ownership error publications. Require exact rejection, output disposal, conservative unknown creation and real worker reap.
- Preserve genuine success, missing executable, early exit and killed-helper behavior. Freeze every runner/audit input, preserve failed samples and all prior evidence, compare individual cases and independently audit recorded process identities and owned scratch paths.
- Record limitations in the diagnostic report, cut a separate review of this delivered artifact and leave the full original runtime contract live in flat sibling complete-worker-scheduler-k60. Keep all ancestors open.

## Decisions (running log)

- 2026-11-14: Flat sibling split ships a bounded protocol consistency improvement. k60 owns every original k58 requirement, including persistent bootstrap, gated helper identity, independent ownership, shared scheduler, remaining lifecycle/error consistency and full control inventory. No aggregate completion, authenticated helper identity, guest, privilege, installed-app, stalled-storage or ten-clone acceptance claim; acceptance remains 0/10 unavailable.

## Validation

- k58-red-01: four intended failures, 196 passes; k58-fixed-01: all 200 pass. Lint, format and strict types pass in both. Sources frozen before each run, identical per-case inventory; only four decoder lines change between RED and final production sources.
- k58-audit-01 retains a Ruff unused-variable failure before execution. k58-audit-02 passes immutable subject and prior-evidence verification, individual receipt checks, external oracle controls, 994 positive-selector absence checks and 346 owned fixture directory reconciliations.
- Report: docs/experiments/exec-reliability-diagnostic-foundation.md, k58 section. All original runtime work stays live in k60; no ancestor closes. No GROVE_PROVIDER_EVIDENCE in the inherited environment; no Creator receipt may be invented.
