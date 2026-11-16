# macos-timeout-inspection-k4


## Goal

Account for the historical macOS post-timeout inspection stall independently
of ordinary completion, and locate any cleanup/inspection failure.



## Context

- Issue #14 failure A, original VM-fit report and `fixture/r3/guest.sh`, plus
  retained `diagnosis-macos-01/`, which did not reproduce that earlier stall.
- The report from `macos-completion-k3` and the safe process-fixture cleanup
  from `macos-process-fixtures-k185`.
- `ProcessRunner.runShell`, `collectDescendants`, `directChildren`,
  `waitForExit`, macOS `handleExec` and Rust HTTP/deadline fixtures.

## Done when

- Preserve original failed inspections separately from later clean samples and
  the ordinary-command timeout. State which exact old builds remain available.
- Freeze one fresh unchanged clone's exact issue #14 `sleep 120` and fork/wait
  controls with guest timeout 2 s. Follow immediately with the exact known-child
  `ps` observation (guest budget 30 s, host bound 50 s), then ten declared
  after-timeout observation/health operations. Run the complete original macOS
  fixture as a separate condition, retaining its inputs rather than replacing it.
- Record shell/child identities and exit/wait, descendant enumeration, TERM,
  kill/reap, response and subsequent-inspection phases. Distinguish intentional
  timeout from overhead; compare grace <=2 s and kill/drain/reap <=2 s, recording
  violations rather than enlarging ceilings. Identify whether the late operation
  waits on its own child, an earlier request, enumeration or host transport.
- Where necessary use one instrumented clone and deterministic process/HTTP
  controls to force a suspected ordering or blocked enumerator. Bound the
  observer and its teardown. Failed or timed-out inspection never proves absence.
- Deliver a verified Taskfile diagnostic and
  `docs/experiments/exec-reliability-macos-timeout-inspection.md` with each
  result, old-path failure or explicit non-reproduction, regression candidates,
  and the established or unproved relationship to ordinary completion.
- Account for owned children/groups and dispose owned VMs independently even
  when observation, guest cleanup or transfer fails; retain residual identities
  and remedies under the common root contract.

## Notes

Select no production ownership/cancellation mechanism here. No broad process
name cleanup or inherited passing result from the ordinary-completion inquiry
is acceptable. Decompose if the complete fixture or controlled reproducer needs
another session; keep each old failure distinct in the design handoff.
