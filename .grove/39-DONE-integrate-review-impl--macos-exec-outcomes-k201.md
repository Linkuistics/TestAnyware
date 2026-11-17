# macos-exec-outcomes-k201

## Goal

Triage the findings from `macos-exec-outcomes-k200` against agreed acceptance
and material risk, and integrate the actionable outcomes before later lifecycle
evidence relies on the k12 matrix.

## Context

- Read the findings from the review's own commit, using handle
  `macos-exec-outcomes-k200`.
- The reviewed producer is `macos-exec-outcomes-k12`. Keep its recorded run
  intact and follow the root brief's measurement and isolation boundaries.

## Done when

- Each finding has a source-supported classification and recorded disposition.
- Any accepted repairs and their necessary verification are complete; the
  resulting evidence and report accurately state what later work can rely on.

## Notes

Each finding comes from review commit `aefcc335e6`. The source supports each
one. The report section "Review integration (k201)" holds the evidence.

- P1, cleanup ownership: a real issue against the root brief. The brief
  requires positively identified, run-owned cleanup. `sweep()` and `audit()`
  killed any same-user process that carried a constant token. Disposition:
  repaired. Each token now ends with a 12-digit run tag, and each kill checks
  the start time and command again.
- P2, checker gate: a real issue. A reference deadline record without its phase
  marks passed, and so did a wrong signal. Disposition: repaired. The new class
  `phase-marks-invalid` rejects absent or unordered marks. The deadline cases
  now expect signal 15 or signal 9. A new control also exposed a `KeyError`
  crash on a record without `reap_ns`, and this leaf repaired it.
- P2, CLI ceiling: a real issue against the root brief. The CLI bound added
  10 s to the agreed guest budget plus 20 s. Disposition: repaired. The bound
  is now the agreed ceiling. The checker measures each elapsed time against
  that ceiling and does not rely on the flag.

Run-01 of k12 stays byte-identical. The stricter checker accepts a copy of it.
