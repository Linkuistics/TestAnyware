# 24. One owner ends each guest command, and it holds the leader until the group is signalled

Date: 2026-11-18

## Status

Accepted. Recorded in grove `TestAnyware.gh-issue-14`, leaf
`lifecycle-evidence-k8`. The area's spec is
`docs/specs/guest-command-lifecycle.md`.

## Context

The investigations of issue #14 found these defects in how the agents end a
guest command:

- On a Linux deadline, `subprocess.run` sends SIGKILL to the shell only. A
  forked child survives and keeps running (k5, k204).
- The macOS runner sends SIGTERM, waits 3 s, then sends SIGKILL to the shell
  only (k12).
- The macOS host suite sometimes reports a command that finished as timed out,
  7 of 20 runs (k3). A probe reproduced that receipt when the runner's second
  wait block queued behind a busy dispatch pool. That cause is not proved for
  the guest. Operation 08 and the post-timeout inspection stall did not recur
  (k186, k193, k195).
- Pipes reach end-of-file only when every descendant closes them, so a
  background child can hold a capture open.

The root brief requires a grace period of 2 s at most, then kill, drain and reap
within 2 s. Each known child identity needs a check. A failed check never
proves absence.

## Decision

One owner handles the leader exit, the deadline and a cancellation of one guest
command, in order. The first outcome that it chooses is final.

At a deadline or a cancellation, the owner first checks the kernel state of the
leader without a reap. If the leader has exited, the outcome is `exited` or
`signalled`, whatever the notification order was.

The leader starts in a new session, so its group is the owned group. On a
timeout or a cancellation, the owner sends SIGTERM to the group. It waits for
the leader for 2 s at most, then sends SIGKILL to the group. It sends both signals
while it still holds the unreaped leader. It then reaps the leader and probes
the group with signal 0 for 2 s at most.

Output goes to capture files, not pipes. On macOS, the owner waits on kqueue
sources and holds no dispatch pool worker. A normal leader exit does not signal
the rest of the group.

## Considered options

- **Owner with kernel arbitration and a held leader (chosen).** The held zombie
  keeps its PID, and the kernel does not reuse a live group ID. The group ID
  therefore cannot name an unrelated process. No PID and start-time comparison
  is needed before a signal.
- **Signal the group by ID after the leader is reaped, with a start-time check.**
  Rejected. A PID can return between the check and the signal. The k206
  controller has the same same-tick limit.
- **Kill the group at every leader exit.** Rejected. Existing tutorials can
  start `server &` and depend on it. The record reports the remaining group
  instead.
- **Contain descendants with cgroups or a job object.** Rejected for this issue.
  macOS has no equivalent, and the root brief asks only for stated limits.
- **Keep the pipes and add a drain timeout.** Rejected. A drain timeout makes a
  complete capture look incomplete whenever a background child holds the pipe.

## Consequences

- A finished command cannot be reported as timed out, because the kernel state
  decides the race. The record shows how late the owner was.
- Both candidate mechanisms of the macOS timeout symptom go away. If either
  macOS symptom recurs, the record names it, and acceptance fails.
- A descendant that calls `setsid` escapes the group. The agent does not signal
  or report it. This limit is stated.
- An agent crash leaves its groups running.
- Detached commands keep their behaviour.
