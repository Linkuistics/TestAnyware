# lifecycle-evidence-k214

**Reviews:** lifecycle-evidence-k8

## Goal

Challenge the guest command lifecycle design of lifecycle-evidence-k8 before
any planner relies on it. Find each premise, bound or contract that the
evidence does not support or that an implementation cannot meet.

## Context

The producer commit names lifecycle-evidence-k8. Re-derive every citation from
that commit. The artifacts under review are these:

- `docs/specs/guest-command-lifecycle.md`, the specification.
- `docs/adr/0023-guest-command-outcome-is-a-typed-record.md`.
- `docs/adr/0024-one-owner-ends-each-guest-command.md`.
- `docs/adr/0025-cancellation-is-explicit-and-disconnect-is-not-cancellation.md`.
- `docs/adr/0026-a-download-sends-one-open-file-version.md`.
- The new glossary terms in `CONTEXT.md`.
- The six views in `docs/design/guest-command-lifecycle/`. To preview them,
  run `mise exec -- task design:lifecycle-preview`.

The causal evidence is in the retired leaves 01 to 55 and their reports. The
root brief `.grove/_BRIEF.md` holds the acceptance cases, byte oracles and
ceilings.

The ADRs use the numbered format of this repository, not the Grove ADR format.
Review their content, not that format choice.

## Done when

- Challenge the causal premises. Check that each row of the spec's evidence
  table matches its report. Check that the established, non-reproduced,
  unavailable and unresolved classes match the evidence. Check that no design choice depends
  on an unresolved cause, such as op 08 or the macOS post-timeout stall.
- Challenge races and ownership. Check the arbitration at exit, deadline and
  cancellation races. Check the tombstone order, the held leader and the
  identity checks. Check the 2 s grace, the 2 s kill-and-reap bound and the
  limit on escaped descendants.
- Challenge evidence consistency. Check that the record validity rules reject
  each contradiction. Check that a guest outcome never sets `ok` to false.
  Check the single-handle download, the framing checks and the exclusion of
  in-place overwrite.
- Challenge compatibility. Check the skew matrix for macOS, Linux and Windows
  agents and for old and new Host CLIs. Check that legacy fields keep their
  meaning for consumers that use only the CLI. Check that ADR-0004 routing and
  ADR-0001 bounded-memory streaming stay intact.
- Challenge acceptance feasibility. Check that each guarantee has an old-fail
  and a corrected-pass control at a real seam. Check that the validation route
  can reach the frozen five-clone-per-OS trial with every root case.
- Record each finding with its file, line, evidence and severity. A clean
  review cuts nothing.
- If a finding needs a change, insert an `integrate-review-design` leaf before
  `lifecycle-delivery-k213` with `grove-llm leaf-insert`. If a finding needs a
  substantial redesign, insert a new producer and review chain before that
  planning leaf instead.

## Notes

Do not implement any candidate. Do not repeat the requirements interview.
Escalate only a material owner trade-off that the design exposes, with a
recommendation and evidence.
