# complete-worker-scheduler-k122

## Goal

Expose incremental active-workload exit-wait turns in the real controller, preserving the reviewed k91 deadline behavior and synchronous caller compatibility.

## Context

The full original aggregate charter and historical log are preserved in complete-worker-scheduler-k124. This flat split shares bounded-inspection-k28 context; no new context node is needed. Read experiments/exec-reliability/evidence/workload-wait-k91/report.md and the k95 review for deadline semantics. The preceding termination and leader-reap turn owners establish the integration shape.

## Done when

- An active workload wait owner performs at most one non-reaping exit observation per advance, yields an absolute wake capped by its fixed deadline, and never sleeps within an advance.
- Entry at/after deadline admits no observation, and observation completion at/after deadline cannot establish success. Terminal advances remain inert; errors propagate without a later retry.
- The production _wait consumer drives those turns, preserving normal completion, late-observation failure, termination and safety teardown behavior.
- Retain frozen RED/GREEN controls, meaningful mutation rejection, real exact-child absence audits, the complete diagnostic suite and static checks, before/after subject hashes and prior evidence preservation.
- Report the precise remaining synchronous boundaries and schedule independent review of this seam as final producer work.

## Notes

complete-worker-scheduler-k124 owns every original aggregate requirement, including the single shared scheduler, helper transport and sticky failure recovery, teardown/shutdown overlap, outstanding real controls, retained intermittent failures, parent/candidate interleaved repeats and final bounded-inspection-k28 review. Ancestors remain open; ten-clone acceptance stays 0/10 unavailable.

## Decisions (running log)

- 2026-11-15: The remaining aggregate is too large for this focused session. Choose one independently green workload-wait turn seam, used by the existing production consumer and directly verified. Copy the entire original task to flat continuation k124 before narrowing this task. No shared-scheduler or overlap acceptance is claimed.

- 2026-11-15 k122 result: `_WorkloadWait` turns and the production compatibility consumer pass 18 focused controls, six rejected mutations and 866 full-suite tests/static checks. First candidate float-sleep failure retained; corrected candidate passes. All 60 new child journals and two external fixture journals show absence; 193,709 prior evidence paths unchanged. See experiments/exec-reliability/evidence/workload-turns-k122/report.md. Shared scheduler, persistent failure cleanup/close and actual teardown overlap remain k124; no aggregate acceptance or historical-failure repair. Independent seam review is inserted before continuation.
