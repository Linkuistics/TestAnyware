# remaining-direct-dials-k48

## Goal

The last direct dials of a guest on a macOS host, outside golden creation, go through the app. After this leaf, an enumeration of the dials finds only golden creation and its live tests.

## Context

- The design: `docs/specs/testanyware-app.md`, sections "Guest traffic on a macOS host" and "How the host harnesses reach their guests".
- The decision log of `app-all-guest-traffic-k42` holds the enumeration of the direct dials and their classes. The decision log of `app-all-guest-traffic-k46` holds the owner's two answers of 2026-10-21.
- `harness-guest-streams-k47` built the pieces for the Linux harness: the public `app_route` function that opens a guest stream, and the agent client with a caller timeout. Reuse them.

## Done when

- The Windows host harness (`cli-rs/crates/testanyware-cli/tests/windows-host-harness.rs`) takes its agent connections from the app, by the HUT's id. Its forward listens on `127.0.0.1` and opens guest streams by the golden's id.
- During a manual run, another local user can reach that forward. The owner accepted this as a bounded exception, and the spec states its limits. Do not add a peer check for it.
- No release runs this harness. The demonstration is the build and the unit tests. A live run is a bonus when a person builds a Windows host binary by hand. The decision log says which one happened.
- `scripts/live-start-checks.sh` (the qemu-start check) and `scripts/windows-healthy-start.sh` stop their `curl` to a QEMU agent on `localhost`. Each still checks that the start reported an agent endpoint. Then it runs `testanyware agent health --vm <id> --json`. The app dials the endpoint that the spec of the VM records. Neither script passes `--agent`, because the owner's rule is that TestAnyware's own tests and scripts name a VM only by its id. The check keeps its meaning: the agent that the start reported answers.
- The `nc -z` controls of `herdr-acceptance-live.sh` and `app-routing-live.sh` stay. They carry no traffic.
- The module docs of `windows-host-harness.rs` describe the channel and the forward as they now are.
- An enumeration of the dials finds no direct dial on a macOS host outside golden creation and its live tests. Classify the dials as the design's log does. Enumerate every dial first and classify each one. Do not sweep a list of patterns. Check the instrument with the controls of `references/execute.md` in the grove skill, and record them.

## Notes

- Run the two scripts if the session can. They need a QEMU guest and the app. If it cannot run them, the decision log says which one did not run and why.
- Do not change `Taskfile.yml` task text that is not guest traffic. The description of `app:routing-live` is outside this leaf.
- This leaf changes code and scripts after the candidate commit, so it voids the candidate `v2.4.0`. `candidate-rebuild` in `app-release-k21` rebuilds it.
- The owner asked for focus: small increments and no rabbit holes.

## Decisions (running log)

- **The Windows harness takes its agent connections from the app, by the HUT's id.** `AgentChannel::to_vm(id, timeout)` calls `agent_client_with_timeout` on the app socket. Both `windows_host_harness` and `windows_dist_zip_smoke` use it. `agent_endpoint` is gone, so the harness reads no endpoint from a spec.
- **The forward binds `127.0.0.1` and opens a guest stream by the golden's id for each accepted connection.** `ForwardPlan`, `forward_plan` and `forward_plan_here` mirror the Linux harness, without the HUT address and the peer check. Slirp delivers the HUT's `10.0.2.2` to the host's loopback. `golden_endpoints` and `to_socket_addr` are gone. `golden_vnc_password` reads the one field of the spec that the harness needs.
- **No peer check, as the owner decided.** During a manual run another local user can reach the forward. The module docs state the limits of the exception, as the spec does.
- **The two scripts ask for the agent by the VM's id.** `windows-healthy-start.sh` and the qemu-start check of `live-start-checks.sh` run `testanyware agent health --vm <id> --json`. Each still fails when the start reported no agent endpoint. `require_tools curl` is gone from the qemu-start case. `agent health --vm` for an unknown id exits 3 with `VM_NOT_FOUND`, so the check can fail. The `nc -z` controls stay.
- **I fixed three stale phrases in the module docs and comments of the Windows harness.** They named SSH, the `parse_default_gateway` function and a forward on `0.0.0.0`.
- **The unit test fails when I break the rule it covers.** I changed the bind address to `UNSPECIFIED`, and `a_forward_plan_binds_loopback_and_reaches_the_golden_by_id` failed. I restored the file, and `shasum -c` reported OK.
- **The enumeration, and its classes.** The instrument is `dial.sh` in the session scratchpad. I first extracted the tokens that name a connection primitive from the tracked files outside `.grove/` and the binaries: 356 distinct tokens in code files. From them I built one expression of the primitives that occur (`TcpStream::connect`, `lookup_host`, `SshSession::`, `russh::`, `AgentClient::new`, `AgentConfig::new`, `RfbConnection::connect`, `connect_vnc`, `rvnc_connection_connect`, `create_connection`, and `curl`, `nc`, `ssh`, `scp` and `wget` as commands). It ran over the 630 tracked files that are not Markdown, and found 136 sites in 45 files. I classified every file:
  - **Golden creation and its live tests, direct by the owner's rule:** `golden.rs`, `golden_linux.rs`, `golden_windows.rs`, `ipsw_base.rs`, `recovery.rs`, `finalize.rs`, `ssh.rs` of `testanyware-vm`, and `recovery-live.rs` and `ipsw-base-live.rs`.
  - **The app and what it starts:** `dial.rs` and the `/usr/bin/nc` diagnosis of `streams.rs` in `testanyware-app`; `keeper.rs` (tart's VNC on `127.0.0.1`); `health.rs` (the health wait) and `display.rs` (the resolution switch) of `testanyware-vm`, which the app runs for `vm start`.
  - **The Host CLI's direct path, which runs only with no route (Linux and Windows):** `connect_vnc` and the agent client in `commands/mod.rs`, `connect_or_exit` in `agent.rs` and `input.rs`, `screen.rs`, `record.rs`, `viewer.rs`, `vm.rs` (`viewer_connect` is a name), and the library defaults `rfb/connection.rs` and `agent-client/transport.rs`. `app-agent-routing` and `app-vnc-routing` (33 tests) pin the macOS rule, and they passed.
  - **Fakes and units on loopback:** the tests of `testanyware-agent-client`, `app-agent-routing.rs` and `app-vnc-routing.rs`. `app_route.rs` builds an `AgentConfig` only to name the agent. The strings `error.rs`, `keeper.rs` and `monitor.rs` hold a test value or a comment.
  - **Clients of the app, not of a guest:** the `curl --unix-socket` `status()` of `app-adoption-live.sh`, `app-bundle-live.sh`, `app-routing-live.sh` and `herdr-acceptance-live.sh`.
  - **Controls with no traffic:** the `nc -z` probes of `app-routing-live.sh` and `herdr-acceptance-live.sh`.
  - **Not a guest dial:** `release-doctor.sh` (a remedy string), `collect-training-data.sh` (a vision server), the vendored RoyalVNCKit sockets and bindings (the Rust RFB client does not use them).
  - **Not in the design's log, and left alone:** `scripts/tart-vnc-idle-repro.py` dials the host and port of the `vnc://` line that `tart run --vnc-experimental` prints. The unit tests of `tart.rs` show the host as `127.0.0.1`. So it is a loopback dial of tart's own server, a repro of a tart defect, and it "uses no TestAnyware code". The Local Network grant covers no loopback dial, so it is not a guest dial. I did not change it.
  - **Result:** after this leaf, no site is left that dials a guest on a macOS host outside golden creation and its live tests.
- **The controls, seen to fail.** The same `dial.sh` on the files as they were before the work came back dirty: the Windows harness at `HEAD` (`lookup_host` at line 779, `TcpStream::connect` at line 901, `ssh`), the Linux harness at `91a53e0^` (SSH), `windows-healthy-start.sh` and `live-start-checks.sh` at `HEAD` (the `curl` lines), and a copy of `Taskfile.yml` with a `curl` line added (line 365). On golden creation and the app's `dial.rs` it found `SshSession::`, `russh::` and `curl`. On the current harnesses, the two scripts and `Taskfile.yml` it found nothing. A clean read there cannot come from a broken instrument.
- **Not covered by the instrument:** a dial through a primitive that I did not meet in the token list. The token list came from every tracked file, so a new primitive would need to be a name that no file uses today.
- **The ADR set needs no change.** ADR-0009 and the spec already describe the loopback forward, the guest streams and the bounded exception.

### Demonstration

All runs were on 2026-10-21 from this agent session. The development bundle of the app was running (`testanyware doctor`: running, `local network: a dial to 192.168.64.22:8648 connected`).

- **`scripts/windows-healthy-start.sh`:** `OK: testanyware-980badc5 agent healthy at localhost:53520, TPM attached`, exit 0, 19.7 s. Its `agent health --vm` ran through the app.
- **`scripts/live-start-checks.sh qemu-start`:** `qemu-death` passed (exit code 1, `QEMU_FAILED`, the VM gone), then `✓ the start names its agent at localhost:53662` and `✓ that agent answers /health`, `rc=0`.
- **The Windows host harness, live:** `TESTANYWARE_WINDOWS_HARNESS=1 TESTANYWARE_WINDOWS_BIN=cli-rs/target/aarch64-pc-windows-gnullvm/release/testanyware.exe TESTANYWARE_WINDOWS_FFMPEG_DIR=~/.cache/taw-ffmpeg-sr/aarch64-windows/bin cargo test -p testanyware-cli --test windows-host-harness -- --ignored --nocapture windows_host_harness`. Result line: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 16 filtered out; finished in 50.36s`. Forward lines: `[forward] agent: 127.0.0.1:54057 -> Vm { id: "testanyware-1b767d30" } agent stream of the app` and the same for `vnc` on port 54058. Passed: the 6 endpoint-free checks and the 11 endpoint-driven checks (`agent-health`, `agent-snapshot`, `agent-windows`, `agent-wait`, `screen-size`, `screen-capture`, `screen-record`, `screen-find-text` with `engine=windows_media_ocr found 'File'`, `input-key`, `input-type`, `input-click`). The digest of `windows-host-harness.rs` was the same before and after the run (`shasum -c` OK), and one writer wrote `harness.log`. The binary under test is the `v2.4.0` build of the Windows exe from earlier in the day, which is not rebuilt: the leaf changes only the test side.
- **Unit tests:** `cargo test -p testanyware-cli --test windows-host-harness`: 15 passed, 2 ignored (the live tests), no warnings.
- `shellcheck` on both scripts reports nothing.

### Not proved

- **`live-start-checks.sh` other cases.** I ran only `qemu-start`, the case I changed.
- **A foreign local user on the forward.** The exception is accepted, and no test connects as another user.
- **A Windows host binary built by hand.** No release runs this harness, and the binary I used is the one from the cross-build of the candidate.
