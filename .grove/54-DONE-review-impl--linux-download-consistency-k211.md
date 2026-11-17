# linux-download-consistency-k211

**Reviews:** linux-download-consistency-k14

## Goal

Review the k14 Linux download consistency tooling, checker and report adversarially. Do this before `lifecycle-evidence-k8` uses the report as design input.

## Context

- The producer is the commit `linux-download-consistency-k14`. Its decision log is in `.grove/53-DONE-impl--linux-download-consistency-k14.md`.
- The report is `docs/experiments/exec-reliability-linux-download-consistency.md`.
- The tooling is in `experiments/exec-reliability/`: `download_k14.py`, `download_k14_seam.py`, `download_k14_check.py`, `download_k14_run.sh` and `test_download_k14.py`. The controller reuses the raw client and the CLI client from `download_k7.py`.
- The Taskfile entries are `diagnostic:linux-download-consistency-container`, `-check` and `-tools`.
- The evidence is in `experiments/exec-reliability/evidence/linux-download-consistency-k14/`: `local-01` with `local-01-outer`.
- The run used a Docker container from the pinned k5 image, not a TestAnyware Linux guest. The Host CLI used `--agent` to a loopback port.

## Done when

- The review tests whether the seam observers change the behaviour that they measure. The observers wrap `system_endpoints.open`, `server.os` and the handler writer. The review compares the forced cases with the unchanged agent's stress result.
- The review checks that each hook changes the file at the event that the report names. It checks that the trace can show a hook that ran at another event.
- The review checks the classes in `download_k14_check.py`. It looks for a wrong body that a class could accept, and for a detected failure that a class could call success. It checks how the checker treats the Host CLI prefix in `append-after-fstat`.
- The review checks the bounds, the cleanup trap and the container disposal in `download_k14_run.sh` and `download_k14.py`.
- The review traces each number and each claim in the report to a byte or a record in the retained evidence.
- The review states whether the container result is enough evidence for a TestAnyware Linux guest. The run did not test the guest kernel, its file system or the relay.
- Each finding has a severity, a `file:line` and a concrete failure. The review writes its findings to `docs/experiments/exec-reliability-linux-download-consistency-review-k211.md`.

## Notes

The review does not choose a repair. A finding that needs a new measurement names it for the integrate step. The report's design implications are inputs for `lifecycle-evidence-k8`, not decisions.

## Decisions (running log)

- 2026-11-18. Review stays inspection-only: source, committed artifact and retained evidence; no diagnostic, test, build, lint or formatting run. The graph CLI cannot start because a pre-coordination or unverified generation is active, so source is read directly and graph coverage is unavailable.
- 2026-11-18. The checker establishes only that some action or pause occurred, not that its event matches the armed event. Record this as a material checker finding; inspect the retained event order separately before attributing it to the measured run.
- 2026-11-18. All retained armed traces name the planned event and place it after open, after fstat or between reads as appropriate. The hook finding concerns missing validation, not an observed wrong-event measurement.
- 2026-11-18. The outer trap selects the shared label key, while the controller selects its unique run value; another k14 run is therefore inside the trap's deletion scope. Record this as a material resource-ownership finding.
- 2026-11-18. Considered requiring allowed-complete for all immutable controls, then rejected that as a finding: this diagnostic deliberately allows a complete measurement to report a wrong body, and exposes that class as a finding. A successful diagnostic is not a claim that every transfer succeeded.
- 2026-11-18. Stress and publication retain aggregates rather than the required item-by-item receipts. The publication success count has no retained per-record byte oracle. Record this acceptance-evidence gap; integration owns any new measurement.
- 2026-11-18. The RSS samples are process-lifetime high-water marks on a reused instrumented agent. Their zero difference does not measure per-transfer RSS growth or establish the stated 16 MiB bound. Record the measurement limitation as a finding against the fixed-memory acceptance claim.
- 2026-11-18. A separate evidence-completeness defect remains: if a normal armed download has an open/action but loses its finish record, the checker skips emitted/received comparison and still marks it complete. Keep intentional SIGKILL cases distinct; they can use retained partial writes rather than a finish event.
- 2026-11-18. Findings are in `docs/experiments/exec-reliability-linux-download-consistency-review-k211.md`: one P1 resource-ownership risk and four P2 validation/measurement gaps. Source and retained receipts were inspected; no tests or diagnostics were executed. The unchanged byte findings remain supported within the container scope.
- 2026-11-18. The only later sibling is live `lifecycle-evidence-k8`. Insert the paired integration immediately ahead of it, carrying this review's handle rather than its findings. No parent closes while those live tasks remain.
