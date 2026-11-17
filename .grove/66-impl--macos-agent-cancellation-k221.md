# macos-agent-cancellation-k221

## Goal

The macOS agent accepts explicit cancellation, retains finished records and
sends each download from one open file handle.

This leaf is the second part of candidate C. Leaf 63 owns the first part.

## Context

- Spec: sections "Exec record", "Evidence transfer", "Bounds", "Test seams and
  required controls" and candidate C.
- ADR-0025 and ADR-0026.
- Old failing paths: `diagnostic:macos-disconnect-availability-host` and
  `diagnostic:download-consistency-local`.
- Retired leaves 40 to 42 and 50 to 52 hold the macOS causal evidence.

## Done when

- A health request during a command replies within 1 s.
- `/exec/cancel` meets the same contract as leaf 64. That covers the 5 s reply
  bound, idempotence, the exit-and-cancel race and the tombstone order.
- The agent retains each finished record for the retention time in the spec.
- A client disconnect does not cancel the command or change its record.
- A download reads one open file handle and sends exactly one file version.
- Each control below fails on the old path and passes on the new one:
  - `diagnostic:macos-disconnect-availability-host`, extended with a
    cancellation case, shows that a disconnect changes nothing.
  - `diagnostic:download-consistency-local` shows one file version.
  - `agent:macos:test` covers cancellation, idempotence and the tombstone
    order.
- `agent:macos:test` passes.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The single-handle download does not make an in-place write readable as one
version. The spec excludes that case.
