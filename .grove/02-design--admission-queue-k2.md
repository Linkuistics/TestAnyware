# admission-queue-k2

## Goal

Design the admission queue of issue #7, and write the spec that the human agrees
to before planning starts. The requirements are the decision log of `plan-k1`.
Do not ask its questions again.

## Context

- The decision log of `plan-k1`: nine decisions, and the requirements of section
  1 of #7 that this grove keeps.
- `docs/specs/testanyware-app.md`: "The protocol", with `POST /v1/vms` and its
  cancel on a closed connection, and "Lifecycle and adoption".
- The contract amendment of 2026-10-19 in
  `docs/architecture/cli-design-contract.md` §4.2. It found that only the
  framework's refusal proves that a slot is free, and it left a queue out.
- The slot wait of today: `run_until_slot` and `wait_for_slot` in the VM
  lifecycle crate.
- Golden creation: `provision_boot1`, and its poll of the `tart run` log, which
  does not notice that `tart` has exited. ADR-0008 boots the setup VM again for
  its recovery cycle, and an IPSW base build boots a VM before it.
- Sources to read in place of a survey: Orchard's scheduler, and how the CI
  runners built on tart handle the limit of two guests.

## Done when

- A spec describes the admission queue. It says how the app counts slots, orders
  and admits starts, and observes external occupancy.
- The spec says how a refusal by the framework sends a start back to the head.
  It says how a golden creation asks for its slot, and what structured output
  shows each waiter.
- The spec says why no cancel and no crash loses a slot or leaves a VM. It says
  how the app reconciles after a restart of the app or of the Mac.
- The spec lists each addition to the command surface. Each command, flag,
  environment variable, exit code and `--json` field keeps its meaning.
- The spec records the three test seams of the root brief.
- The ADR set and the contract describe the current design. The amendment of
  2026-10-19 no longer says that the queue was left out.
- The human agreed to the spec.

## Notes

The design must answer these questions, which `plan-k1` found but did not
settle:

- Only a refusal proves that a slot is free. How does the app combine that with
  its own count of the running macOS guests?
- `tart list` shows only the tart VMs of this user. How does the app see a guest
  of another user or another tool?
- How does a golden creation hold its slot across the gaps between its boots?
  How does the app free that slot when golden creation ends or crashes?
- Can a start without `--wait` take a slot that frees while starts wait? The
  queue must not let it jump ahead of them.
- Where does a person or an agent see the queue, as an addition that changes no
  `--json` field?
- What names who asked: the working directory of the client, its process, or
  both?
- A wait that runs out stays `VM_LIMIT_REACHED`. Which details show that it
  waited, and its place in the queue?

The queue must not serialise starts that the limit does not block. That was the
reason of 2026-10-19 for leaving a queue out.

This leaf chooses whether the queue gets a spec of its own or a section of the
app's spec. The membership and grain tests of the spec set decide it. This leaf
also decides whether the spec earns a `review-design` leaf. The human
rejected a Quint model of the queue, because one process behind one lock runs
it.
