# complete-worker-scheduler-k69

## Goal

Publish explicit helper fork failure with its true errno through the interim worker control channel, preserving confirmed no-child evidence when the terminal receipt is lost.

## Context

Read k48 and ancestor briefs and the k67 producer/review. The full unchanged scheduler charter is carried by flat sibling complete-worker-scheduler-k71. This independently useful protocol slice does not complete any aggregate ancestor.

## Done when

- Publish only an OSError raised by the actual helper os.fork call, with positive errno, request token, immutable deadline and monotonic failure time. Setup/admission/exec errors must not masquerade as fork failure.
- Validate fields, types, request/deadline and timing before accepting no-child evidence. Preserve confirmed errno when terminal publication is lost; query still fails. Silence, malformed/late publication and conflicting creation remain conservative. Known helper identities remain retained through matching reap acknowledgement.
- Require terminal no-child errno/admission/timing consistency with the publication. Exercise genuine helper-boundary injection separately from setup failure, lost terminal, unknown creation, malformed/duplicate/conflicting publications and healthy helper success.
- Retain RED/GREEN immutable evidence and independent exact-identity audit; preserve previous evidence unchanged. Schedule separate Grove review before k71. No guests, app changes, privileges or unrelated process signals.

## Decisions (running log)

- 2026-11-15: Narrowed to explicit helper fork error after source verification showed interrupted terminal delivery conflates a failed fork with unknown creation. The complete original contract and final aggregate review obligation are preserved verbatim in complete-worker-scheduler-k71; k48 and all aggregate ancestors stay open.

- 2026-11-15: Behavioral doubt: a confirmed helper fork error must survive lost terminal publication without erasing a known identity or accepting conflicting evidence. Retained RED exposes missing classification; final GREEN has 311 passing cases and lint/format/strict types all zero. Twenty-three new controls cover actual worker boundaries and protocol corruption. EAGAIN/EMFILE are injected at the named boundary, not induced kernel exhaustion.
- 2026-11-15: Independent k69-audit-01 passed: 1,808 positive retained PID selectors absent; 14 existing identity plus 23 fork-failure checks, 833 scratch directories, three exact mock markers disposed; 65,742 prior evidence files unchanged. Later absence never repairs runtime unknown/reap evidence. See experiments/exec-reliability/evidence/helper-fork-error-k69/report.md. Separate review required for conservative classification/ownership conflicts; full charter and final aggregate review remain k71.
