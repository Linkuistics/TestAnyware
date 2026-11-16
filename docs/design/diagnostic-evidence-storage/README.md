# Diagnostic evidence storage views

These editable views accompany the
[evidence storage specification](../../specs/diagnostic-evidence-storage.md).
They show the ownership and timing contract. Delivery evidence belongs in the
[foundation report](../../experiments/exec-reliability-diagnostic-foundation.md#evidence-storage-design-k18).

Graph entry nodes are dark with white bold text, and their labels start with
"Start here". Numbered edges show order. The sequence diagram reads from its
first message. Boxes show local processes and responsibilities, not deployment
containers. The diagrams are contracts, not evidence from a formal model.

From the repository root, run `mise exec -- task design:storage-preview`. Then
open `http://127.0.0.1:8766/#discussion`. To use another port, set `PORT`. The
task serves only this directory.

The viewer loads Mermaid 12.0.0 from jsDelivr,
so it needs an internet connection. The `.mmd` files are the editable sources.
The viewer is a copy of the inspection document's viewer.

- [Ownership → Give storage to the writer](index.html#diagram-ownership) — Shows the writer, the outbox and the descriptors that each process holds.
- [Phases → Bound each storage phase](index.html#diagram-phases) — Shows readiness, the intent gates and the publication cutoff.
- [Failures → Stop forward, keep teardown](index.html#diagram-failures) — Shows the result of a storage failure in each phase.

The viewer is a temporary discussion service. After review, stop its exact
server process.
