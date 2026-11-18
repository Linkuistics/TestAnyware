# macos-agent-cancellation-k221

## Goal

The macOS agent accepts explicit cancellation, retains finished records and
sends each download from one open file handle. It shuts down within bounds.

This leaf is the second part of candidate C. Leaf k219 owns the first part.

## Context

- Spec: sections "Agent shutdown", "Cancellation and CLI interruption",
  "Responsiveness", "Exec record", "Evidence transfer", "Bounds", "Test seams
  and required controls" and candidate C.
- ADR-0025 and ADR-0026.
- Old failing paths: `diagnostic:macos-disconnect-availability-host` and
  `diagnostic:download-consistency-local`.
- Retired leaves 40 to 42 and 50 to 52 hold the macOS causal evidence.

## Done when

- A health request during a command replies within 1 s.
- `/exec/cancel` meets the same contract as leaf k220. That covers the 5 s reply
  bound, idempotence, the exit-and-cancel race and the tombstone order.
- The agent retains each finished record for the retention time in the spec.
- A client disconnect does not cancel the command or change its record.
- A download reads one open file handle and sends exactly one file version.
- At SIGTERM, the agent meets the spec's "Agent shutdown" section, as in leaf
  k220. A VM stop does not replace this check.
- Each control below follows its row in the spec's required-control table.
  An old-fail control fails on its old path and passes on the new code. A
  new-behaviour control has no old failing path, and it passes on the new
  code. A compatibility control equals its recorded old reference.
  - Corrected-pass: a health request during a command replies within 1 s.
    The old-fail case of this row is Linux.
  - Old-fail: `diagnostic:macos-disconnect-availability-host` fails with the
    shipped agent. Extended with a cancellation case, it shows that a
    disconnect changes nothing.
  - Old-fail: `diagnostic:download-consistency-local` fails with the shipped
    agent and shows one file version with the corrected agent.
  - New behaviour: `agent:macos:test` covers cancellation, idempotence, the
    exit-and-cancel race and the tombstone order.
  - New behaviour: `agent:macos:test` covers SIGTERM during a command. A
    second case holds a leader whose status stays unavailable.
- `agent:macos:test` passes.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The single-handle download does not make an in-place write readable as one
version. The spec excludes that case.
