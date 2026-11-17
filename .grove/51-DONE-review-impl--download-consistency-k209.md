# download-consistency-k209

**Reviews:** download-consistency-k7

## Goal

Review the k7 download consistency tooling, checker and report adversarially. Do this before `linux-download-consistency-k14` reuses the tooling and before `lifecycle-evidence-k8` relies on the report.

## Context

- The producer is the commit `download-consistency-k7`. Its decision log is in `.grove/50-DONE-impl--download-consistency-k7.md`.
- The report is `docs/experiments/exec-reliability-download-consistency.md`.
- The tooling is in `experiments/exec-reliability/`: `download_k7.py`, `download_k7_shim.c`, `download_k7_check.py`, `download_k7_run.sh` and `test_download_k7.py`.
- The Taskfile entries are `diagnostic:download-consistency-local`, `-check` and `-tools`.
- The evidence is in `experiments/exec-reliability/evidence/download-consistency-k7/`: `local-01` with `local-01-outer`, and `historical/` with copies of the failure D files.
- The run used the host route only: two macOS agent binaries on loopback, with no VM and no relay.

## Done when

- The review tests whether the shim changes the race that it measures. It compares the forced cases with the stress cases, which ran without a pause.
- The review checks that the calibration picks the lookup that supplies `Content-Length` on both agents.
- The review checks the classes in `download_k7_check.py`. It looks for a wrong body that a class could accept, and for a detected failure that a class could call success.
- The review checks the raw client ends, the bounds and the agent stop in `download_k7.py`.
- The review traces each number and each claim in the report to a byte or a record in the retained evidence. It checks the Hummingbird and swift-nio source lines that the causal chain cites.
- The review states whether the host route is enough evidence for the guest result, without a test of the relay.
- Each finding has a severity, a `file:line` and a concrete failure. The review writes its findings to `docs/experiments/exec-reliability-download-consistency-review-k209.md`.

## Notes

The review does not choose a repair. A finding that needs a new measurement names it for the integrate step.

## Decisions (running log)

- Inspection only: no tests, builds, lint, format or diagnostic runs. The graph CLI could not start because a pre-coordination or unverified generation is active; source and retained evidence are the fallback, with no graph coverage claim.
- The stress checker has a material completeness gap: `stress_summary` returns complete for immutable publications without requiring successful reads or rejecting mismatches, and for mutable stress with unclassified outcomes. Record this as a finding before Linux reuse.
- The runner has no independent whole-invocation bound. Per-socket timeouts and worker joins do not supply one; the mutable writer can survive an exception indefinitely. Record the deadline and cleanup gap against the root brief's explicit bounds.
- The retained `Package.resolved` is present. Its absence from an earlier file listing was a listing issue, not a finding.
- The independent binary comparator controls do not govern the transferred immutable case: its expected bytes come from the controller's `allowed-*.bin` files. Record the resulting false-success path against the independent-oracle requirement.
- The host evidence establishes a sufficient metadata/open race, including the historical digest, but does not identify the historical guest/relay execution uniquely. Record the report's stronger attribution as a design-input risk. Unpaused stress still uses the shim and its extra lookup work.
- The before/after digest lists cover the diagnostic tools but omit agent source, dependency inputs and executable subjects. Record the missing freeze evidence without alleging those inputs actually changed.
- The review artifact is `docs/experiments/exec-reliability-download-consistency-review-k209.md`: two P1 and three P2 findings, with retained-byte and dependency-source evidence. Calibration, the reported stress counts, detected failures and the signal temporary-file leak reconcile with the retained records. No tests or workloads were run.
- The findings meet the acceptance/material-risk threshold. Cut one integration sibling immediately before `linux-download-consistency-k14`, with the review handle as its handoff and independent triage as its charter; do not duplicate the finding list or choose a production consistency repair here.
