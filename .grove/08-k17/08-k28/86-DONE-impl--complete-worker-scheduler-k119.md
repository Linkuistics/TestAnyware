# complete-worker-scheduler-k119

## Goal

Expose workload-leader reaping as nonblocking turns, used by the existing synchronous controller consumer. This is one bounded prerequisite of complete-worker-scheduler-k122, not aggregate completion.

## Context

Read controller.py `_reap_leader`, leader-reap-k93/workload-wait-k91 first-reap evidence and k118 termination seam. The full original charter and all historical handoffs now live verbatim in complete-worker-scheduler-k122. No requirement is discharged by moving its owner.

## Done when

- A workload reap owner performs at most one exact positive-PID WNOHANG per advance and never sleeps; it returns an absolute next wake clamped to the immutable cutoff.
- The first reap is attempted even at an expired cutoff. Later turns at/after cutoff never poll. Repeated terminal turns are inert. Successful reap updates Popen status and releases the retained exact owner; failure retains ownership and explicit evidence.
- `_reap_leader` consumes the same state machine and preserves existing receipt semantics and bounded sleeping outside turns.
- Retain exact-clock boundary/late-return controls, real-child controls and complete diagnostic suite/static results, including all failures. Freeze/hash subjects and audit exact fixture cleanup; preserve prior evidence.
- Leave k122 and all aggregate ancestors open. Assess a separate review of this ownership seam once the artifact exists.

## Decisions (running log)

- 2026-11-15: Flat sibling decomposition keeps this leaf as a useful nonblocking reap slice and moves the ENTIRE aggregate charter/history to complete-worker-scheduler-k122. The existing parent supplies the shared context; another node would duplicate it. This slice removes a private waiting seam but does not implement shared scheduling or teardown overlap.
- 2026-11-15: Graph searches are still pending and coverage generation 2026-11-15T02:19:52Z reports metadata_changed for controller.py, test_controller.py, worker_capture.py and Taskfile.yml. Use exact source reads as authority for this boundary rather than stale graph claims.
- 2026-11-15: RED froze the old controller with four new real-child controls. All four fail on the missing incremental owner, with independent child absence retained. The owner now closes authority at construction, performs one exact WNOHANG per advance, and returns a cutoff-clamped absolute wake. The synchronous wrapper owns sleeping. This preserves inclusive first-reap semantics and sticky residual ownership.
- 2026-11-15: Focused 22/22 pass. Removing the later-poll admission guard fails exactly the withheld cutoff case. Injecting sleep before a returned wake fails ready/missing/withheld (3/4); the late-return case terminates before that site. Corrected the auditor's anticipated 4 to observed 3, retaining both measured auditor revisions and explicit condition-specific subject bindings. The full collected suite has 860 cases (not anticipated 859). No runtime/control edits followed GREEN.
- 2026-11-15: Full 860/860 plus Ruff/format/strict Pyright pass. Condition audit validates 20 new-control child absences, exact expected mutation failures, subject/artifact hashes and frozen candidate equality. Prior before/after inventories match all 191643 paths. Report records the initial tooling error/corrections rather than replacing their samples. Historical scheduler failures remain binding in k122. This refactor is version-invariant plumbing using the existing cited waitpid contract; no new framework API decision or ADR policy change.
- 2026-11-15: The independently retained ownership state machine is a load-bearing scheduler boundary. Schedule a separate Grove review before k122; no in-session reviewer, because that fresh-context leaf supplies the adversarial read. Scope includes first-attempt exception, immutable cutoff, terminal/residual ownership and whether the finite controls detect regressions. Ancestors stay open with k122 live.
