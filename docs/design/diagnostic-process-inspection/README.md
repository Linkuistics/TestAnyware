# Diagnostic process inspection views

These editable views accompany the
[inspection specification](../../specs/diagnostic-process-inspection.md).
They show the ownership and timing contract. Delivery evidence belongs in the
[foundation report](../../experiments/exec-reliability-diagnostic-foundation.md#bounded-inspection-design).
Graph entry nodes are dark with white bold text and start with
“Start here”. Numbered edges indicate order; sequence diagrams read from their
first message. Boxes denote local processes and responsibilities, not deployment
containers. Timing diagrams are contracts, not model-checking evidence.

From the repository root, run `mise exec -- task design:inspection-preview`
and open `http://127.0.0.1:8764/#discussion`. Override the port with `PORT=8765`
if needed. The task serves only this presentation directory.
The static viewer loads pinned Mermaid 12.0.0 from jsDelivr; an internet
connection is required. The `.mmd` files are the editable sources and no rendered
exports are committed. The viewer comes from Grove's
`grove-design/assets/diagram-viewer.html`, with Mermaid SVGs constrained to the
available width for this document.

- [Ownership → Keep signals with the controller](index.html#diagram-ownership) — Defines one scheduler, the worker and the two separately pinned process groups.
- [Deadlines → Expire evidence without delaying safety](index.html#diagram-deadlines) — Shows group KILL at grace expiry, a stalled final query, epoch closure and bounded reap.
- [Helper creation → Publish before exec](index.html#diagram-helper-start) — Shows publication without a per-helper ACK, fork failure and unknown creation.

The viewer is a temporary discussion service. Stop its exact server process
after inspection; restart it with the command above for a later review.
