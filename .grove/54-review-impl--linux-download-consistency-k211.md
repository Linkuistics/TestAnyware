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
