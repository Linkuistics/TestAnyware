# complete-worker-scheduler-k78

## Goal

Reject worker and helper creation when preparation changes fork admission or exhausts the fixed capture deadline. This is an independently useful repair to the interim per-query runtime; complete-worker-scheduler-k81 owns the entire original aggregate charter.

## Context

Read bounded-inspection-k28 and docs/specs/diagnostic-process-inspection.md. The early admission snapshots currently precede channel creation and signal masking. Revalidate Python/native threads and Python/native SIGCHLD immediately before each actual fork, then check the original capture deadline. Admission is a snapshot, never a lock. Preserve early checks, real fork errno publication, ownership and cleanup rules.

## Done when

- Both actual worker/helper fork boundaries recheck admission after all pipe and mask preparation and refuse child creation if it fails or the original capture deadline has expired. Refusal must restore the previous mask, close prepared descriptors, retain the final admission/error, create no owner, and never publish a kernel fork-error receipt for an admission/deadline refusal.
- Controls fail against the original code and pass against the repair: real late Python thread / SIGCHLD state changes, native observation refusals, and preparation consuming the deadline. Assert no fork, no helper execution, no leaked descriptors or renewed deadline. Preserve success and actual fork-error behavior.
- Retain frozen inputs, RED/GREEN receipts, checks and report without changing prior evidence. Run the full diagnostic self-check and independently reconcile any new process identities. No guests, installed app mutations, privileges or unrelated signals; ten-clone acceptance remains 0/10 unavailable.
- Cut a review-impl sibling for this fork boundary artifact with **Reviews:** complete-worker-scheduler-k78. The full runtime and eventual k28 review remain in k81; do not retire aggregate ancestors.

## Decisions (running log)

- 2026-11-15: Flat siblings fit because this is a complete fork-boundary refusal repair within the existing k28 purpose. k81 preserves the full original charter byte-for-byte apart from its handle and this handoff. k78 claims neither a persistent worker nor shared scheduling. Late admission refusal must occur outside the actual os.fork OSError publication handler so it cannot manufacture kernel fork-error authority.

- 2026-11-15: Both fork sites now repeat admission after mask preparation and then check the original capture cutoff, before os.fork. The helper refusal sits outside its kernel errno handler. RED original controls attempted all 14 forbidden forks through a no-child EAGAIN tripwire; repaired controls all passed. Real late Python/native threads, SIG_IGN/native SA_NOCLDWAIT, native observation EIO adapter and real elapsed deadline exhaustion are covered. Full-01 retained one obsolete preparation-delay expectation (414 pass, one failure); move that control delay inside the actual fork adapter so late kernel-return cleanup remains exercised, retaining the failed evidence. Full aggregate remains k81.

- 2026-11-15: Final full-02 is green (415 pytest cases, lint/format/types; all four checkers reaped without timeout). Retained red-02 again fails all 14 attempted-fork controls on original production with the final fixture suite. The final independent audit reconciles all five conditions and exact reasons/29 repaired observations, 2,818 stable subjects, 85,216 unchanged prior evidence files, 1,463 exact fixture PIDs independently absent and 155 inspected scratch directories; no markers needed disposal. audit-01 remains failed for its Linux errno=11 assumption; audit-final uses platform errno.EAGAIN (Darwin35). Report: experiments/exec-reliability/evidence/fork-boundary-k78/report.md. No guest/app/privilege work; ten-clone acceptance remains 0/10 unavailable.

- 2026-11-15: Doubt claim: preparation must not authorize a later unsafe/expired fork, and refusing it must not create kernel-error authority. Executable RED/GREEN controls are the behavioral disproof instrument; Grove owns the independent fresh-context review as the required review-impl sibling, so no duplicate helper reviewer is spawned. ADR reconciliation: this repairs behavior already required by the inspection spec, with no new architectural decision. Only k78 retires; k81 and aggregate ancestors remain live.
