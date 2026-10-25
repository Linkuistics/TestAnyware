# admission-increments-k3

## Goal

Cut the agreed design of the admission queue into working increments. Each
increment is a vertical slice that one session can finish and check. The last
increment publishes the release.

## Context

- `docs/specs/admission-queue.md`, which the human agreed on 2026-10-25, and
  ADR-0022. The decision log of `admission-queue-k2` records why.
- The reviews `admission-queue-k4`, `admission-queue-k6`, `admission-queue-k8`,
  `admission-queue-k10`, `admission-queue-k12` and `admission-queue-k14` read
  the design first. Plan from the spec as those reviews and their integrations
  leave it.
- The root brief: the done-when roll-up and the three test seams.
- The earlier groves on this repository each ended with a release leaf. The
  `release:*` tasks of the Taskfile publish a release.

## Done when

- Each increment is an `impl` leaf whose body names what it delivers, the seam
  that tests it, and its checks.
- The small change to the `using-testanyware` skill in the Grove repository has
  a leaf of its own. That leaf asks the human before it pushes there.
- The last leaf publishes a tagged release to `linkuistics/taps`. It runs the
  live checks of the root brief on that release, on this Mac.

## Notes

- The human asked for small increments and for focus.
- The skill change need not wait for the release, because `--wait` exists since
  2026-10-19. The queue only makes the wait fair.
- The parts of #7 that moved to #9, #10, #11 and #4 are not work for this grove.
- The help of `vm start` still says that the waiters are not served in order.
  `cli-contract.rs` checks that text, and `docs/reference/cli-commands.md`
  repeats it. The increment that ships the ordered queue changes all three.

## Decisions (running log)

**The order follows the user's value, then the dependencies.** Fair service is
the part of issue #7 that agents need first. So `queue-order-k16` comes first. It
uses only the own count, which adoption already supplies.

The next four leaves add
the refusal, the setting, the guests that the app can see, and the command that
shows the queue. Golden creation follows. The crash safety follows golden
creation. The skill, the live checks and the release come last.

**`queue-order-k16` keeps the retry of today for a refusal.** A queue that drops
the retry would regress `--wait` beside an unseen guest. So the admitted start
keeps its retry inside its hold. `queue-refusal-k17` replaces it, and only that
leaf changes the help, because only from there does the order hold under every
refusal. The note in the brief about the help names the increment that ships the
ordered queue. That increment is `queue-refusal-k17`.

**The count of guests that the app sees is its own leaf.** Ordering works without
it, because a refusal already proves an unseen guest. The observer, the 5 s
bound and the answer newer than the join are large. They gain nothing from
sharing a session with the queue.

**Golden creation has two leaves.** The boot fix works with the app and without
it, and it replaces a 60 s timeout with `VM_LIMIT_REACHED`. The hold needs
protocol version 2, which `queue-vm-slots-k20` adds.

**`vm slots` comes before the golden hold.** It adds the protocol version 2, the
skew rows and `requested_by`. The golden hold then reuses all three.

**Documents move with their leaf.** A last document leaf would stay stale for
ten sessions. `CONTEXT.md` already holds the terms of the queue from the design.
Each leaf lists the documents of its scope.

**The skill leaf sits late.** It stops to ask the human before it pushes to the
Grove repository. A stop in the middle would hold the AFK leaves behind it. The
leaf says that it can move ahead if the human wants the skill sooner.

**No `review-planning` leaf.** The spec had six rounds of review, and the leaves
follow its sections. Each large leaf names a decomposition seam for the case
that it exceeds one session. A review of an impl leaf is cut when its session
reaches the second-review boundary.

**The release leaf follows the memory of this repository.** The grove publishes
its own release before its finish, so `queue-release-k26` exists in this grove.
