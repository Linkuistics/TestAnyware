# app-all-guest-traffic-k42

## Goal

Decide how the TestAnyware app carries the guest traffic that it does not carry today, except `vm create-golden`. The owner's rule: the app is the only point that knows where a VM runs and how to reach it. Every other process reaches the app on loopback.

## Context

- The owner said this while `non-macos-runtime-check-k40` was blocked: "all comms should be routed through the app, because it is the only point that should know where the VM is running and how to get to it."
- The owner then narrowed it: `vm create-golden` stays direct. It is a manual operation. A person or an LLM runs it from a `.command` file on macOS, which Terminal opens and which therefore holds the Local Network grant. The root brief already says this, so the brief needs no correction.
- The design only names this way of running `vm create-golden`. It builds no helper for it. A helper that writes and opens the `.command` file is a later delivery, if the owner asks for it.
- Today the app carries only the agent and VNC streams of the Host CLI. The spec says that SSH does not use the app (`docs/specs/testanyware-app.md`, "Golden creation keeps its direct path").
- The direct traffic that this leaf must bring into the app, on a macOS host:
  - the first-contact SSH, the SSH channel and the in-process `PortForward` of the Linux host harness (`cli-rs/crates/testanyware-cli/tests/linux-host-harness.rs`).
  - the same machinery of `windows-host-harness.rs`.
- The failure that showed this: `connect 192.168.64.3:22: No route to host (os error 65)`. It is the Local Network denial of issue #8. Terminal is the only context that avoids it today.
- ADR-0004 (each command opens its own guest connections) and ADR-0019 (the app holds the Local Network grant) are the ADRs to rework. The glossary terms "Direct path" and "Guest stream" change meaning.
- Linux and Windows hosts keep the direct path of today. This leaf changes macOS hosts only.
- The owner wants Linux and Windows hosts to operate identically. The owner is not concerned about driving TestAnyware from a Windows host: that is future work, and it will use WSL. This leaf designs nothing that is specific to a Windows host.
- Windows VMs stay in scope, because the owner still needs them as guests. A design element for the traffic to a guest treats a Windows guest as it treats any other guest.
- The Host CLI must stay compatible, because many projects use only the CLI. Each command, flag, environment variable, exit code and `--json` field keeps its meaning (root brief).

## Done when

- The spec and the ADRs state which guest traffic the app carries on a macOS host.
- The spec and the ADRs name the traffic that stays direct, and why. `vm create-golden` is the one named traffic.
- The design answers these questions:
  - How does a client ask the app for a stream to another guest port, such as 22?
  - How does the app refuse a port or a guest that it does not own?
  - How do the test processes of the two host harnesses reach the guest? They use SSH, a forward to the golden and the agent client.
  - Do those processes use the app protocol directly, or does a shared library layer hide it?
  - How does the design keep the rule that only the user who runs the app can reach it?
  - What do `vm list --json` and `vm start --json` report for an endpoint, now that no client may use it directly?
- The design lists the increments that follow, small and in order, so that a `planning` session can cut them. The first increment is the one that unblocks `non-macos-runtime-check-k40`.

## Notes

- This work voids the release candidate `v2.4.0`. It changes code after the candidate commit `8a7ad3f31fb0f817bc17c9b2051c9f5e38bbc28f`, as the node brief of `app-release-k21` says.
- The increments that follow must end with a new candidate. The checks `non-macos-runtime-check-k40`, `upgrade-identity-check-k36` and `release-publish-k37` run on that candidate. The design should say which leaf rebuilds it.
- `non-macos-runtime-check-k40` already holds the Taskfile tasks `release:linux-harness` and `release:windows-harness`. They need no change unless the design changes how the harnesses start.
- The owner asked for focus in this grove: small increments and no rabbit holes. Keep the design to the traffic named above. Issues #7, #4 and #5 stay later deliveries.
- This leaf reverses a recorded decision about SSH. A `review-design` leaf is earned, because later increments build on the spec.
