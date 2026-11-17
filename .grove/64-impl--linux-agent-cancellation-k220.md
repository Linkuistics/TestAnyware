# linux-agent-cancellation-k220

## Goal

The Linux agent stays responsive while a command runs. It accepts explicit
cancellation, retains finished records and sends exact-length downloads.

This leaf is the second part of candidate B. Leaf 61 owns the first part.

## Context

- Spec: sections "Exec record", "Evidence transfer", "Bounds", "Test seams and
  required controls" and candidate B.
- ADR-0025 (cancellation is explicit and disconnect is not cancellation) and
  ADR-0026 (a download sends one open file version).
- Old failing paths: `diagnostic:linux-disconnect-container` and
  `diagnostic:linux-download-consistency-container`.
- Retired leaves 47 to 49 and 53 to 55 hold the Linux causal evidence.

## Done when

- The agent server serves each request on its own thread. A health request
  during a command replies within 1 s.
- `/exec/cancel` ends the command through the lifecycle owner of leaf 61. Its
  reply waits at most 5 s (`waitMs` 5000).
- Cancellation is idempotent. The exit-and-cancel race resolves by the spec's
  arbitration rule. The tombstone order holds.
- The agent retains each finished record for the retention time in the spec.
- A client disconnect does not cancel the command or change its record.
- A download sends exactly the declared length of one open file version.
- Each control below fails on the old path and passes on the new one. Each
  runs in `agent:linux:test`:
  - The agent stays responsive during a command.
  - A disconnect changes nothing.
  - Cancellation, idempotence and the tombstone order hold.
  - A download sends no extra bytes.
- When the change exists, insert a `review-impl` leaf directly after this
  leaf.

## Notes

The cancel types in the Rust protocol crate belong to leaf 66. Test the
endpoint here through raw HTTP in the agent tests. Do not replace a golden.
