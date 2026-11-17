# Guest command lifecycle views

These editable views accompany the
[lifecycle specification](../../specs/guest-command-lifecycle.md). They show the
target contract for issue #14, not delivery evidence. The decisions are in
ADR-0023 to ADR-0026.

Graph entry nodes are dark with white bold text and start with "Start here".
Sequence diagrams read from their first numbered message. Boxes are processes
and responsibilities, not deployment containers.

To preview the views, run `mise exec -- task design:lifecycle-preview` from the
repository root. Then open `http://127.0.0.1:8768/#discussion`. To use another
port, set `PORT=8769`. The viewer loads pinned Mermaid 12.0.0 from jsDelivr, so
it needs an internet connection. The `.mmd` files are the editable sources, and
no rendered exports are committed. The viewer is a copy of the one in
`../diagnostic-process-inspection`.

- [Lifecycle → Choose one outcome, then make it final](index.html#diagram-state-lifecycle): adds the states, the arbitration rule and the retention of one guest command.
- [Lifecycle → Signal the group while the leader is held](index.html#diagram-seq-terminate): adds the termination steps and the two 2 s bounds.
- [Cancellation and interruption → Cancel on a new connection, then re-raise](index.html#diagram-seq-cancel-interrupt): adds explicit cancellation, CLI interruption and the fallback after SIGKILL.
- [Evidence → Read the record before any claim](index.html#diagram-flow-read-record): adds how the Host CLI classifies an `/exec` reply.
- [Evidence → Send exactly the length of the open handle](index.html#diagram-seq-download): adds the single-handle download and the Host CLI checks.
- [Seams → Place each guarantee at its seam](index.html#diagram-component-seams): adds the implementation candidates and the test seams.

The preview server is a temporary service. Stop its process after the review.
