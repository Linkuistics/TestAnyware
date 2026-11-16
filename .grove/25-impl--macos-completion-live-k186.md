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

## Decisions (running log)
- Declared workload, frozen before the live run in
  `experiments/exec-reliability/completion_k186.py`: one owned clone
  `k186-macos-<run>` of `testanyware-golden-macos-27` (the golden that k19
  booted, agent sha256 1282f4cf…8a84). Calls in order: health, ten serial
  exact `file exec --timeout 30 --json "printf ordinary-ok"` (host bound
  50 s each), identity, three rounds of four concurrent exact requests
  (`concurrent_k186.py`, each request bounded at 50 s, then TERM 2 s and
  KILL 2 s), a guest pool read (`sysctl` + `ps -M` of the agent), health.
  One run, no retry. A failed sample stays.
- The concurrent control is four requests, not more: the k3 mechanism needs
  two global workers per request, so four requests ask for eight. That
  stays far below a default limit of 64 or more but can expose a guest
  whose limit is low. A larger count would test load, not the k3 candidate.
- Phase seams: the installed CLI and app expose no per-phase log. The
  receipt gives child exit (`exit_code`), the guest timer result
  (`timed_out`) and capture content (`stdout`, `stderr`). The host
  controller gives client deadline and elapsed time. Wait completion,
  handler response and app relay stay unknown unless a receipt shape
  separates them, as `completed-then-timed-out` does.
- The instrumented clone (done-when 3) is conditional. Run it only when the
  unchanged clone shows a failure or the pool read shows a constrained
  limit near the per-request demand. Otherwise record it as not needed,
  with the pool read as the evidence.
- Guest cleanup action: `/bin/echo` note only. Each request either exits
  or the agent kills its tree, and disposal removes the guest.
