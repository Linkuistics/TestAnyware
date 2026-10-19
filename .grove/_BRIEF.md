# TestAnyware.gh-issue-8 — brief

## Goal

Deliver the VM broker of issue #8: a signed TestAnyware app through which the
Host CLI reaches each guest on a macOS host. Then an agent under herdr, which
macOS refuses Local Network access, can start and drive a macOS guest again.

## Done when

- On a macOS host, each command that reaches a guest goes through the VM
  broker, explicit `--agent`, `--vnc` and `--connect` endpoints included. Only
  `vm create-golden` stays direct.
- When the app is unreachable, a command fails and says why. No command falls
  back to a direct connection on macOS.
- Linux and Windows hosts keep the direct path of today.
- VMs outlive the app, and the VM broker adopts them when it starts again.
- Each command, flag, environment variable, VM id, exit code and `--json` field
  keeps its meaning.
- After the one-time grant, an agent never waits for a human. When the app is
  not running, the CLI starts it.
- Only the user who runs the app can reach its VM broker.
- `testanyware doctor` reports whether the app is installed, running and
  allowed, each with a remedy. The `llm-instructions` of the CLI describe the
  app.
- A version mismatch between the CLI and the running VM broker is never silent.
- A tagged release, published to `linkuistics/taps`, installs the signed app
  with the CLI.
- On this Mac, after the one-time grant, the herdr acceptance run passes, and
  the grant survives an upgrade of the app.

## Decomposition

1. `vm-broker-prior-art` (research-a): the prior art for a CLI that reaches VMs
   through a local app. The design needs its answers first.
2. `vm-broker-design` (design): the spec and the rework of ADR-0004 and
   ADR-0019.
3. `vm-broker-increments` (planning): the working increments of the agreed
   design, which end with a release leaf.

## Pointers

- ADRs this work reworks: ADR-0004 (no persistent server) and ADR-0019 (the
  Local Network diagnosis).
- ADRs a session here also reads: ADR-0009 (verification harness), ADR-0010
  (host-side framebuffer) and ADR-0018 (VNC keeper).
- Glossary terms in play: VM broker, TestAnyware app, Host CLI, In-VM agent,
  Host-side framebuffer, VNC keeper, Local Network denial and macOS guest limit
  (see `CONTEXT.md`).
- Spec covering this area: `docs/specs/vm-broker.md`, once
  `vm-broker-design` writes it.
- Evidence: `docs/research/macos-local-network-platform-binary-exemption.md`
  and Apple TN3179, "Understanding local network privacy".
- Test seams that the leaves here share, agreed with the human:
  - The protocol of the VM broker, the one new seam. A test drives a real VM
    broker with the real CLI binary. The guests are fakes on loopback: a
    wiremock In-VM agent and a synthetic RFB server.
  - The existing seams of pure functions and injected probes, for decisions
    inside the VM broker such as adoption.
  - The existing Taskfile live checks, for the herdr acceptance run and for the
    grant that survives an upgrade.

## Notes

- The decision log of `plan-k1` holds each requirement, the words of the human
  and the rejected options. Find it with `grove-llm resolve plan-k1`.
- Issues #7, #4 and #5 are later deliveries, outside this grove. The design
  leaves room for each of them, and this grove builds none of them.
- The human asked for focus: small increments, and no rabbit holes.
- ADR-0019 records the view "to give the harness its access, not to engineer
  around the denial in product code". The app keeps to that view: the user
  gives the app its access, and clients reach the app on loopback.
