# single-final-observation-k41

## Goal
Make the actual controller use at most one final process-table query per call,
after the final signal and observed leader exit, with no retry to repair failure.

## Context
This independently verifiable slice follows k39's workload owner/reap boundary.
Read the ancestor worker-scheduler briefs and docs/specs/diagnostic-process-inspection.md.
It does not deliver worker isolation or hard helper timing; the full remainder
stays live on worker-scheduler-k42 under the unchanged k40 charter.

## Done when
- Group and PID-only termination wait for observed exit before their sole final
  query. Normal completion uses the same final-query cutoff gate.
- Live/unknown/late final evidence is failure; missing exit or expired cutoff
  records unknown without launching a query or performing its signal-0 probe.
- Final group KILL EPERM is classified from that sole post-signal table; earlier
  denied TERM stays failure. Preserve group KILL at T and fixed reap ownership.
- Retain old-path failures, green full self-check, frozen inputs and independent
  exact-PID/resource audits; update the durable diagnostic report.

## Notes
- Decomposed k40 at this behavior boundary because querying until favorable
  independently corrupts evidence, and can be corrected in the current runtime.
- Doubt pass: executable old-path replay contests the claim; a further reviewer
  is reserved for the mandatory final subsystem review, already required by k40.
- Delivered: _final_snapshot gates both ordinary and interrupted completion on
  observed exit and the existing cutoff. Termination repeats only exit polling,
  then consumes one final table. Final KILL EPERM uses that table after the signal
  receipt finishes. Earlier signal failures remain sticky.
- Old-controller replay k41-previous-01: four new cases fail, 66 pass and 39 are
  deselected. k41-inputs-01 freezes the previous/current sources and revision;
  only controller.py differs. The same new tests run on both conditions.
- Full k41-final-01: Ruff lint/format, strict Pyright and all 109 tests pass
  (pytest 23.04 seconds). No runtime module or framework API was added.
- Independent k41-audit-02: 967 indexed artifacts verified, 1,004 inputs stable,
  all 182 recorded real PIDs absent, 50 declared resource markers accounted for,
  and 71 exact-argv fixture directories absent after cleanup. The first audit
  stopped on an interpreter/source-snapshot comparison error before cleanup;
  its failure remains in k41-audit-01. The fresh audit corrected that comparison.
- Updated docs/experiments/exec-reliability-diagnostic-foundation.md. Helper timing
  remains conditional, simulations remain labelled, and guest acceptance remains
  0/10 unavailable. The entire remaining worker contract and required final
  subsystem review remain live on worker-scheduler-k42. No parent node closes.
