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

## Decisions (running log)

- **The direct dials on a macOS host, enumerated.** I listed every TCP, SSH, agent-client and RFB dial in the Rust workspace, and every `curl`, `nc` and `ssh` in `scripts/` and `Taskfile.yml`. Then I classified each one. Golden creation and its live tests (`golden*.rs`, `finalize.rs`, `recovery.rs`, `ipsw_base.rs`, `ipsw-base-live.rs`, `recovery-live.rs`) dial guests directly. The owner keeps them direct. The app makes the dials of the health wait, the resolution switch, the adoption probes and the guest streams. The VNC keeper, which the app spawns, dials tart's VNC on `127.0.0.1`. The Host CLI dials directly only when it has no app socket, which is the Linux and Windows path. Outside these, three places dial a guest on a macOS host. The Linux host harness dials with SSH and with its forward. The Windows host harness dials with its forward, and with the agent client on the QEMU loopback port. Two live-check scripts `curl` a QEMU agent on `localhost`: the qemu-start check of `live-start-checks.sh`, and `windows-healthy-start.sh`. The `nc -z` probes of `herdr-acceptance-live.sh` and `app-routing-live.sh` are controls that prove the shell cannot dial. They carry no traffic, and they stay.
- **The Linux harness gets its test VM from the app, not from tart.** The HUT becomes the Linux golden, which `vm start --platform linux` starts through the app, as the Windows harness already starts the Windows golden. The test process reaches the HUT through its In-VM agent, by VM id, on guest streams. So it needs no SSH. ADR-0009 chose a stock Ubuntu image and ssh for one stated reason: the Linux golden was deferred. The golden `testanyware-golden-linux-24.04` now exists on this Mac. Its clones mask `sshd`, and its agent runs `/exec` through `/bin/bash -c` as `admin`, who has passwordless `sudo`. Rejected: a stream to an explicit endpoint, port 22 of a HUT that the test starts with tart. The test would then know where its VM runs, against the owner's rule. Rejected: a lifecycle start of a stock image with no agent, and an `ssh` stream kind. Each adds surface to the app and to the start request for one test.
- **The app gets no new route and no new stream kind.** No client needs a stream to port 22 after the harness change, because golden creation stays direct. A stream by VM id reaches only the agent or VNC endpoint of that VM's spec, and the path names no port. An id with no spec fails with `VM_NOT_FOUND`, as today. The route for an explicit endpoint stays as the owner agreed it on 2026-10-20. `--agent`, `--vnc` and `--connect` keep their meaning. So the app relays them to the address that the user names.
- **The harness forward stays in the test process, and only its dial moves into the app.** TN3179 states that listening for and accepting incoming TCP connections needs no Local Network access. Making an outgoing TCP connection needs it. So the forward still accepts the HUT's connection on the host. For each accepted connection, it opens a guest stream by the golden's id instead of dialling the golden. It reads the golden's spec only for the VNC password, as the Host CLI does for `--vm`.
- **The forward keeps the one-user rule as far as a TCP listener can.** It binds only the address that the HUT dials, not `0.0.0.0`. For a tart HUT that is the host's address on the guest bridge, the HUT's default gateway. The forward then accepts only the HUT's own address, the `src` of its default route. For a QEMU HUT the address is `127.0.0.1`, because QEMU's user network delivers `10.0.2.2` there. On loopback, another local user can connect during a run. That is a visible trade-off: the forward lives only for one harness run.
- **The shared library layer is the Host CLI's `app_route`.** `testanyware_cli::app_route` is already public to the integration tests. `agent_client` builds an agent client on guest streams. The harness needs one more public function: a guest stream for a target and a kind, with the agent client's timeout set by the caller. The harness builds no path and no header. The layer is the code that the `testanyware` binary runs, so it changes no flag, exit code or `--json` field.
- **A host-side check replaces what the stock image guaranteed.** The Linux golden is a desktop image, so it holds libraries that a server lacks. A library that only a desktop has could hide a missing dependency of the binary. I read the candidate `v2.4.0` with `objdump -p`. The binary and its five bundled ffmpeg libraries need only `libc.so.6`, `libm.so.6`, `libpthread.so.0`, `libdl.so.2`, `libgcc_s.so.1` and `ld-linux-aarch64.so.1`, beside each other. Before it stages the binary, the harness checks that no other library is needed.
- **`vm start --json` and `vm list --json` keep their endpoint fields and values.** Each field keeps its meaning, as the root brief requires. An endpoint is where the guest listens. A client hands the id back with `--vm`, or the endpoint with `--agent` or `--vnc`, and the Host CLI routes either through the app. A client that dials the endpoint itself is not TestAnyware traffic. On a macOS host, its own process then meets the Local Network denial.
- **The candidate is rebuilt by a new `impl` leaf in `app-release-k21`.** It runs after `release-without-windows-host-k44` and after the increments of this design, and before `non-macos-runtime-check-k40`. The candidate `v2.4.0` was never published, and `app-release-bundle-k22` already deleted and re-made the unpushed tag once. So the rebuild keeps the version: it moves the local tag `v2.4.0` to the new commit and builds the candidate again.
- **The increments, in order.** (1) `harness-guest-streams`: the Linux host harness reaches every guest through the app. It unblocks `non-macos-runtime-check-k40`. (2) `remaining-direct-dials`: the Windows host harness and the two live-check scripts reach their guests through the app. After it, only golden creation dials a guest on a macOS host. (3) The candidate rebuild above. A `planning` leaf cuts them.
- **I published no browser presentation.** This session runs alone, and the ending command stops any server that it starts. The spec's Mermaid diagrams are the visual record, and they follow the spec's "Start here" convention.
- **Seen, and left alone.** The description of the Taskfile task `app:routing-live` still asks a person to start the guest in Terminal. The app now starts the guest, but the text is not guest traffic, so this design leaves it.
