# helper-fork-error-k72

**Reviews:** complete-worker-scheduler-k69

## Goal

Independently review the explicit helper fork-error slice for incorrect no-child classification or loss of a known helper obligation.

## Context

Find the producer commit by complete-worker-scheduler-k69; read its diff, ancestor briefs, the process-inspection spec and experiments/exec-reliability/evidence/helper-fork-error-k69/report.md. Sources: experiments/exec-reliability/inspection.py, experiments/exec-reliability/worker_capture.py, experiments/exec-reliability/test_worker_capture.py and experiments/exec-reliability/test_inspection.py. Retained conditions are k69-red-01, k69-green-01, k69-green-02; final frozen inputs k69-inputs-02; independent audit k69-audit-01. The session had no GROVE_PROVIDER_EVIDENCE; no Creator is invented or copied from another attempt. Owner declaration may be needed by review routing policy.

## Done when

- Trace the actual fork OSError callback, control publication, parser, terminal validation, failure/shutdown paths and helper obligation retention. Can pipe/setup/admission/exec failure masquerade as fork failure? Can a valid failure followed by malformed/duplicate/late/partial/conflicting publication or terminal remove uncertainty or a known identity improperly? Check timing/deadline/errno and exact schema/type validation, callback publication failure and clean worker exit versus no-child semantics.
- Verify that injected EAGAIN is described as boundary injection rather than induced kernel exhaustion, synthetic created identities are never signaled, query failures remain failures, and later independent absence is not treated as matching reap acknowledgement.
- Assess whether the 23 new controls plus existing healthy/helper/admission controls would catch the material failure modes. Review remedy preservation and unchanged helper capture callers. Retained source/artifact hashes and audit claims must match their actual bounds, including partial historical source-copy coverage.
- Reconcile with the full unchanged k71 charter. This review cannot close aggregate scheduler ancestors or discharge persistent bootstrap, shared scheduler, full identities/epochs, remaining controls, final aggregate review or unavailable acceptance.
- Record findings with source coordinates and concrete consequences. Create an integration sibling immediately before k71 only for actionable findings; no empty integration. Keep k71 and all aggregate ancestors open.
