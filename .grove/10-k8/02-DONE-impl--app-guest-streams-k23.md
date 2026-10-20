# app-guest-streams-k23

## Goal

Serve guest streams. The app dials one guest endpoint for each stream, relays the bytes unchanged, and diagnoses a Local Network denial where it dials.

## Context

- `docs/specs/testanyware-app.md`, sections "The protocol", "The Local Network diagnosis" and "The socket and access".
- ADR-0004 and ADR-0019.
- `docs/research/testanyware-app-local-network-grant.md`, for the measured timings.
- The existing probe and `nc` oracle code of the vm crate. The health wait already has a rule of five probes. Reuse its types where they fit.

## Done when

- `GET /v1/vms/{id}/agent` and `/vnc` find the endpoint of a VM in the state directory. `GET /v1/endpoints/{host}:{port}/agent` and `/vnc` use the endpoint that the request names.
- After the dial connects, the app answers `101 Switching Protocols` and relays the bytes in both directions.
- Each stream has its own guest connection. The app closes it when either side closes. It reuses none.
- A half-close passes through. Each relay buffer has an upper bound. The app sets no timeout on an open stream.
- A failed dial before the upgrade gives the error object with `code`, `message`, `remediation` and `details`. A refused dial is `CONNECTION_REFUSED` with its errno, and a dial that times out is `CONNECTION_TIMEOUT`.
- The app retries a `host_unreachable` dial for 5 s at most, and then asks `/usr/bin/nc` through an injected oracle. The error carries cause `local_network_denied` or `host_unreachable`.
- The remedy of a denial names the TestAnyware app and the pane of System Settings.
- The app keeps its last observation of the grant. Only a dial to a network that a host interface attaches, other than loopback, changes it. Pure functions with injected probes cover each rule.
- The app opens no host file or host socket for a client.
- Tests use a wiremock In-VM agent and a synthetic RFB server on loopback. They cover a half-close and a large transfer.

## Notes

- The app never reads a setting from the environment of a client.
- A QEMU guest is on loopback and never asks. The diagnosis matters for tart guests.

## Decisions (running log)

- **The route grammar lives in the shared crate.** `testanyware-app-protocol::stream` holds `parse_stream_path` and `StreamTarget::path`, so the parser and the builder agree by test. The leaf `cli-app-agent-routing-k24` calls the builder. The module also holds `STREAM_UPGRADE_TOKEN` (`testanyware-stream`). The spec named only "with `Upgrade`", so the spec names the token now.
- **A VM id is never decoded.** An id has ASCII letters, digits, `.`, `-` and `_`, and no leading dot. Any other id is `VM_NOT_FOUND` with no look at the disk. A test places a decoy spec above `vms/` and shows that `..%2Fevil` and `..` dial nothing. An endpoint is `host:port` with a required port. An IPv6 address needs brackets, because `::1:80` would hide a mistake.
- **The error vocabulary is the Host CLI's.** A missing spec is `VM_NOT_FOUND`, and a spec with no `agent` section is `NO_CONNECTION_SPECIFIED`. A spec that does not read is `IO_ERROR`, and a bad endpoint is `INVALID_ENDPOINT`. The messages copy the ones of `resolve.rs`. The HTTP statuses are 502, 504, 404, 409, 500, 400 and 405. The spec lists them, and the Host CLI reads the code only.
- **The state directory is the `state_dir` of `VmPaths`.** The app reads `{state_dir}/vms/{id}.json`. The test mode argument `--state-dir` therefore names the directory that holds `vms/`, as `VmPaths` does. I added `VmPaths::new` for explicit directories.
- **The dial rules, in `dial.rs`.** The dial is generic over the connect and the oracle, so a test drives each rule with scripted results and paused time. Only `host_unreachable` retries, with a pause of 250 ms. The window counts from the first try, and a try in progress runs to its end. The last try of a fast failure starts at exactly 5 s, which gives 21 tries. A real denial takes tries at 0 s, 2.25 s and 4.5 s, then `nc`, so about 8 s as the spec says. A refusal, a timeout or another connect error ends the dial at once. An `ETIMEDOUT` errno counts as a timeout, so it is `CONNECTION_TIMEOUT`.
- **One try has a limit of 10 s.** The spec gave none. A try that passes it is `CONNECTION_TIMEOUT`. The Host CLI's own timeout still bounds the whole request.
- **The oracle runs once, for the address of the last try.** It reuses `platform_tool_reaches` of the vm crate. On a host that is not macOS it answers `false`, so the app never claims a denial without proof. I made `kind_from_errno` public in the vm crate, so the dial uses the rule of the health wait and does not copy it.
- **The grant observation, in `grant.rs`.** The state is `NotYetSeen`, `Allowed` or `Denied`. The last two carry the destination and the time. A dial counts only when its address sits in a network of a non-loopback interface that is up. I added one rule that the spec does not state. An address of the host itself does not count, because such a dial never reaches the network. The interface list comes from `getifaddrs`, and it needs the nix feature `net`. The observation lives in `AppContext::grant`. `GET /v1/status` does not report it yet, and a note in `doctor-app-check-k20` says so.
- **The relay is `copy_bidirectional_with_sizes` with 64 KiB for each direction.** Its end of one side shuts down the write half of the other, so a half-close passes through. I read tokio 1.52.1 in the cargo registry for this call, and hyper 1.9.0 for `with_upgrades` and `upgrade::on`, and nix 0.29.0 for `getifaddrs`. Each call has its documentation URL beside it.
- **A known limit.** A client that closes both halves while the guest is silent leaves the guest connection open until the guest closes it after the app's FIN. A guest that ignores a FIN keeps its connection and one relay task. The RFB servers of tart and QEMU close on a FIN, and the agent closes after its response. I did not add a probe of the client, because a Unix socket cannot tell a half-close from a close before a write.
- **The app crate now depends on `testanyware-vm`.** On macOS that brings the objc2 binding crates through the OCR client into the build. The app crate has no AppKit code, and the lifecycle leaves need the vm crate anyway. `cargo zigbuild` passes for `aarch64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`, library and executable. The Homebrew `rustc` has no cross targets, so the build needs `~/.cargo/bin` first on `PATH`.
- **Tests.** The unit tests cover the dial, the observation, the endpoint lookup, the error objects and the relay. They use injected probes and paused time. The integration tests run the real executable against a wiremock agent and a synthetic RFB server. They cover the agent by VM and by endpoint, the RFB handshake and a half-close. They cover a transfer of 48 MiB down and 32 MiB up at the same time. They also cover one guest connection for each stream, an idle stream, and each refusal before the upgrade. The shared crate tests the route grammar.
- **Controls seen to fail.** I broke the code in five ways, and the matching tests went red each time. A relay with no half-close hung the unit test, and the integration test failed after its 30 s read limit. A window of 4 s, an inverted oracle and a refusal that retries each failed the dial tests. A VM id that admits `%` and a leading dot failed the traversal tests. I restored the code after each one. Twelve rounds of both integration binaries, called by their exact paths, gave 0 failures in 24 runs.
- **The flake of the previous leaf.** The new harness starts its fakes before it spawns the app, and it holds the global lock for each test body. The stress run found no hung or empty answer.
- **Review.** I spent no in-session reviewer. Unit tests cover each pure rule, the integration tests cover each refusal with the real executable, and the mutations show that the tests can fail.
- **Not done here.** The Host CLI does not call the new routes. That is `cli-app-agent-routing-k24` and `cli-app-vnc-routing-k25`. I left a note in each, and one in `doctor-app-check-k20` for the observation.
