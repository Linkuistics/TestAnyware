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

## Decisions (running log)

- The original failed inspections stay in the retained VM-fit workspace
  (Ravel-Strategies.gh-issue-1, `experiments/tutorial-vm-fit/evidence/macos-0{1,2,3}`).
  This leaf reads them and does not copy their receipts. The report cites each one by
  path and keeps it apart from later clean samples and from the k3 ordinary timeout.
- The 2.7.0 CLI and app of the original runs are not on this host. Homebrew holds only
  2.8.0. The guest agent 1282f4cf… is the agent inside the retained golden, so the
  guest side of the old path stays available. The host side does not.
- One live plan holds both conditions on one clone: the issue #14 payloads first,
  then the complete r3 sequence. The r3 condition then runs after the issue payloads,
  which differs from the original order. The report states that difference. A second
  unchanged clone would exceed the one-unchanged-clone limit of the root brief.
- The r3 fixture runs from a byte-exact copy under
  `evidence/macos-timeout-inspection-k4/fixture-r3/`. The plan refuses to run when a
  hash differs from the retained `frozen.json`.
- The resource cleanup acts only on the known children: a PID from a pid file that
  the plan wrote, and only while that PID runs exactly `sleep 120`. No name-based
  cleanup.
- The checker reports a stall as reproduced from the stalled call alone. It reports
  not-reproduced only from complete evidence. Every other case is unknown.
- The checker runs without `python -I`, because it imports the k186 checks and the
  frozen shape from the plan module.
- Attempt `k4-timeout-macos-01` stopped before acquisition. The controller refused
  the call names `r3-upload-guest.sh` and the other dotted names, because a call name
  must match `[a-zA-Z0-9_-]+`. No clone started: the VM inventory is equal before and
  after, and the tart inventory differs only in key order. The attempt stays in
  evidence and does not count as the unchanged clone. The plan now replaces each dot
  with a hyphen, and a control runs the controller's own validation on the plan.
