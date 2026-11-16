# evidence-writer-k162 — brief

## Goal

Move every evidence storage operation out of the diagnostic controller and into
a separate writer process. Storage stalls and storage failures must not stop
cleanup, disposal or inspection. The controller and its turns must not block on
the file system from writer readiness to the end of publication.

## Context

The design is `docs/specs/diagnostic-evidence-storage.md`. The leaf
`diagnostic-disposal-k18` wrote it, `diagnostic-disposal-k159` reviewed it and
`diagnostic-disposal-k160` repaired it. The visual companion is in
`docs/design/diagnostic-evidence-storage/`. Run
`mise exec -- task design:storage-preview` to view it.

The code is in `experiments/exec-reliability/`. Today `execute()` in
`controller.py` writes the plan, the subject digests, the intent records, the
call streams, the result and the identity file directly. The fork admission
rule allows one Python thread and one native thread. A storage thread is
therefore impossible, and the writer must be a process.

## Done when

- The thirteen controls in "Test seams and required controls" pass. These are
  the nine required controls and the four review controls.
- No control keeps an expected-failure marker.
- `execute()` still returns the run directory as a `Path`.
- The callers in `peer.py`, `preflight.py` and the existing tests run without
  change.
- `retention.py` accepts a run only by the rules in "Completion authority".
- The foundation report section "Evidence storage design (k18)" in
  `docs/experiments/exec-reliability-diagnostic-foundation.md` records the
  delivered behaviour and its limits.

## Decomposition

The slices follow the order of ownership in the spec. Each slice leaves the
suite green.

1. `storage-controls-k163` writes the nine required controls first. Each
   control fails on the current build. The production code does not change.
2. `storage-writer-k164` builds the writer process, the frame protocol, the
   close transaction and the completion authority in `retention.py`. The
   controller does not use the writer yet.
3. `storage-client-k165` builds the controller side of the channel. This is the
   writer spawn, readiness, the nonblocking outbox and the acknowledgement
   reader.
4. `storage-cutover-k166` sends every run record from `execute()` through the
   writer. It adds the intent gate, forward stop and the storage section of the
   result snapshot.
5. `pipe-capture-k167` replaces the call stream files with pipes, the running
   digests and the bounded final drain.
6. `storage-publication-k168` adds the publication phase, the writer stop, the
   terminal outcome and the audit hook control.

## Notes

The node earns its level for three reasons. Its subgoal is distinct from the
guest routes that follow it. Its slices share one completion test, which is
the thirteen controls. Its slices share the spec as new context that later
siblings do not need.

A control can pass earlier or later than its slice expects. In that case,
remove its marker in the slice where it first passes, and record the reason.

Each slice is load-bearing. As its last work act, each producer cuts its own
`review-impl` leaf. An integration leaf follows only when the review warrants
it.

None of these slices needs a live virtual machine.

## On the horizon

- Crash recovery and `fsync` are out of scope by the spec.
- The spec does not say how `retention.py` treats run directories from before
  the writer. `storage-writer-k164` decides this and records the decision.
