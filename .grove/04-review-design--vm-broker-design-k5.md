# vm-broker-design-k5

**Reviews:** vm-broker-design-k3

## Goal

Read the agreed design of the TestAnyware app adversarially, in fresh context,
before `vm-broker-increments-k4` cuts increments from it. Produce findings, not
fixes.

## Context

- The design: `docs/specs/testanyware-app.md`, ADR-0004, ADR-0019, ADR-0020 and
  ADR-0021. Find the producer's commit by the handle `vm-broker-design-k3`.
- The evidence: `docs/research/testanyware-app-local-network-grant.md` and
  `docs/research/vm-broker-prior-art-a.md`.
- The decision logs of `plan-k1` and `vm-broker-design-k3`. They hold the
  owner's words and each rejected option. A finding that reopens an owner's
  choice says so, and it names the evidence.

## Done when

- Each doubt below has a finding, or a sentence that says why it holds.
- The ADR set is checked as a minimum coherent set, and each citation of the
  four ADRs is checked across `docs/`, the code comments and `CONTEXT.md`.
- The spec is checked against each line of the root brief's done-when.

## Notes

The producer could not settle these doubts in its own session:

- **The pinned UUID.** One row of the measurements supports it: new code with
  the old UUID, on a build that was not notarised. Check whether a pinned
  `LC_UUID` is safe with `codesign`, notarisation and the crash reporter. Check
  also whether the launcher split, which the ADR rejects, is the safer default.
- **Explicit endpoints.** The app relays a stream to any TCP address that a
  process of the same user names. Judge whether that defeats the purpose of
  Local Network privacy more than the requirements need.
- **The guest stream under the agent client.** The agent client uses `reqwest`.
  Check that an HTTP client can run on a stream after an HTTP/1.1 upgrade on a
  Unix socket. If none can, name the change of client that the spec omits.
- **Golden creation and the directories.** Golden creation keeps the direct path
  and resolves its own data directory. The app's `vm start` reads goldens from
  the app's data directory. Nothing checks that the two agree.
- **The Koine start model.** Requirement 1 of `plan-k1` changed. Look for any
  sentence, in the spec or elsewhere, that still says a client starts the app.
- **Adoption of an older sidecar.** It stops a guest by name only. Check that a
  stop by name covers tart, QEMU, swtpm and the VNC keeper.
