# diagnostic-disposal-k160

**Integrates:** diagnostic-disposal-k159

## Goal

Reconcile the committed design review before `evidence-storage-k158` plans the
implementation. Verify each finding against the design, requirements and source.
Repair the acceptance-blocking or materially risky issues. Record justified
rejections or accepted tradeoffs.

## Context

Read the findings from the commit retiring `diagnostic-disposal-k159`.
The producer is `diagnostic-disposal-k18`. Its artifact is
`docs/specs/diagnostic-evidence-storage.md`, accompanied by
`docs/design/diagnostic-evidence-storage/` and its section of
`docs/experiments/exec-reliability-diagnostic-foundation.md`.

## Done when

Each review finding has an evidenced disposition. Accepted repairs are
consistent across the spec, visual document and recorded design decisions, with
verification appropriate to the repaired contract. Planning has a coherent
design to slice and no unresolved acceptance-blocking issue silently passed to
it. Any necessary redesign has an explicit owner ahead of dependent work.

## Notes

This is a flat sibling under the existing root because it shares the producer's
scope and inherited context. Its position immediately before storage planning
preserves the review's source coordinates until triage. The findings belong to
the review. This task does not prescribe which ones to accept.

## Decisions (running log)

**Method.** I compared each k159 finding with the spec and with
`experiments/exec-reliability/controller.py`. The current controller writes
`result.json` and then the identity record. It pins call streams after the
reap. The spec kept neither order, so the findings apply to the spec.

**F1, result boundary: real issue, repaired inline.** The result frame
precedes its own acknowledgement and the close, so it cannot hold the final
publication outcome. The spec now makes the result a snapshot with a stated
cutoff and publication `pending`. Digest entries cover only earlier records,
so the coverage has no cycle. A terminal section in the in-memory outcome
holds the final state. A completion authority rule tells a retained reader
when a directory is complete.

**F2, identity order: real issue, repaired inline.** The identity request
preceded the storage-close record, and the retention verifier rejects an
unlisted file. The close frame now starts one writer transaction. The writer
creates the storage-close record, computes identity over every file and
acknowledges both digests. The contract of `experiments/exec-reliability/retention.py`
changes with the writer.

**F3, final output: real issue, repaired inline.** Closing the read ends at
the final observation lost bytes that a pipe still held. A bounded final drain
now reads them without a wait. Each stream records `eof`, `writer_open` or
`cutoff`, and only `eof` can pass. The drain adds no cleanup or disposal
allowance.

**No redesign or new review round.** The repairs keep the separate
writer, every phase bound and the teardown deadlines. Each repair narrows
an existing rule, so no ADR and no other owner is necessary. Four new controls
in the spec cover the repaired rules. `evidence-storage-k158` plans from the
repaired spec.

**Verification.** The spec, report and diagram text pass the
technical-communication checker with no hard finding. The local viewer
rendered all three diagrams with no Mermaid error. No code changed, and no
control ran.
