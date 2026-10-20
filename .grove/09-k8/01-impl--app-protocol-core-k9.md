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
