# harness-golden-forward-k51

## Goal

The harness forward stops dialling the macOS golden. For each accepted connection it opens a guest stream by the golden's id, through the app. After this leaf, `linux_host_harness` passes from an agent session that has no Local Network access of its own. This is the demonstration of `harness-guest-streams-k47`, and it unblocks `non-macos-runtime-check-k40`.

## Context

- The brief of `harness-guest-streams-k47`, Notes. The decision log of `app-all-guest-traffic-k42` holds the reasons: "The harness forward stays in the test process, and only its dial moves into the app" and "The forward keeps the one-user rule as far as a TCP listener can".
- TN3179 says that accepting a connection needs no Local Network access. Only an outgoing dial does.
- `harness-golden-hut-k49` added the public `app_route` function that opens a guest stream for a target and a kind. Use it. The harness builds no path and no header.
- The forward today is `PortForward` with `forward_accept_loop` and `forward_splice`. It binds `0.0.0.0:0` and dials the golden's agent and VNC endpoints from the spec.
- The Windows harness has its own copy of the forward. `remaining-direct-dials-k48` changes it. Do not touch it here.

## Done when

- `forward_splice` opens a guest stream by the golden's id, with the agent kind or the VNC kind, instead of `TcpStream::connect`. The harness reads the golden's spec only for the VNC password, as the Host CLI does for `--vm`.
- The forward binds the HUT's gateway address, not `0.0.0.0`. It accepts only the HUT's own address, the `src` of `ip route show default`. A connection from any other peer is closed.
- The address choice and the peer check are pure functions with unit tests. Tests cover the gateway address, the HUT's address, a foreign peer and a missing route.
- Demonstration: `linux_host_harness` passes from an agent session that has no Local Network access of its own. A person allowed the development bundle of the app. The binary is a workspace build for aarch64 Linux. The decision log records the command, the result line and the list of passed checks.
- If the session cannot run the harness, the log says what is missing. It does not claim the demonstration.
- The module docs of `linux-host-harness.rs` describe the forward, the bind address and the peer check as they now are. The text on running the test from Terminal is gone, because the test no longer needs Terminal.
- `task release:linux-harness` still parses. Its description matches the new run.

## Notes

- The Linux HUT is a tart guest, so the gateway and the peer address are right. `harness-golden-hut-k49` made the backend check fail for any other backend.
- The forward lives only for one harness run.
- Do not add a flag, an exit code or a `--json` field to the Host CLI.

## Decisions (running log)

- **The forward opens a guest stream for each accepted connection.** `forward_splice` calls `app_route::open_guest_stream(socket, &StreamTarget::Vm { id: <golden id> }, kind)` with `StreamKind::Agent` or `StreamKind::Vnc`, then runs `copy_bidirectional`. The harness builds no path and no header. An app error is logged with its message, code and remedy.
- **The harness reads the golden's spec only for the VNC password.** `golden_endpoints` and `to_socket_addr` are gone. `golden_vnc_password` replaces them.
- **The route is parsed into one value.** `parse_default_route` replaces `parse_default_gateway` and returns the `via` address (the bind address) and the `src` address (the HUT). A default line without `via`, without `src` or with a token that is not an IP address is an error. Without `src` the forward could not tell the HUT from another peer, so the harness fails and does not guess.
- **The peer check is `peer_is_hut`.** The accept loop closes a connection from any other peer before it spawns a splice. The comparison reads an IPv4-mapped IPv6 address as its IPv4 form.
- **`linux_dist_install_layout` uses the same forward.** The type `PortForward` is shared, so the old dial could not stay for that test. Its VNC forward now binds the gateway, accepts the HUT and opens a VNC stream by the golden's id. I changed no other line of that test. `dist-layout-golden-hut-k52` moves its HUT. The test compiles and its pure parts pass, but I did not run it live.
- **I did not touch `windows-host-harness.rs`.** `remaining-direct-dials-k48` owns it.
- **The Taskfile description says that the session needs no Local Network access.** The task still parses (`task --list-all` prints it).
- **The module docs describe the forward, the bind address and the peer check.** The text on running the test from Terminal is gone. An earlier draft of the docs named an error code, `LOCAL_NETWORK_DENIED`, that does not exist. I found it by a grep before the commit and removed it.
- **The unit tests fail when I break the rule they cover.** I mutated the code and watched the suite fail: the peer check made always true (1 test failed), the HUT address taken from `via` (4 failed), the bind address made `0.0.0.0` in `forward_plan` (1 failed, after I added `a_forward_plan_binds_the_gateway_and_accepts_the_hut_for_the_golden_by_id`; before that test, the same mutation passed all 39). I restored the file after each, and the digest of the restored file matched the backup.

### Demonstration

The run was on 2026-10-21 from an agent session. This session reached the guest only through the app, as the earlier runs of this tree show: their forward dial from the same kind of session failed with `No route to host (os error 65)`. The development bundle of the app was running (`testanyware doctor`: running, `local network: a dial to 192.168.64.17:8648 connected`). The binary was the workspace build for aarch64 Linux, with the ffmpeg libraries of `~/.cache/taw-ffmpeg-sr/aarch64-linux`. The sha256 of `linux-host-harness.rs` was the same before and after the run (`shasum -c` reported OK). One writer wrote `harness.log`.

Command:

```
TESTANYWARE_LINUX_HARNESS=1 TESTANYWARE_LINUX_BIN=$PWD/target/aarch64-unknown-linux-gnu/release/testanyware \
  TESTANYWARE_FFMPEG_LIB_DIR=$HOME/.cache/taw-ffmpeg-sr/aarch64-linux/lib \
  cargo test -p testanyware-cli --test linux-host-harness -- --ignored --nocapture linux_host_harness
```

Result line: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 255.03s`

The forward lines:

```
[forward] the HUT 192.168.64.3 reaches the host-gateway at 192.168.64.1
[forward] agent: 192.168.64.1:51638 accepts 192.168.64.3 only -> Vm { id: "testanyware-584ae005" } agent stream of the app
[forward] vnc: 192.168.64.1:51639 accepts 192.168.64.3 only -> Vm { id: "testanyware-584ae005" } vnc stream of the app
```

Passed checks: the 6 endpoint-free checks (`help`, `capabilities`, `schema`, `llm-instructions`, `doctor`, `dry-run`) and the 11 endpoint-driven checks: `agent-health`, `agent-snapshot`, `agent-windows`, `agent-wait`, `screen-size`, `screen-capture`, `screen-record`, `screen-find-text` (`engine=easyocr_daemon found 'File' at 26x16 px`, cold daemon 8.3 s), `input-key`, `input-type`, `input-click`. The OCR library check read 350 native libraries. The four agent cases and `screen-find-text` failed in the runs of `harness-golden-hut-k49` and `harness-ocr-library-check-k50`. They pass here.

### Not proved

- **A foreign peer in a live run.** The peer check has unit tests and a mutation test. No other peer connected to the forward during the run, so no log line shows a closed connection.
- **A pass from Terminal.** This session cannot run the harness from Terminal. The forward needs no Local Network access now, so Terminal has no special role. A person may still confirm it.
- **`linux_dist_install_layout`.** I changed its forward and did not run it. `dist-layout-golden-hut-k52` moves that test and runs it.
- **That this session lacks Local Network access.** I did not run a direct dial in this session. I rely on the earlier runs of the same kind of session and on the fact that no direct dial remains in the harness forward.
