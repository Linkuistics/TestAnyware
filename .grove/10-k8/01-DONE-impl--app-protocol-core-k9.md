# app-protocol-core-k9

## Goal

Create the app executable with its test mode. Add the part of the protocol that needs no guest: the socket, its safety checks, `GET /v1/status` and the version handshake.

## Context

- `docs/specs/testanyware-app.md`, sections "The socket and access", "The protocol" and "Version skew".
- ADR-0004 and ADR-0005. The second one shows how this repository keeps a window apart from its protocol work.
- The Cargo workspace in `cli-rs/`. Read how the CLI crate and the vm crate depend on each other. Keep the new crates free of AppKit.

## Done when

- A shared crate holds the protocol types: the range, the build version, the error object and the socket path rules. It builds on every host.
- The app crate has a library and an executable. The executable has a test mode with the arguments that `app-streams-k8` lists.
- The app creates the socket directory with mode 0700. It refuses to serve when another user owns the directory, or when its mode admits anyone else.
- The app closes each connection whose peer uid is not its own. A pure function takes the peer uid as an argument, so a test can drive it.
- The app removes a socket file that no process answers. A second process exits when it finds the first one on the socket.
- The app and the Host CLI both refuse a socket path of more than 103 bytes. Neither one truncates it.
- `GET /v1/status` answers for any protocol range and any directories. Its fields for the build, the range and the directories keep their names in every protocol version.
- Every other request fails with `APP_INCOMPATIBLE` when the ranges do not overlap, and with cause `directories` when the directories differ. Both errors name both sides.
- Every response carries the app's range and build in headers.
- Tests drive the app in test mode with raw HTTP on the socket. They cover each refusal above.

## Notes

- The Host CLI side of the handshake comes in `cli-app-agent-routing-k24`. This leaf needs a small test client only.
- The skew notice and the remedy table of `app-version-skew-k16` come later. This leaf reports the two builds and the two ranges and nothing more.
- Keep the guest endpoint lookup out of this leaf. It belongs to `app-guest-streams-k23`.

## Decisions (running log)

- **Two crates.** `testanyware-app-protocol` holds the shared types and rules: `ProtocolRange`, `ClientHello`, `Directories`, `AppError`, `StatusResponse` and the socket rules. It depends on `serde`, `serde_json` and `percent-encoding` only, so every host builds it. `testanyware-app` holds the library (`access`, `gate`, `listener`, `server`) and the executable. It has no AppKit code. Its Unix code sits behind `cfg(unix)`, and a Windows build gets an empty library and a `main` that says the app runs on macOS only. `cargo zigbuild` passes for `aarch64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`, library and executable.
- **Header names.** `X-Testanyware-Protocol` (`MIN-MAX`), `X-Testanyware-Build`, and on a request `X-Testanyware-State-Dir` and `X-Testanyware-Data-Dir`, percent-encoded. The spec names them now. The spec did not fix them before.
- **A silent client.** A request with no range, or a malformed one, fails with cause `protocol` and a `null` client range. A request with no directories fails with cause `directories`. The range check runs first. The app compares directories as strings and canonicalises none. The configuration leaf may want to canonicalise.
- **Status code.** `APP_INCOMPATIBLE` answers `409`. An unknown route, after the handshake passes, answers `404` with the existing code `USAGE_ERROR`. The guest routes replace that `404` in `app-guest-streams-k23`.
- **The remedy.** A protocol error carries no remedy: the remedy table belongs to `app-version-skew-k16`. A directories error names `XDG_STATE_HOME`, `XDG_DATA_HOME` or both, whichever differ, because `app-streams-k8` asks for that. The leaf `config-in-app-k33` adds `config.rhai`.
- **A second instance exits 0.** The login item can start a second app, and that is no failure. The executable prints "already answers" on stderr. The spec said only that the process exits.
- **Stale sockets.** A socket file refuses a connection (`ECONNREFUSED`) when its process is gone. Only then does the app remove it. A file that is not a socket stays untouched. Any other connect error stops the app, because it proves nothing about the file.
- **Test mode arguments.** `--test-mode --socket --state-dir --data-dir` and an optional `--config`. Nothing reads `--config` yet. Each of the four arguments `requires` `--test-mode`, and a run without test mode exits 2.
- **Where the Host CLI rule lives.** The refusal of a path over 103 bytes and the directory check are functions of the shared crate, with unit tests. The Host CLI has no socket client yet, so `cli-app-agent-routing-k24` calls them. This leaf wires only the app side. I left a note in that leaf.
- **A flake, and its cause.** The first parallel runs of the integration tests failed in 19 of 60 runs: an empty response, and a hang that left app processes behind. macOS has no atomic close-on-exec for a new socket or pipe. A test thread that spawns the app can hand its child a listener or a pipe that a sibling test has just opened. A "stale" socket then keeps a live listener. A test now holds a global lock for its whole body, and 100 runs of the binary gave 0 failures. The harness of `cli-app-agent-routing-k24` must serialise in the same way, or spawn from one thread.
- **That race is also a product hazard.** The app accepts connections and spawns `tart`, QEMU and `swtpm` from several threads. A child could inherit a client socket. I left a note in `app-vm-start-k27`.
- **A control seen to fail.** My first stress loop picked the wrong binary and reported 60 failures of 60. It measured nothing, and I redid it with the exact path. The corrected loop showed the 19 real failures before the fix and none after.
- **Peer uid test.** No test can connect as another user. The test gives the in-process server an `own_uid` that differs from the real one. It first checks that the same request is answered with the right uid, so the empty answer cannot come from a broken request.
- **The remaining unit gap.** A directory that another user owns has a unit test of the pure function only. A process-level test needs root.
- **Live check not ported.** `agent-client-hyper-k7` asked the leaf that adds the test mode to port the `keep-on-failure` live check. The app has no guest streams yet, so it has no seam to inject a fault. I left a note in `cli-app-agent-routing-k24`, which has the fake guests.
- **Review.** I spent no in-session reviewer. Unit tests cover every pure rule, and the integration tests cover each refusal with the real executable.
- **Source for the hyper API.** I did not read the hyper sources this session. I wrote the server from memory of `hyper::server::conn::http1::Builder::serve_connection` with `TokioIo`, and the compiler and the tests then confirmed it against `hyper-1.9.0` and `hyper-util-0.1.20`. The `server` feature of hyper is needed. That API is the one the previous leaf read in the cargo registry for the client side.
