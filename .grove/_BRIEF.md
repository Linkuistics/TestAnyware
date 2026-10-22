# TestAnyware.gh-issue-8 — brief

## Goal

Deliver the TestAnyware app of issue #8: a signed menu bar app through which
the Host CLI reaches each guest on a macOS host. Then an agent under herdr, which
macOS refuses Local Network access, can start and drive a macOS guest again.

## Done when

- On a macOS host, each command that reaches a guest goes through the
  TestAnyware app, explicit `--agent`, `--vnc` and `--connect` endpoints
  included. Only `vm create-golden` stays direct.
- When the app is unreachable, a command fails and says why. No command falls
  back to a direct connection on macOS.
- Linux and Windows hosts keep the direct path of today. The release ships no Windows host
  binary (the owner decided on 2026-10-21). A Windows host is future work and will use WSL. The
  x86_64 Linux archive is build-verified only.
- VMs outlive the app, and the app adopts them when it starts again.
- Each command, flag, environment variable, VM id, exit code and `--json` field
  keeps its meaning.
- After the setup, an agent never waits for a human while the app runs. An
  upgrade can ask a person to allow the app again, and until then a command
  fails and names that remedy. macOS starts the app at login, as it starts
  Koine, and a client never starts it. When the app is not running, a command
  fails and names the remedy.
- Only the user who runs the app can reach it. The owner accepted one bounded
  exception on 2026-10-21: the loopback forward of a manual Windows host
  harness run (spec, "How the host harnesses reach their guests").
- One optional Rhai file, `~/.config/testanyware/config.rhai`, configures the
  app and the Host CLI.
- `testanyware doctor` reports whether the app is installed, running, allowed
  and set to start at login, each with a remedy. The `llm-instructions` of the
  CLI describe the app.
- A version mismatch between the CLI and the running app is never silent.
- A tagged release, published to `linkuistics/taps`, installs the signed app
  with the CLI.
- On this Mac, after the grant, the herdr acceptance run passes. An upgrade of
  the app keeps its signing identity and its one Local Network row.

## Decomposition

1. `vm-broker-prior-art` (research-a): the prior art for a CLI that reaches VMs
   through a local app. The design needs its answers first.
2. `vm-broker-design` (design): the spec of the TestAnyware app, and the
   rework of ADR-0004 and ADR-0019.
3. `vm-broker-increments` (planning): the working increments of the agreed
   design, which end with a release leaf. Planning cut them in this order:
   - `agent-client-hyper`: the agent client runs on any byte stream. No
     behaviour changes.
   - `app-streams`: a headless app serves guest streams, and the Host CLI
     routes agent and VNC connections through it when a test variable asks.
   - `app-bundle`: the signed menu bar app, and macOS routes through it by
     default. An agent under herdr can drive a guest that a person started.
   - `app-lifecycle`: the four lifecycle commands run in the app. An agent
     under herdr can start, drive and stop a macOS guest. The herdr acceptance
     run passes on the development bundle.
   - `app-adoption`: VMs outlive the app, and the app adopts them. The safe stop of an older QEMU guest lands before the signal guard.
   - `app-version-skew`: a mismatch is never silent.
   - `app-configuration`: the Rhai file. Linux and Windows hosts read it first, with the `doctor` exception. A macOS host reads it last, when the app reads it too.
   - `app-diagnostics`: `doctor`, the `llm-instructions` and the documents.
   - `app-release`: the notarised cask and a release candidate, then three
     checks and one publication. The checks are the Linux runtime check, the
     upgrade check and the acceptance run on the candidate. The publication
     ships that same candidate. No Windows host binary ships, so no Windows
     runtime check runs.
4. `app-all-guest-traffic` (design, then planning): on a macOS host, the harness traffic goes through the app too. Only golden creation stays direct. Planning cut two increments before `app-release`: `harness-guest-streams`, which unblocks the Linux runtime check, and `remaining-direct-dials`. It cut `candidate-rebuild` inside `app-release`.
5. `clone-disk-size` (design, then planning): the owner's request of 2026-10-21, outside the issue #8 delivery. Golden creation grows the disk of the Linux golden to 50 GB. Planning cut one leaf, `linux-golden-disk-size`. It ships with the next release, and no release leaf exists for it.
6. `default-config-file` (design, then planning): the owner's request of 2026-10-22, outside the issue #8 delivery. When the app starts and finds no `config.rhai`, it writes a file of comments and an empty map. The file changes no setting, and the Host CLI never writes it. Planning cut one leaf, `default-config-file-writer`. It ships with the next release, together with `linux-golden-disk-size`, and no release leaf exists for either.

## Pointers

- ADRs in this area:
  - ADR-0004: each command opens its own guest connections.
  - ADR-0019: the app holds the Local Network grant.
  - ADR-0020: one signing identity, and a cask.
  - ADR-0021: one Rhai file.
- ADRs a session here also reads: ADR-0009 (verification harness), ADR-0010
  (host-side framebuffer) and ADR-0018 (VNC keeper).
- Glossary terms in play (see `CONTEXT.md`):
  - TestAnyware app, Guest stream, Direct path and Adoption
  - Local Network grant, Local Network denial and TestAnyware configuration
  - Host CLI, In-VM agent, Host-side framebuffer, VNC keeper and macOS guest
    limit.
- Spec covering this area: `docs/specs/testanyware-app.md`.
- Evidence: `docs/research/macos-local-network-platform-binary-exemption.md`,
  `docs/research/testanyware-app-local-network-grant.md` and Apple TN3179,
  "Understanding local network privacy".
- Test seams that the leaves here share, agreed with the human:
  - The protocol of the TestAnyware app, the one new seam. A test drives the
    app's executable in its test mode with the real CLI binary. The guests are
    fakes on loopback: a wiremock In-VM agent and a synthetic RFB server.
  - The existing seams of pure functions and injected probes, for decisions
    inside the app such as adoption.
  - The existing Taskfile live checks, for the herdr acceptance run and for an
    upgrade that keeps the app's identity.
  - The existing Linux host harness of ADR-0009, for the runtime proof of the
    direct path on the release candidate. The Windows host harness stays in the
    source as a manual check, and no release runs it.

## Notes

- The decision log of `plan-k1` holds each requirement, the words of the human
  and the rejected options. Find it with `grove-llm resolve plan-k1`.
- Issues #7, #4 and #5 are later deliveries, outside this grove. The design
  leaves room for each of them, and this grove builds none of them.
- The human asked for focus: small increments, and no rabbit holes.
- ADR-0019 records the view "to give the harness its access, not to engineer
  around the denial in product code". The app keeps to that view: the user
  gives the app its access, and clients reach the app on loopback.
