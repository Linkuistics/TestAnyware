# 4. Each command opens its own guest connections, and on macOS only the TestAnyware app dials a guest

Date: 2026-10-21

## Status

Accepted. First recorded on 2026-05-31, when the Rust CLI dropped the shared-VNC
server. Reworked in grove `gh-issue-8`, leaf `vm-broker-design-k3`, when the
TestAnyware app arrived. Reworked again in leaf `app-all-guest-traffic-k42`, when
the tests that drive guests moved to the app. The area's spec is
`docs/specs/testanyware-app.md`.

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

The first rework left the tests that drive guests outside the app. On 2026-10-21
the Linux host harness of ADR-0009 failed in an agent session. Its SSH to its
test VM got `No route to host` (errno 65). The owner then set the rule: the app
is the only point that knows where a VM runs and how to reach it. The owner kept
golden creation direct, because a person or an LLM runs it from Terminal. The
owner also kept the explicit endpoints of `--agent`, `--vnc` and `--connect`,
because each flag keeps its meaning.

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
its own connections to the guest. On a macOS host, only the TestAnyware app and
the processes that it starts dial a guest. Each connection of a command, and of a test
that drives a guest, is a guest stream. The app relays it to the guest with the
bytes unchanged.**

- The agent client, the RFB client, OCR, the embedded viewer and `screen record`
  stay in the Host CLI. They run on a guest stream as they run on a TCP
  connection.
- A guest stream is one connection to the app's Unix socket. The app opens one
  guest connection for it, and closes that connection when either side closes. A
  half-close passes through.
- A stream by VM id reaches only the agent or the VNC endpoint that the VM's spec
  records. The app has no route for another port of a VM. A stream to an
  explicit endpoint reaches the address that the user names with `--agent` or
  `--vnc`, or in the spec file of `--connect`.
- The owner's rule covers each VM that the app holds. An explicit endpoint is
  the one compatibility exception: the user names the address, and the app only
  makes the connection. The app checks neither its port nor its protocol.
  TestAnyware's own tests and scripts name a VM only by its id.
- The app runs only the lifecycle commands: `vm start`, `vm stop`, `vm list`
  and `vm delete`. They own processes and state that outlive one command.
- A test that drives a guest, such as a host harness of ADR-0009, gets each
  connection as a guest stream through the Host CLI's library. A forward in such
  a test accepts on the host and opens a guest stream for each connection.
- Nothing multiplexes two commands onto one guest connection, and no command
  attaches to a connection that another one holds. The VNC keeper (ADR-0018)
  holds one silent connection, and it carries no traffic. The app starts the
  keeper from its own executable.
- On a Linux or Windows host, and in golden creation, each connection is a
  direct TCP connection, as before. Golden creation is the one traffic on a
  macOS host that the app does not carry.

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
- **A stream by VM id to any port of the VM, for SSH.** Rejected. Its one client
  would have been the Linux host harness, which now reaches its test VM through
  the In-VM agent (ADR-0009). Golden creation, the other user of SSH, stays
  direct. A route by id that named a port would give a client every port of
  each VM, and no client needs that.
- **A stream to an explicit endpoint, for a test VM that the test starts with
  tart.** Rejected. It needs no new route, but the test would then know where
  its VM runs, against the owner's rule.

## Consequences

- The Host CLI's protocol code is the same on every host. Only the connection
  under it differs: a TCP connection, or a guest stream.
- The agent client leaves reqwest. A reqwest connector can return only
  reqwest's own connection type, so it cannot carry a guest stream. The agent
  client speaks HTTP/1.1 through hyper's client connection, which takes any
  byte stream. It does so on every host.
- A guest stream adds one relay hop and one bounded buffer. A stall of the relay
  shows as a stream that does not open, and the Host CLI's own timeouts bound
  it.
- After a person allows the app, a session with no Local Network access of its
  own can run the host harnesses. Golden creation, and the live tests of its
  parts, still need Terminal or another context with that access.
- **This does not retire the OCR daemon.** The Shared-VNC server and the OCR
  daemon are different problems. One multiplexed a VNC connection. The other
  keeps a Python EasyOCR process warm, because it takes seconds to start.
  `OcrChildBridge` stays for the Linux and Windows OCR path (ADR-0002). See the
  glossary terms *Shared-VNC server* and *OCR daemon* in `CONTEXT.md`.
