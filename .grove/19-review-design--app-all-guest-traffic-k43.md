# app-all-guest-traffic-k43

**Reviews:** app-all-guest-traffic-k42

## Goal

Read the design of `app-all-guest-traffic-k42` adversarially, in fresh context, before a `planning` session cuts increments from it. Produce findings, not fixes.

## Context

- The design: `docs/specs/testanyware-app.md`, ADR-0004 and ADR-0019, as `app-all-guest-traffic-k42` reworks them. Find the producer's commit by the handle `app-all-guest-traffic-k42`.
- The producer's charter: its body states the owner's rule and the owner's narrowing. A finding that reopens either says so, and it names the evidence.
- The decision log of `non-macos-runtime-check-k40`. It holds the failure that caused this design and the owner's words.
- The root brief of the grove and the node brief of `app-release-k21`.

## Done when

- Each doubt below has a finding, or a sentence that says why it holds.
- The ADR set is checked as a minimum coherent set. Each citation of ADR-0004 and ADR-0019 is checked across `docs/`, the code comments and `CONTEXT.md`.
- The spec is checked against each line of the root brief's done-when, in particular "Each command, flag, environment variable, VM id, exit code and `--json` field keeps its meaning" and "Only the user who runs the app can reach it".

## Notes

Doubts that the producer could not settle in its own session:

- A stream to an arbitrary guest port may widen what a process on loopback can reach. Check that the app limits a request to the ports and guests that it owns.
- The harness test processes may need a library layer that hides the app protocol. Check that this layer does not change a flag, an exit code or a `--json` field of the Host CLI.
- Linux and Windows hosts keep the direct path and operate identically. Check that no wording of the spec changes their behaviour. Check that no design element is specific to a Windows host. The owner treats driving from a Windows host as future work, through WSL. Windows guests stay in scope.
- `vm create-golden` stays direct. Check that the design names it as the one direct traffic and says why.
- The design voids the candidate `v2.4.0`. Check that it says which leaf rebuilds the candidate.
- Check that each increment is small, ordered, and demonstrable on its own.
