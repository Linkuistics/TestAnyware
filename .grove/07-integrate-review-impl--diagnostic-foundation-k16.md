# diagnostic-foundation-k16

**Integrates:** diagnostic-foundation-k15

## Goal

Triage the findings of review `diagnostic-foundation-k15` against the diagnostic
controller of `diagnostic-foundation-k11`. Apply the findings that are real
before a causal inquiry uses the controller.

## Context

- Read the findings in the review leaf for `diagnostic-foundation-k15`, as its
  commit records them. Its line numbers refer to producer commit `dfd48c9`.
- The implementation is under `experiments/exec-reliability/`. The durable
  handoff is `docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

- Each finding has a recorded decision: applied, rejected with a reason, or
  moved to a named later leaf.
- Each applied controller change has a failure control. Show that control fail
  against the old behaviour before you credit its pass.
- A changed controller is a new condition. Retain the earlier self-check
  evidence unchanged, and record the new run under a new directory.
- The report states each changed limit and each route fact that remains open.

## Notes

Do not invoke `agent:macos:test`. The leaf `macos-completion-k3` owns the scope
of its unsafe cleanup. Do not quit or replace the installed app. A live guest probe
must follow the root brief's common investigation contract. Declare its bounds
before the run. Keep the historical 0/10 unavailable trial unchanged.
