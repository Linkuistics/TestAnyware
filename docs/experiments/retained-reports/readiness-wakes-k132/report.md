> Historical report. Paths and commands refer to the [archived checkout](../../README.md).

# Readiness wake dispatch (k132)

The common driver can wake a bounded owner when its command pipe becomes writable or its result/control pipe becomes readable. It waits until readiness or the earlier timer, then rechecks due safety work before dispatching transport. Request callbacks switch from command-writer interests to stream-reader interests; failure retirement registers only its open cleanup reader. Owners retain their existing nonblocking operations, caps, immutable deadlines and signal/reap authority.

This is an independently useful transport seam. Production controller owners still run sequentially. Actual concurrent scheduling and worker-retirement/resource-teardown overlap, every remaining control, retained failures and the final independent k28 review remain in `complete-worker-scheduler-k134`. The separate readiness review is `complete-worker-scheduler-k135`, before that continuation. k28 and k17 remain open; ten-clone guest acceptance is **0/10, unavailable**. No guest or installed-app changes were made.

## Source and measurement boundaries

The baseline is commit `63b9b43270c5b30a9f3cbf519bf21421eee47aff` (`mvzwzxulkoxxtwtuuporolvwxrznrytl`). `parent-source/` contains its exact `turns.py`, `worker_capture.py` and `test_turns.py`; `inputs-parent/` combines them with the shared unchanged runtime, tests and current task recipes. The final candidate changes those three Python files. `inputs-candidate/` retains the first implementation; `inputs-candidate-final/` adds only the explicit pending-list type annotation. Both candidate full pairs use all thirteen frozen Python modules/tests, the manifest and lockfile, and the frozen Taskfile. No runtime source changed after final input preparation.

`comparison-plan.json` and `observed-deviations.json` record the declared comparisons and corrections. `instruments-at-measurement/`, `instruments-final/` and `instruments-current/` preserve successive instrument bytes. Each condition retains commands, raw stdout/stderr, JUnit case identities, duration, checker exit/reap receipts, exact fixture audits, artifact hashes and before/after subject hashes. Subjects include every frozen Python file, manifest, lockfile, both task recipes, all four evidence tools, plan/deviations, the inspection specification, interpreter and command binaries. Original and final full pairs ran in parent/candidate order with exclusive outputs; each sequence completed before its instruments changed.

Checks use the repository's `diagnostic:readiness-wakes-{prepare,check,preserve,tools,audit}` tasks through `mise exec -- task`. Full checker timeout is 120 seconds; focused controls 60 seconds and each static checker 30 seconds, followed by bounded TERM/KILL/reap cleanup. These are checker containment limits; they do not extend runtime owner deadlines. Signal/fixture auditing uses recorded exact identities without widening the target set.

## Controls and retained results

Nine added driver cases fail against the parent driver and pass against the candidate. The RED selector also includes ten existing passing readiness cases: its actual result is 10 passes and 9 failures out of 19, rather than nine selected cases. Six parameterized cases cover read/write wake arrival before, at and after a safety deadline with an injected clock. Other controls cover owner routing and terminal removal, empty/spurious readiness with timer expiry, and a real pipe. Existing real request/retirement and command-dispatch controls now require the production registrations. Both candidate focused runs pass 106 cases.

Each of six mutations rejects against both the original and final candidate: ignoring ready descriptors (1 case), dropping safety priority (6), waiting past the earliest deadline (6), omitting query registration (4), omitting retirement registration (2), and registering readers while a command write is pending (6). Each mutation changes exactly its declared source site; unrelated frozen Python inputs match its candidate. The exact-clock safety arrivals are simulated scheduling conditions; real pipes and helper/worker requests exercise actual transport. These controls do not establish a production latency distribution or concurrent production ownership.

| Condition | Runtime passes / cases | Pytest elapsed | Failed static checker |
| --- | ---: | ---: | --- |
| parent-full-1 | 887 / 887 | 80.66 s | pass |
| candidate-full-1 | 896 / 896 | 86.85 s | pyright |
| parent-full-2 | 884 / 887 | 104.07 s | pass |
| candidate-full-2 | 896 / 896 | 81.21 s | pyright |
| parent-final-1 | 887 / 887 | 88.06 s | pyright |
| candidate-final-1 | 896 / 896 | 80.37 s | pyright |
| parent-final-2 | 886 / 887 | 77.62 s | pyright |
| candidate-final-2 | 896 / 896 | 73.88 s | pyright |

Both original candidate full suites pass 896 tests but strict Pyright reports five inference errors at `turns.py:62`. An explicit `list[tuple[float, int, Turn]]` annotation repairs the inference without changing dispatch logic; `static-final` verifies that frozen repair. All four final full conditions expose a second, instrument-only issue: Pyright cannot resolve the auditor's local `prepare` import from a frozen working directory (eight errors), although the direct tool check passes. The auditor now carries explicit mutation specifications without that import. `static-parent-current` and `static-candidate-current` both pass Ruff, format and strict Pyright, as does the direct four-tool check. No runtime rerun was needed for the evidence-only import repair. Every original failed check remains retained.

The original `parent-full-2` has three unresolved timing failures: `test_review_exited_leader_does_not_hide_a_surviving_group_member[False]` sees return code -9 instead of 0; `[True]` observes leader exit at 349937.987481333 after KILL at 349937.980860666; `test_review_live_group_is_failure_even_with_expected_leader_exit[True]` also sees -9 instead of 0. `parent-final-2` reproduces `test_persistent_worker_expired_request_is_sticky` at `test_worker_capture.py:2751`, where `residual_remedy` is None. That matches the retained k129 failure family. Later clean candidate samples do not explain or repair these parent observations.

`condition-audit.json` compares every shared and added JUnit item across all four pairs, checks the nine additions and the focused selections, verifies hashes and mutation sites, and distinguishes integrity from outcome. Its overall and final comparison gates remain rejected because retained failures are unresolved. Passing current statics and both final candidate runtime suites are separate facts, not aggregate acceptance. The audit deliberately exits nonzero on those retained rejections.

## Evidence integrity and remaining acceptance

All 94 recorded exact-fixture audits report absence, all checker children are reaped, and no checker times out. This is the set of fixtures recorded by the runner; it is not an exhaustive claim about unrecorded descendants. Before/after subjects and retained artifact hashes are checked by the auditor. Prior-evidence preservation compares every recorded path, file byte hash or symlink target, excluding this new corpus and generated caches; all 232,416 prior paths are unchanged, as recorded in `prior-after.json`.

Historical unresolved evidence remains binding: k85/k106 unknown ps tables or missing PIDs; k110 unsupported real ps state `?E`; k114 lost-result errno 35/exit 70, incomplete frame and missing EOF; k116/k129 deferred-reap postcutoff probe; k129 expired-request `residual_remedy` None; and the standalone helper timeout/postcutoff retirement. The new parent failures above join that set. The intact aggregate charter and detailed original handoffs remain in k134, and earlier corpora are retained byte for byte.

Privileged real ps KILL and UID EPERM fixtures remain unavailable; no privilege was acquired. Storage-stall work remains separate in k18. The aggregate must still establish one safety-first scheduler across actual production owners, required teardown overlap, every outstanding real/fault/mutation control, comparisons against that completed scheduler, and independent `review-impl` of `bounded-inspection-k28` before k21.
