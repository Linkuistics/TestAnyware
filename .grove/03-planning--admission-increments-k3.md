# admission-increments-k3

## Goal

Cut the agreed design of the admission queue into working increments. Each
increment is a vertical slice that one session can finish and check. The last
increment publishes the release.

## Context

- The spec that `admission-queue-k2` writes, and the decisions that it records.
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
