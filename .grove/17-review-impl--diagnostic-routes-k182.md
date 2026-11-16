# diagnostic-routes-k182

**Reviews:** diagnostic-routes-k19

<!-- tc: off because the creating session wrote this charter and a review does not rewrite it -->

## Goal
Adversarially review the public route probe, the inventory checker and the
golden-name guard before the first causal leaf (k3) relies on them.

## Context
- Read the producer commit for `diagnostic-routes-k19`, this grove's root
  brief, and the section "Baseline public routes (k19)" of
  `docs/experiments/exec-reliability-diagnostic-foundation.md`.
- The checker is `scripts/vm_inventory.py`. Its controls are
  `experiments/exec-reliability/test_vm_inventory.py`. The probe is
  `experiments/exec-reliability/routes_k19.py` with
  `experiments/exec-reliability/routes_k19_run.sh`.
- The guard is the Taskfile task `vm:golden-name-free` and its call in both
  `vm:create-golden-*` recipes.
- Raw evidence is under `experiments/exec-reliability/evidence/k19-*`.

## Done when
- Check whether the checker can report absence while either inventory lists
  the name, or while an inventory read failed. Check whether disposal can
  accept any result other than a proved stop or a proved `VM_NOT_FOUND`.
- Check whether the fault controls (`k19-red-01`, `k19-red-02`) would reject
  a broken checker or guard that the 52 controls do not name.
- Check the frozen probe plans against the raw evidence: plan order, bounds,
  one owned clone per OS, retained failed samples, and the one revision
  between the Linux and macOS runs. Decide whether that revision is
  admissible under the root brief.
- Check each identity claim (app signature, guest agent hashes, guest OS)
  against the raw streams, and check the golden preservation claim against
  the before and after listings.
- Check that the guard runs before any create in both recipes, and that its
  executable defaults and variable precedence cannot bypass it.
- Report each finding with its evidence. Do not edit the producer's files.

## Notes
- Do not run `agent:macos:test`, start a clone, or create a golden. The
  review needs no live VM.
