# complete-worker-scheduler-k95

**Reviews:** complete-worker-scheduler-k91

## Goal

Independently review the active workload exit-wait deadline repair shipped by k91, using its committed code, final controls and retained evidence. This is the slice review; the full bounded-inspection-k28 runtime review remains required by complete-worker-scheduler-k94.

## Context

Read the producer commit, ancestor brief chain, docs/specs/diagnostic-process-inspection.md and experiments/exec-reliability/evidence/workload-wait-k91/report.md. Runtime change is `controller._wait`; integration is through existing `_call`/`_terminate`/`_reap_leader`. The complete scheduler charter is carried verbatim into flat sibling complete-worker-scheduler-k94. Persistent worker/bootstrap, preemption and shared scheduler are explicitly incomplete; no aggregate ancestor was closed.

## Review focus

- Trace actual callers and verify strict admission before each observation, including sleep overrun, and strict result validation before accepting true. Check equality, false late result, timely polls, OSError propagation, scheduling between samples, and the separate inclusive first-reap rule. Look for renewed authority or misleading success in caller integration.
- Verify normal timeout still attempts mandatory group KILL with exited leader, PID-only interruption suppresses KILL after exit without widening authority, and denied TERM remains sticky. The real local Darwin zombie EPERM is not the unavailable privileged UID fixture. Check remaining synchronous observation latency is disclosed rather than claimed bounded/preemptible.
- Audit all twelve final cases and the real-process fault seams. Independent `/bin/ps` observations must cover every recorded workload leader, helper and worker outside the faulted observer. Confirm cleanup/disposal/absence and unreached work, exact reap and final query evidence. No injected exit or errno establishes privileged behavior.
- Recompute source/tool/runner and artifact hashes, exact per-case JUnit identities and expected outcomes. Final green-2 is 12 pass, full-2 is 481 pass plus Ruff/format/strict Pyright. Final root sources equal inputs-final-2. Red-2 has exactly 8 failures; admission/result/sleep mutants exactly 4/4/2. Checker timeout/reap receipts and residuals matter. Verify new runner type checks, including explicit strict configuration.
- Preserve and classify the original failed test-draft green sample (10 pass/2 fail) and relocated full-suite sample (480 pass/1 layout failure); do not certify either. Verify baseline 3ab5a2cebccfe83f5367628e319d6be1c483d80b's 94,996 tracked evidence paths are unchanged and all retained new frozen inputs/large-output artifacts are actually committed. `verification.json` records checks but is producer evidence, not independent review.
- Keep k28/k17/root open and full aggregate obligations in k94. Any findings earn a concrete integrate-review-impl sibling before k94; create no empty integration step. Follow review routing/provider requirements without inventing missing evidence.

## Artifact entry points

experiments/exec-reliability/controller.py; experiments/exec-reliability/test_controller.py (`workload_wait` cases); experiments/exec-reliability/evidence/workload-wait-k91/{report.md,run_condition.py,verification.json}; Taskfile.yml (`diagnostic:workload-wait-check`, `diagnostic:workload-wait-tools`). All conditions are immutable and reruns require a fresh OUT. Repository-layout full runs use actual experiments/exec-reliability as INPUTS; focused controls use frozen input directories.
