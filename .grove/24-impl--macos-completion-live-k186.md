# macos-completion-live-k186

<!-- tc: off because the creating session wrote this charter for a later session -->

## Goal
Measure ordinary macOS synchronous completion on one fresh unchanged clone.
Test the k3 global-queue starvation candidate in the guest, at most on one
instrumented clone.

## Context
- Split from `macos-completion-k3`. Read
  `docs/experiments/exec-reliability-macos-completion.md` first. It names
  the retained failure, the provenance gap of agent `1282f4cf…8a84`, and the
  old-path detector `diagnostic:macos-completion-old-path`.
- The k3 slice reproduced the retained receipt shape on the host with the
  real `ProcessRunner.runShell` when one GCD worker was free. It did not
  observe starvation in a guest. The retained recovery wait succeeded within
  0.058 s, so any starvation there was transient.
- Reuse the k11 controller, ledger and route table and the k19 inventory
  checker. Run `macos-process-fixtures-k185` first if this leaf runs
  `agent:macos:test`.

## Done when
- Freeze one fresh unchanged macOS clone. Run ten consecutive exact
  `printf ordinary-ok` commands with guest timeout 30 s and an independent
  host bound of 50 s. Record every sample. Do not replace failures.
- Compare serial execution with a small predeclared concurrent-request
  control against the same build. Record child exit, wait completion,
  capture read, handler response, client deadline and app relay where an
  existing seam observes them. Unobserved phases stay unknown.
- If needed, use at most one isolated instrumented clone to observe the
  guest dispatch pool (for example its constrained-thread limit and the
  number of blocked global workers) during the same workload. Retain
  evidence for and against each cause in the k3 table.
- Add the results, the owned-resource audit and any change to the cause
  table to the k3 report. A clean run is not a repair.

## Notes
The post-timeout inspection stall belongs to `macos-timeout-inspection-k4`.
Select no new production wait or cancel semantics here.
