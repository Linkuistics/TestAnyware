# TestAnyware.gh-issue-7-vm-queue — brief

## Goal

Deliver the admission queue of issue #7: one ordered queue for the macOS guest
slots of a Mac, in the TestAnyware app. Then concurrent groves and agents that
start macOS guests with `--wait` wait their turn, and none of them fails while a
slot is about to free.

## Done when

- On a macOS host, `vm start --wait` of a macOS guest waits in the admission
  queue of the TestAnyware app, and the app serves the queue in order.
- A start holds its slot from before its boot until its guest has stopped. The
  returned VM id does not release the slot.
- Without `--wait`, a full limit fails at once with `VM_LIMIT_REACHED`, exit 6,
  as it does today.
- Structured output shows each waiter: a stable request id, its place, who
  asked, and what holds the slots.
- A cancel before the grant, at the grant or during the boot leaks no slot and
  leaves no VM.
- After a client dies, the app quits or the Mac restarts, no slot is lost and no
  live guest's slot is freed. The app never stops a VM that it did not start.
- External occupancy counts. A refusal by the framework sends the start back to
  the head of the queue, with a bounded pause.
- `config.rhai` can lower the slot count to one or zero.
- A macOS golden creation takes its slot from the queue when the app runs.
  Without the app, it runs as it does today. A full limit gives it
  `VM_LIMIT_REACHED`.
- Each command, flag, environment variable, VM id, exit code and `--json` field
  keeps its meaning. The documents state that one user per Mac is the supported
  deployment.
- The `using-testanyware` skill in the Grove repository passes `--wait` for a
  macOS start and runs a long wait in the background.
- A tagged release, published to `linkuistics/taps`, carries the queue. On this
  Mac the live checks pass on that release.

## Decomposition

1. `admission-queue` (design): the spec of the queue, and the rework of the
   contract amendment of 2026-10-19, which left a queue out.
2. `admission-increments` (planning): the working increments of the agreed
   design. They end with a release leaf.

## Pointers

- Issue #7, which the human narrowed to admission on 2026-10-25. Issues #9
  (reuse and the skill), #10 (durable jobs) and #11 (pools, budgets and routing)
  hold the parts that moved. A comment on #4 holds section 4.
- Spec in this area: `docs/specs/testanyware-app.md`, the TestAnyware app with
  its protocol, its lifecycle and its adoption.
- The contract: `docs/architecture/cli-design-contract.md`, with the §4.2
  amendments of 2026-10-19 (`--wait`) and 2026-10-20 (the app runs `vm start`).
- ADRs a session here reads: ADR-0004 (each command opens its own guest
  connections), ADR-0008 (the recovery cycle of golden creation) and ADR-0018
  (the VNC keeper).
- More ADRs: ADR-0019 (the app holds the Local Network grant) and ADR-0021 (one
  Rhai file configures the app and the CLI).
- Glossary terms in play: Slot, Admission queue, External occupancy, macOS guest
  limit, TestAnyware app, Adoption, Direct path, Golden image and Base VM (see
  `CONTEXT.md`).
- Test seams that the leaves here share, agreed with the human:
  - The protocol of the app. A test starts the app in its test mode and drives
    it with several real Host CLI processes at once. A fake `tart` counts its
    guests against a capacity and refuses as Virtualization.framework does.
  - Pure functions, for the decision to admit and the decision to queue again
    after a refusal.
  - The Taskfile live checks on this Mac. Three real macOS starts with `--wait`
    run from separate shells. A `tart run` outside the app counts as occupancy,
    and a golden creation waits for its slot.

## Notes

- The decision log of `plan-k1` holds each requirement, the words of the human
  and the rejected options. Find it with `grove-llm resolve plan-k1`.
- The human asked for focus: admission only, in small increments.
- The skill change lands in the Grove repository. Its leaf asks the human before
  it pushes there. It need not wait for the release, because `--wait` exists
  since 2026-10-19.
- The grove publishes its own release before its finish, as the earlier groves
  on this repository did.
