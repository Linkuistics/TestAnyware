# agent-client-hyper-k7

## Goal

Move the agent client from reqwest to the HTTP/1.1 client connection of hyper. The client then runs on any byte stream, as ADR-0004 requires. No command changes behaviour on any host.

This increment is the expand step. It adds the stream-generic transport and keeps the TCP path as the default. The guest streams of `app-streams-k8` build on it.

## Context

- `docs/specs/testanyware-app.md`, section "The protocol". Its list of behaviours that the agent client keeps is the contract of this leaf.
- ADR-0004, which gives the reason. ADR-0001, which requires streaming uploads and downloads.
- The crate `testanyware-agent-client`. Callers are the Host CLI commands and the crate `testanyware-vm`. The latter uses it in the display setup, the health wait and golden creation of Windows.
- The decision log of `vm-broker-design-k6`, finding F2. reqwest cannot take a foreign stream, and hyper 1.9 is already in the lockfile.

## Done when

- The agent client takes a connector that returns any async byte stream. A TCP connector is the default for every existing caller.
- Each request opens its own connection, and the client closes it after the response. No connection serves two requests.
- One timeout covers the whole request, from the dial to the last byte of the body. A timeout for one request, such as the one of `exec`, replaces it.
- An upload and a download stream their bodies. A test proves that nothing holds the whole file.
- Each failure keeps its code. A failed dial is `CONNECTION_REFUSED` with its errno. A timeout is `CONNECTION_TIMEOUT`. A connection that ends early is `CONNECTION_DROPPED`. A status or an error body maps as today.
- The client follows no redirect.
- A test runs the client over a stream that is not a TCP socket, such as an in-memory duplex stream.
- Nothing in the workspace depends on reqwest any more, unless another crate needs it.
- `task cli:test` passes. The tests of the agent client from before the change still pass without edits to their assertions.

## Notes

- The transport is the only change. Keep every public method of the client with its meaning, and change a caller only where its type changes.
- The Linux and Windows cross-builds must keep working. Add no C dependency.
- Name the connector seam so that the leaf `cli-app-agent-routing-k24` can plug a guest stream into it.

## Decisions (running log)

- **The connector seam.** `AgentConnector` has one method, `connect()`, which returns a boxed byte stream. `TcpConnector` is the default. `AgentClient::with_connector(config, connector)` plugs in another stream. The leaf `cli-app-agent-routing-k24` uses that constructor. `config.host` and `config.port` then only name the agent in the `Host` header and in messages.
- **One deadline.** `Deadline` holds one absolute instant. The dial, the handshake, the response head and every body read run under it. A download streams under the same instant. `exec` builds its own `Deadline` with its own limit, so its timeout replaces the one of the client.
- **One connection for each request.** Each request sends `Connection: close`. A task drives the connection. A guard aborts that task when the request ends, so a timeout leaks no task.
- **Failure codes.** A failed dial is `ConnectionRefused` with the errno from the `io::Error` chain. A timeout is `Timeout`. Every hyper error after the dial is `ConnectionDropped`, as reqwest's request and body errors were.
- **Public API.** `AgentClient::http()` returned a `reqwest::Client`. It had no caller, and it is gone. `AgentError::HttpStatus` keeps its field type, because `http::StatusCode` is the type reqwest re-exported. The crate now re-exports it.
- **Upload chunk.** An upload reads its file in 64 KiB pieces. The old `ReaderStream` default was 4 KiB. The wire form is the same.
- **Proxy variables.** reqwest read `HTTP_PROXY`. hyper does not, and I added no proxy code. A proxy has no part in the route to a guest. The spec now says so.
- **A live check breaks.** `scripts/live-start-checks.sh keep-on-failure` used a dead `HTTP_PROXY` to starve the agent probes. That no longer works. The check now stops at once and says why. It needs a new fault injection, and the app gives the natural seam. I did not build one. The leaf that adds the app's test mode should port this check.
- **Proof of streaming.** The tests of `streaming_tests.rs` move 32 MiB through a 64 KiB duplex stream. Nothing in them holds the file. They measure the gap between bytes produced and bytes consumed. A mutation that buffers the body made both tests fail with a gap of 32 MiB. The real code passes with a gap far below 2 MiB.
- **Other controls seen to fail.** Without `Connection: close`, the test of one connection per request failed. Without the deadline on body reads, the body-timeout test hung.
- **Review.** I spent no in-session reviewer. The mutation controls cover the claims that the compiler cannot check.
- **Cross-builds.** `cargo zigbuild -p testanyware-agent-client --lib` passes for `aarch64-unknown-linux-gnu` and `x86_64-pc-windows-gnu`. The shell needs `~/.cargo/bin` before `/opt/homebrew/bin`, or the Homebrew `cargo` finds no std for the cross targets. The lockfile lost `reqwest` and `rustls`, so the build has one C dependency less.
- **Source for the hyper API.** I read `hyper-1.9.0` and `hyper-util-0.1.20` in the cargo registry. The client connection is `hyper::client::conn::http1::handshake`. `hyper_util::rt::TokioIo` adapts a tokio stream. hyper keeps a `Content-Length` header that the caller sets (`src/proto/h1/role.rs`).
