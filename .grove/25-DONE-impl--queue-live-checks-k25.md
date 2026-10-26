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

## Decisions (running log)

**One script, one task.** `scripts/queue-live.sh <bundle>` runs the three checks, and `task queue:live
[APP=<bundle>]` wraps it in the form of `app:herdr-acceptance`. The task builds the development bundle
when `APP` is empty. The script is in `scripts:check`, and shellcheck and shfmt pass.

**The checks stop at the first FAIL.** They share guests (check 3 reuses the two guests of check 2 as
its full house, which saves two boots), so a later check has no meaning after an earlier failure. The
exit trap prints PASS, FAIL or "not run" for each of the three.

**"Separate shells" is three bash processes.** Each runs `exec testanyware vm start --wait` in its own
working directory and its own process group. The view must name the pid and the directory of each
asker, and the script compares both. The order of the join is the order of the `since` of the two
holders, and the third waits at place 1 until the stop of the first admits it.

**A background job of a script needs job control for Ctrl-C.** A job started with `&` in a script can
ignore SIGINT. The script starts each job inside `set -m`, and a test showed that SIGINT then ends
the job with exit 130. Check 3 sends that signal to the waiting `vm create-golden`.

**Check 3 builds no golden.** The command uses `--version tahoe` (a vanilla image, so no IPSW build)
and a name that exists nowhere. Both slots are held, so the command passes its fast checks, joins
the queue as a golden waiter at place 1 and never reaches its first `tart clone`. After Ctrl-C the
queue is empty, both holders still hold, and the sorted list of tart names is the one from before
the join. The cleanup removes the golden name if a mistake admitted the command.

**The refusal.** The script refuses before it starts the app when `tart list` shows a running guest.
It names the guest and stops, and it touches only the ids that it made. It was run against a real
foreign guest (`koine-verify-23707`) and it stopped with exit 1 and left that guest running.

**A real run on the development bundle passed.** `task queue:live`, 2026-10-26, bundle
`TestAnyware Dev.app`, build 0.0.1, ids `queue-live-67872-*`:
- Check 1 PASS: the third start waited at place 1 (asked by pid 68450 from its own directory), the two
  holders ran in the order of their join, and the stop of the first admitted the third within 19 s.
- Check 2 PASS: `vm slots` named `queue-live-67872-ext` as `external`. A start was admitted 2 s after
  it asked. The next start waited at place 1, its line named the outside guest, and the end of the
  outside guest admitted it after 1 s.
- Check 3 PASS: a golden creation waited at place 1 behind two guests, and Ctrl-C changed nothing.
The run ended with no guest, no app and no `queue-live` name in tart.

**Finding: old sidecars with no identity fill the count for ever.** The first run stopped at its own
check of the start view. This Mac held 15 sidecar sets of dead guests. The pids were gone, tart did
not list the guests, and 14 of the metas had no identity. The app counts such a meta as a holder
(`docs/specs/admission-queue.md`, "The count": no check can show that its `tart run` is gone), so
the new app showed 14 holders and would have admitted no start. The human agreed to move the 15 sets
to `~/.local/state/testanyware/vms-stale-2026-10-26`, and the script's message now says so. The
spec's rule is as agreed, so this leaf changes no app code. A user who upgrades with such leftovers
meets the same stall, and that is a question for the human (a cleanup at the start of the app, or a
note in the release notes).

**Permissions from the human.** The human allowed this session to stop `koine-verify-23707` and to
quit the installed app, and to move the stale sidecars. Nothing else of the human's state was touched.

**Checks that a test can fail.** The refusal for a foreign guest and the refusal for stale holders
each fired against real state, and each printed FAIL with "not run" for the three checks. No
mutation of a passing check was run: each takes minutes with real guests.
