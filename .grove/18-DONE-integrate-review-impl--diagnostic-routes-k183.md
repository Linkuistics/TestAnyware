# diagnostic-routes-k183

**Integrates:** diagnostic-routes-k182


## Goal
Triage the committed review of diagnostic-routes-k19 and reconcile the material
findings before macos-completion-k3 consumes the diagnostic handoff.



## Context
- Read the findings in the commit for diagnostic-routes-k182, not a restated
  finding list in this task. The producer is diagnostic-routes-k19.
- The root brief's evidence, ownership and preserved-golden constraints apply.

## Done when
- Verify each review finding against the source and classify it using the
  integration acceptance/material-risk threshold. Record accepted trade-offs
  and rejected findings with reasons.
- Repair the real issues within the producer's scope and retain appropriate
  bounded verification evidence, including controls that detect the repaired
  failure modes. Reconcile the diagnostic report with the delivered behavior.
- Leave the handoff usable by k3, with remaining uncertainty stated explicitly.

## Notes
This is a flat sibling integration of the existing implementation, placed
before the next live consumer because it consumes findings anchored to the
review commit. Read-only review evidence does not authorize golden creation,
starting replacement route clones, changing installed software, or running
the unsafe macOS process suite.

## Decisions (running log)

Integrated review `diagnostic-routes-k182` (commit `4fd0600f23`) of producer
`diagnostic-routes-k19` (commit `e41868954f`). A source read showed both
findings. Both are real issues inside the producer scope, and k183 repaired
both. k183 created no golden, cloned no VM and changed no installed software.

- **F1 (real issue, repaired).** `Taskfile.yml` passed `NAME` through
  `shellQuote` to the guard but unquoted to both `vm create-golden` commands.
  A name such as `"testanyware-golden-linux-24.04"`, with its quote
  characters, passed the guard and named the retained golden in the create.
  Both create commands and the three `test -n` preconditions now use
  `{{.NAME | shellQuote}}`. Twelve controls render each recipe with
  `task --dry` for six names that a shell rewrites. Each control runs the
  rendered guard and create lines under `/bin/sh` with recording fakes. The
  guard and the create must receive the same literal name.
- **F2 (real issue, repaired).** `scripts/vm_inventory.py` counted any exit-0
  success envelope as `stopped`. `_stop_outcome` now requires `id` equal to
  the requested identity, `stopped is True` and no `dry_run`. Eight refused
  replies cover the gap, and the `STOPPED` fixture now matches the reply of
  `run_vm_stop` in this checkout and in tag `v2.8.0`.
- **Fault controls.** Seven faults made the controls fail, and the runner
  restored each subject to its earlier hash (`k183-red-02`). The final
  checks pass: lint, format, types, shellcheck and 72 controls
  (`k183-checks-03`). `k183-red-01`, `k183-checks-01` and `k183-checks-02`
  keep the earlier failed attempts.
- **Other review conclusions (no repair).** The identity claims, the outer
  listings and the 0/10 verdict are accurate. `inventory.json` holds subject
  paths, not hashes. The review gave that no repair obligation, and k183
  agrees.
- **Uncertainty for k3.** The k19 route runs used the checker before k183.
  They kept only the verdict `disposed ID stopped`, not the raw envelope. The
  claim that each stop named its own id with `stopped: true` comes from the
  v2.8.0 source, not from an observation. The checker observed absence from
  both inventories after each stop. The F1 controls run the rendered lines
  under `/bin/sh`, not under the Task shell interpreter. The report section
  "Review k182 integration (k183)" records these limits.
