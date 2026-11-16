# macos-completion-review-k184

**Reviews:** macos-completion-k3

<!-- tc: off because the creating session wrote this charter and a review does not rewrite it -->

## Goal
Adversarially review the first k3 slice: the retained-evidence manifest, the
old-path GCD-starvation detector and its detector control, before later
leaves build on them.

## Context
- Read the producer commit for `macos-completion-k3`, this grove's root brief
  and `docs/experiments/exec-reliability-macos-completion.md`.
- Tooling is under `experiments/exec-reliability/evidence/macos-completion-k3/`:
  `retained.py`, `old_path.py` and `probe/main.swift`. The Taskfile tasks are
  `diagnostic:macos-completion-retained`, `diagnostic:macos-completion-old-path`
  and `diagnostic:macos-completion-tools`.
- Raw runs are `old-path-01` and `single-hop-01` in that directory. The
  retained originals are in the Ravel-Strategies checkout named by
  `retained-manifest.json`.

## Done when
- Check whether the probe could report `completed-then-timed-out` for a
  reason other than the nested-dispatch ordering, such as calibration that
  does not saturate, blockers that leak into the run, or a 3 s timeout that
  the host misses.
- Check whether the single-hop control truly removes only the second block,
  and whether its `process.isRunning` polling depends on the global queue.
- Check the classification and verdict rules against every raw receipt, and
  check that `EXPECT` semantics keep the known-defect probe outside a passing
  gate.
- Check every identity, timing and provenance claim in the report against
  the retained receipts and the manifest. Decide whether the report keeps the
  cause unestablished wherever the evidence is not causal.
- Check the owned-resource audit, and check that the probe always releases
  its blockers, also on calibration failure.
- Report each finding with its evidence. Do not edit the producer's files.

## Notes
