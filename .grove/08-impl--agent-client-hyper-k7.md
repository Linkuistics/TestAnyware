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
