# 4. Each command opens its own guest connections, and on macOS the TestAnyware app relays each one

Date: 2026-10-20

## Status

Accepted. First recorded on 2026-05-31, when the Rust CLI dropped the shared-VNC
server. Reworked in grove `gh-issue-8`, leaf `vm-broker-design-k3`, when the
TestAnyware app arrived. The area's spec is `docs/specs/testanyware-app.md`.

## Context

The Swift CLI had a hidden `_server` command, the Shared-VNC server. It held one
VNC connection open on a Unix socket, with a PID file and an idle timeout. It
multiplexed that connection across short CLI invocations. It saved the cost of
an RFB handshake for each command. Its price was a stateful daemon and a split
into client and server inside the CLI. The Rust CLI never built it, and the
Swift tree is gone.

On a macOS host, Local Network privacy now refuses the Host CLI a route to a tart
guest. This happens when the CLI runs under a login service such as herdr
(ADR-0019). Issue #8
puts a long-lived process between every command and every guest: the
TestAnyware app, which the user allows once. That process must carry the
traffic of every command. So the question is what crosses the boundary between
the Host CLI and the app. Either the app relays the connections, or the app runs
the commands.

The prior-art survey (`docs/research/vm-broker-prior-art-a.md`, section 5)
found these failures in tools that carry guest traffic through a host process:

- Multiplexed streams failed in ways that one connection for each stream
  avoids. Lima's gRPC forwarder leaked tunnels and lost a half-close, and Orchard
  moved off gRPC to avoid multiplexing.
- A tool that runs commands for its client hid errors and failed outside a GUI
  session. That tool is UTM's `utmctl`.
- Localhost TCP listeners were reached by websites and by other users, again and
  again.

## Decision

**The Host CLI keeps the protocol work of every command, and each command opens
its own connections to the guest. On a macOS host, each of those connections is
a guest stream, which the TestAnyware app relays to the guest with the bytes
unchanged.**

- The agent client, the RFB client, OCR, the embedded viewer and `screen record`
  stay in the Host CLI. They run on a guest stream as they run on a TCP
  connection.
- A guest stream is one connection to the app's Unix socket. The app opens one
  guest connection for it, and closes that connection when either side closes. A
  half-close passes through.
- The app runs only the lifecycle commands: `vm start`, `vm stop`, `vm list`
  and `vm delete`. They own processes and state that outlive one command.
- Nothing multiplexes two commands onto one guest connection, and no command
  attaches to a connection that another one holds. The VNC keeper (ADR-0018)
  holds one silent connection, and it carries no traffic.
- On a Linux or Windows host, and in golden creation, each connection is a
  direct TCP connection, as before.

The handshake cost that the Shared-VNC server saved stays accepted. Each command
still pays one RFB handshake.

## Considered options

- **The app runs every command, and the Host CLI becomes a thin client.**
  Rejected. The agent client, the RFB client, OCR, the viewer and the recorder
  would all move into the app. The envelope and the exit codes would have to
  cross a new boundary. The viewer and the recorder would need a frame protocol
  of their own. Relaying bytes needs none of that, and it keeps one code path on
  every host.
- **One multiplexed connection for each guest, as the Shared-VNC server had.**
  Rejected for the failures in the survey above. The VNC crash that a held
  connection prevents already has the keeper.
- **A loopback TCP forward for each VM.** Rejected. A TCP listener on loopback
  gives the app no peer credentials on macOS, and the survey's CVEs came from
  such listeners. A Unix socket with a peer uid check held up best.

## Consequences

- The Host CLI's protocol code is the same on every host. Only the connection
  under it differs: a TCP connection, or a guest stream.
- A guest stream adds one relay hop and one bounded buffer. A stall of the relay
  shows as a stream that does not open, and the Host CLI's own timeouts bound
  it.
- **This does not retire the OCR daemon.** The Shared-VNC server and the OCR
  daemon are different problems. One multiplexed a VNC connection. The other
  keeps a Python EasyOCR process warm, because it takes seconds to start.
  `OcrChildBridge` stays for the Linux and Windows OCR path (ADR-0002). See the
  glossary terms *Shared-VNC server* and *OCR daemon* in `CONTEXT.md`.
