# complete-worker-scheduler-k75

## Goal

Reject malformed result framing incrementally in the interim per-query worker transport, without waiting for channel EOF or consuming payload beyond the declared frame plus one detection byte.

## Context

The full original charter and decision history continue unchanged in complete-worker-scheduler-k78. Read the process inspection spec and k71/k76/k77 before relying on cleanup-only identity parsing. This independently useful slice keeps original query/shutdown deadlines, bounded per-turn reads and distinct helper obligations.

## Done when

- Validate a complete result header immediately; reject oversized declarations, trailing bytes and truncated result EOF even while the control channel remains open. Limit reads to the header and declared remainder plus one detection byte, preserving the 1MiB frame cap and 64KiB combined per-turn cap. Do not decode terminal success before both clean channel ends.
- Demonstrate real pipe/process RED controls for oversized header, partial header EOF, partial body EOF, trailing frame bytes and cap overflow, with an open control channel. Verify fragmented valid delivery with real helper completion. Preserve fail-closed creation accounting and cleanup-only identities under original bounds.
- Retain frozen RED/GREEN sources, itemized verdicts and independent exact-identity audit, preserving all prior evidence. Run static checks and full diagnostic suite. Schedule separate review before k78.
- Keep k78/k28/k17 open. No guests, app changes, privileges or unrelated process signals. Ten-clone acceptance stays 0/10 unavailable.

## Decisions (running log)

- 2026-11-15: Flat sibling split because incremental framing is independently useful and verifiable; k78 owns every original requirement and the final k28 review. The current receiver checks frame length only after both channels close and can read a full 32KiB past a declared boundary. Validate structure as bytes arrive while preserving late control identity retention.
- 2026-11-15: Python >=3.12 and locked diagnostic tooling govern. Binary header parsing uses existing struct.Struct without new dependencies (Python 3.12 struct docs: https://docs.python.org/3.12/library/struct.html#struct.Struct). Grove tree mutation uses installed 22.9.0 leaf-add --help.
- 2026-11-15: First full check exposed three creation-accounting regressions: early framing exceptions bypassed the old post-EOF conflict guard. The fixed cleanup path preserves conflict for partial control/terminal bytes before confirming a fork failure. Two older hostile-frame cases legitimately reject before the all-channel receipt timestamp; their uncertainty/ownership checks remain. Retain green-full-01 (five test failures and optional-int type error) unchanged.
- 2026-11-15: Final frozen self-check green-full-02 passes 357 tests plus lint, format and strict types; all checkers reaped without timeout. Independent frozen audit reconciles five conditions, 2,614 unchanged subjects, 78,030 prior evidence files, 1,397 exact positive PID selectors absent and 670 scratch directories (two exact plan-owned mock markers disposed). See experiments/exec-reliability/evidence/result-framing-k75/report.md; exploratory red.xml lacks recovered receipts and is excluded from that census.
- 2026-11-15: Schedule separate review immediately ahead of k78 via installed `grove-llm leaf-insert --help`; preserve the full unchanged aggregate remainder and final k28 review. No ADR change: this is an implementation correction within the existing contract. Current GROVE_PROVIDER_EVIDENCE is absent; no Creator is invented or copied from another attempt.
