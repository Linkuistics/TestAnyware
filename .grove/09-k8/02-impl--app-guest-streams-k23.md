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
