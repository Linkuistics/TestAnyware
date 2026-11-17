# linux-disconnect-availability-k207

**Reviews:** linux-disconnect-availability-k6

## Goal

Review the k6 Linux disconnect tooling, checker and report adversarially. Do this before a later leaf relies on its claims about the blocking boundary or child survival.

## Context

- The producer is the commit `linux-disconnect-availability-k6`. Its decision log is in `.grove/47-DONE-impl--linux-disconnect-availability-k6.md`.
- The report is `docs/experiments/exec-reliability-linux-disconnect-availability.md`.
- The tooling is in `experiments/exec-reliability/`: `linux_k6.py`, `linux_k6_seam.py`, `linux_k6_check.py`, `linux_k6_run.sh` and `test_linux_k6.py`.
- The Taskfile entries are `diagnostic:linux-disconnect-container`, `-live`, `-check` and `-tools`.
- The evidence is in `experiments/exec-reliability/evidence/linux-disconnect-availability-k6/`: `container-01`, `live-01` (a retained failed sample) and `live-02`, each with its `-outer` directory.
- Correction r2 changed the live checker rules between `live-01` and `live-02`. The decision log and the report state why.

## Done when

- The review tests whether correction r2 could select a wanted outcome. It compares the r2 rules with the k13 rules and with the raw `live-01` and `live-02` records.
- The review tests the `queue-release` basis for the handler end on the live clone. It states what the basis can and cannot prove without a sighting of the shell alive.
- The review checks that the seam ownership rule `owned_by_run` and the final sweep cannot kill a process outside the run. It also checks the live owned cleanup.
- The review checks each bound, each clock and each socket-row reading in the seam and the checker.
- The review traces each number and each claim in the report to a byte or a time in the retained evidence.
- Each finding has a severity, a `file:line` and a concrete failure. The review writes its findings to `docs/experiments/exec-reliability-linux-disconnect-availability-review-k207.md`.

## Notes

## Decisions (running log)

- The review is inspection-only. Source reads and read-only JSON arithmetic supply the evidence; no tests, checker runs or new measurements are performed. The CBM CLI refused to start because a pre-coordination or unverified generation was active, so direct source inspection replaces graph evidence.
- The Linux deadline reply and the exclusion of the `systemd --user` adopter are source-backed corrections. The missing lower handler bound remains material: a queued inspection supplies an upper bound, not a measured handler end.
- Numeric-PID signalling in the imported live cleanup does not meet the run-ownership guarantee. The retained cleanup succeeded, but it cannot exclude killing a replacement process after PID reuse. The container's isolated scope and pidfd-backed signalling do not show the same defect.
- The live provenance omits the app identity and post-run hashes of most installed agent modules. This fails the brief's explicit measurement contract; equality of the recorded subjects cannot cover omitted subjects.
- Both checker entry points follow the original absolute run path. A copied bundle can read another checkout's evidence or fail without it. This is a material replay defect, independently of the observed classifications.
- Record these four P2 findings in the durable review and insert integration immediately before the first later live sibling. The integration triages the review by handle; it owns fixes, verification and any decision about further measurement.
