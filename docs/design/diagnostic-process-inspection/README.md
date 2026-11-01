# Diagnostic process inspection views

These editable views accompany the
[inspection specification](../../specs/diagnostic-process-inspection.md).
They show a selected design awaiting review and implementation, not a measured
repair. Graph entry nodes are dark with white bold text and start with
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

- [Ownership → Keep signals with the controller](index.html#diagram-ownership) — Defines the read-only inspection worker and the two separately pinned process groups.
- [Deadlines → Expire evidence without delaying safety](index.html#diagram-deadlines) — Shows KILL proceeding while a snapshot is late, followed by epoch closure and bounded reap.
- [Helper creation → Register before exec](index.html#diagram-helper-start) — Shows the acknowledgement gate and the residual outcome when creation cannot be confirmed.

The viewer is a temporary discussion service. Stop its exact server process
after inspection; restart it with the command above for a later review.
