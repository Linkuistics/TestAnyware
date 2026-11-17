# 25. Cancellation is explicit, and a disconnect is not a cancellation

Date: 2026-11-18

## Status

Accepted. Recorded in grove `TestAnyware.gh-issue-14`, leaf
`lifecycle-evidence-k8`. The area's spec is
`docs/specs/guest-command-lifecycle.md`.

## Context

No client can end a running guest command today. The investigations of issue #14
found these facts:

- A close, a reset or a half-close of the client connection does not end the
  command on either agent (k6, k13).
- After CLI SIGTERM, the macOS child kept running until its deadline. Three
  samples show it (k4, k193).
- The Linux agent serves one request at a time. A client deadline does not free
  it (k6).
- On a macOS host, the app relays each guest stream with bytes unchanged, and it
  passes a half-close through (ADR-0004). A half-close is also a normal HTTP
  event.

The root brief requires separate tested contracts for explicit cancellation and
abrupt CLI interruption.

## Decision

A client cancels a guest command by its exec id, with `POST /exec/cancel`. The
client chooses the id and sends it with `/exec`. The Host CLI sends a random
UUID unless the caller gives `--exec-id`.

A cancellation is idempotent. It never changes a terminal outcome. An unknown
id creates a tombstone, so an `/exec` that arrives later with that id ends as
`cancelled` without a spawn. The agent retains records and tombstones for
300 s, 64 at most.

The agent does not watch the connection. A disconnect changes nothing, and the
command runs to its outcome.

When `file exec` receives SIGINT or SIGTERM, the Host CLI sends one cancellation
on a new guest connection. It waits for 1.5 s at most. Then it raises the same
signal again. The caller's fallback after SIGKILL is `file cancel --exec-id ID`.

## Considered options

- **Explicit cancellation by a client-chosen id (chosen).** The id is known
  before any reply, so a cancellation can overtake its own `/exec`. Each
  command gets its own connection, as ADR-0004 requires.
- **Cancel on disconnect.** Rejected. A relayed half-close or a broken host
  connection would end a healthy command. The Linux agent also cannot see a
  disconnect while its only thread waits. A SIGKILL of the Host CLI still
  needs a separate path.
- **Agent-assigned id in a first reply.** Rejected. It needs a streamed or
  two-step `/exec`. Until the reply arrives, the client has nothing to cancel.
- **Rely on the guest deadline.** Rejected. A tutorial verifier needs to stop
  work at once, and the root brief requires a cancellation contract.

## Consequences

- Explicit cancellation and CLI interruption have separate contracts and
  separate tests.
- A parent of the Host CLI sees the same signal death as today.
- An old agent answers `/exec/cancel` with HTTP 404. The Host CLI reports
  `AGENT_ENDPOINT_UNSUPPORTED`, and CLI interruption skips the cancellation.
- A command that an old Host CLI started has no known id and cannot be
  cancelled.
- An agent restart loses every record and tombstone.
