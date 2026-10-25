# queue-live-checks-k25

## Goal

The Taskfile gains the live checks of the queue on this Mac, with real macOS
guests. The release leaf runs them on the release candidate.

## Context

- The root brief names the third test seam. It has three runs:
  - three real macOS starts with `--wait`, from separate shells
  - a `tart run` outside the app
  - a golden creation that waits for its slot.
- The Taskfile holds earlier live checks to copy in form, such as
  `app:bundle-live`, `app:stream-end-live` and `app:herdr-acceptance`. They take
  `APP=<bundle>` for another bundle, and they quit only the app that they
  started.
- All the earlier leaves of the grove built what these checks drive.

## Done when

- A task of the Taskfile runs the checks. It takes `APP=<bundle>` so that the
  release candidate can pass its own bundle. Its description states what it
  needs and what it changes.
- Check 1: three `vm start --wait` of a macOS guest run from separate shells.
  The third waits. A stop of the first admits the third, and the order of the
  join is the order of service. The check stops its own guests on every exit.
- Check 2: a `tart run` outside the app holds one slot. A start that the app
  admits waits for the second slot only, and `vm slots` names the outside guest
  as `external`.
- Check 3: two guests hold both slots. `vm create-golden` of a macOS guest then
  waits and shows a place. Ctrl-C ends the wait and changes nothing. The check
  does not build a golden.
- Each check prints PASS or FAIL, and the task exits non-zero on a failure.
- The task refuses to start when another guest that this task did not start
  runs. It names the guest and stops. It never stops a guest that it did not
  start.
- The decision log records one real run on the development bundle.

## Notes

- The checks need both macOS slots. Other sessions can take them. Ask the human
  before you stop a guest that this grove did not start.
- A person first allows the Local Network alert for the bundle, once. The task
  stops at such a step as the earlier live checks do, and it goes on when run
  again.
