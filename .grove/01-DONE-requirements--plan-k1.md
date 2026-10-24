# plan-k1

## Goal

Establish what the `gh-issue-7-vm-queue` grove builds, in the human's own words.
Issue #7 asks for a host-wide queue for macOS guest slots, and for the reuse of
one instance by each grove. It also asks for an upgrade of the
`using-testanyware` skill. Settle the open questions that the issue leaves, and
cut the next leaves.

## Context

- GitHub issue #7. Read it with `gh issue view 7 --repo Linkuistics/TestAnyware`.
  It names issues #2, #3, #4, #5 and #6. Umbrella issue #8 holds it.
- `docs/specs/testanyware-app.md`: the TestAnyware app, which runs the four
  lifecycle commands on a macOS host. Its "Out of scope" section names the app
  as the one point of admission on a host.
- ADR-0004 (each command opens its own guest connections), ADR-0010 (the
  host-side framebuffer gate) and ADR-0018 (the VNC keeper).
- ADR-0019 (the app holds the Local Network grant) and ADR-0021 (one Rhai file
  configures the app and the CLI).
- The canonical skill is `plugins/testanyware/skills/using-testanyware/SKILL.md`
  in the Grove repository, `Linkuistics/grove`. The `linkuistics` marketplace
  distributes it.

## Done when

- The decision log below answers each open question, or names the leaf that
  answers it.
- The root brief states the goal of the grove, its done-when roll-up and its
  pointers.
- `CONTEXT.md` defines each term that the decisions resolved.
- The next leaves exist, and the body of each names what it must deliver.

## Notes

The code and this Mac showed these facts on 2026-10-24, before the questions:

- The TestAnyware app shipped in 2.4.0, and this Mac runs 2.5.0. On a macOS
  host, the app runs `vm start`, `vm stop`, `vm list` and `vm delete`.
- `vm start --wait` waits in the app. It runs `tart run` again until the macOS
  guest limit admits it, so the app does not serve the waiters in order. A
  closed client connection cancels the wait.
- Golden creation keeps the direct path, and it takes no slot from the app.
- The app is one process for each user. This Mac has one human account.
- The consumer projects start a clean VM with `--id`, export
  `TESTANYWARE_VM_ID` and stop the VM at the end. None of them uses `--wait`,
  and the skill does not use it either.
- Grove runs one session at a time in one working tree. Concurrent groves use
  separate working trees.

The issue is long. Its priority line, in the human's words:

<!-- tc: off because this quotes the human's issue verbatim -->

> **Priority:** ship machine-wide admission control and per-Grove/workstream
> reuse first. Retain Tart for macOS; extend toward durable scheduling, reusable
> pools and distributed execution as evidence warrants.

<!-- tc: on -->

## Decisions (running log)

**This grove builds admission only, and the other parts of issue #7 become
separate issues.** The human said:

<!-- tc: off because this quotes the human verbatim -->

> This is a huge issue. Let's split out all of the issues except admission into
> separate issues, then just do admission in this grove

<!-- tc: on -->

Admission is section 1 of the issue. It is one queue for the macOS guest slots
of a host, with its status, its errors, its cancellation and its recovery. The human
rejected three options. The first added reuse for each grove and the skill
upgrade, as a second release. The second also added durable jobs. The third
was the whole issue.

**Three new issues take the other parts, and #7 keeps admission.** The human
chose this split from three options. Each new issue quotes its sections of #7
word for word, with its acceptance criteria and its rollout step:

- Reuse for each grove and the skill upgrade: section 2, criteria 4, 5 and 13,
  and the reuse parts of criteria 6, 7 and 14.
- Durable jobs and leases: section 3, and the bullet of section 5 about an
  immutable snapshot. This issue also takes criteria 8 and 9, and the lease and
  fairness parts of criteria 6 and 7.
- Warm pools and capability routing: the rest of section 5, and criterion 12.

Section 4, with criteria 10 and 11, goes to #4 as a comment that links #5. #7
gets a new title, and in its body a link replaces each section that moved. The
human rejected two options. One left #7 as it was, with a comment. The other
gave section 4 a new issue of its own.

This session files the issues after the human agrees to the shared
understanding, because a later answer can move a part between them.

**The queue serves one user per Mac, in the TestAnyware app.** The human chose
this from three options. The admission queue lives in the app and serves the
user who runs it. A macOS guest of another user or of another tool is external
occupancy. The app sees it and never stops it, and a refusal by the framework
counts as contention. The documents state that one user per Mac is the
supported deployment.

The app is one process for each user by design, as the section "The TestAnyware
app" of the app's spec states. The previous grove chose that because macOS keeps
the Local Network state for each user account (Apple TN3179). It also rejected a
`launchd` daemon and a root helper. This Mac has one human account.

The human rejected two options. One was a machine-wide service for every user.
The other was a shared lock directory, with no order across users and locks
that a crash can leave.

**The queue counts macOS slots only.** The human chose this from three options.
Two slots is the default, and `config.rhai` can lower the count to one or zero
(ADR-0021). Linux and Windows guests take no slot, and they start as they do
today. The CPU, memory and disk budgets of section 1 move to the issue for warm
pools and capability routing.

The problem is Apple's limit of two guests, and Linux and Windows guests have no
such limit. A Linux or Windows host has no app, so a budget there needs a new
authority. This Mac has 128 GB of memory, 16 cores and 2 TB of free disk. The
human rejected two options. One added a memory budget for each guest that the
app starts. The other added budgets for CPU, memory and disk.

**A macOS golden creation takes its slot from the queue.** The human chose this
from three options. Golden creation asks the app for a slot, waits in the same
queue, and holds the slot until its setup VM has stopped. Its guest traffic
stays on the direct path. When the app is not running, golden creation runs as
it does today, because no other TestAnyware start can then take a slot.

Golden creation boots one macOS setup VM at a time, and an IPSW base build boots
one before it. The request for a slot goes over the app's Unix socket, which
Local Network privacy does not cover. So it works from Terminal, where a person
runs golden creation. Linux and Windows golden creation take no slot.

Today a full limit fails golden creation after about 60 seconds, as a VNC boot
timeout. The poll of its `tart run` log does not notice that `tart` has exited.
In either case a full limit must give `VM_LIMIT_REACHED`. The human rejected two
options. One left golden creation outside the queue, as external occupancy. The
other failed golden creation with `APP_UNREACHABLE` when the app is not running.

**`vm start` still fails at once by default, and `--wait` joins the queue.** The
human chose this from three options. Without `--wait`, a full limit fails at
once with `VM_LIMIT_REACHED`, exit 6, as the contract amendment of 2026-10-19
states. With `--wait`, the start joins the admission queue, and the app serves
the queue in order. The remedy of the error and the skill tell an agent to pass
`--wait`.

An agent's tool call often has a shorter timeout than the wait. Claude Code's
shell tool stops a call after 2 minutes by default, and a bare `--wait` lasts
600 seconds. A killed call closes the connection, the app cancels the start, and
the agent loses its place without a reason. A fast error tells the agent to wait
on purpose, in the background.

So the queue helps only a client that passes `--wait`, and today no client does.
The human rejected two options. One made waiting the default, with a new
`--no-wait`. The other let `config.rhai` choose the default for each Mac.

**This grove makes a small change to the canonical skill.** The human chose this
from three options. A leaf of this grove edits
`plugins/testanyware/skills/using-testanyware/SKILL.md` in the Grove repository.
In its workflow, a macOS start passes `--wait`, and a long wait runs in the
background. That leaf asks the human before it pushes to the Grove repository.
The workflow of reuse stays in the reuse issue.

`--wait` exists since 2026-10-19, and the installed 2.5.0 has it. So the change
need not wait for the release of this grove, because the queue only makes the
wait fair. The `linkuistics` marketplace updates itself from the `main` branch
of the Grove repository. The human rejected two options. One changed only the
help, the `llm-instructions` and the remedy of TestAnyware. The other filed an
issue on the Grove repository.

**Tests reach the queue through the three seams of the app's spec.** The human
agreed to this proposal, and each seam grows:

- The protocol of the app. A test starts the app in its test mode and drives it
  with several real Host CLI processes at once. The fake `tart` counts its
  guests against a capacity and refuses as Virtualization.framework does. This
  seam covers criteria 2 and 3 of the issue.
- Pure functions, for the decision to admit and the decision to queue again
  after a refusal. Each is a function of the queue, the observed guests and the
  configured count.
- The Taskfile live checks on this Mac. Three real macOS starts with `--wait`
  run from separate shells. A `tart run` outside the app counts as occupancy,
  and a golden creation waits for its slot. This is criterion 1 of the issue,
  for the one deployment that the human chose.

The protocol seam covers the order of service and a wait that runs out. It
covers a cancel before the grant, at the grant and during the boot. It covers a
lower count, external occupancy, and a refusal while the app believes that a
slot is free. Golden creation's request for a slot and a quit of the app while
starts wait complete it.

The human rejected two options. One added a Quint model of the queue to the
design. The other proved the queue with live checks only.

**No research leaf comes before the design.** The human chose this from three
options. The design leaf reads the sources that it needs. Examples are Orchard's
scheduler, and how the CI runners built on tart handle the limit of two guests.

The queue is one ordered list in one process on one host. The amendment of
2026-10-19 found that only the framework's refusal proves that a slot is free.
The survey of the previous grove covered process identity and adoption. The
human rejected one survey and a pair of surveys.

**The human agreed the shared understanding on 2026-10-25.** It holds the nine
decisions above, and these requirements of section 1, which the issue states
and this grove keeps:

- A start holds its slot from before its boot until its guest has stopped. The
  returned VM id does not release the slot.
- Each waiter has a stable request id, its place, who asked, and what holds the
  slots. Structured output shows each of them.
- A cancel before the grant, at the grant or during the boot leaks no slot and
  leaves no VM.
- After a client dies, the app quits or the Mac restarts, no slot is lost and no
  live guest's slot is freed. The app never stops a VM that it did not start.
- A refusal by the framework sends the start back to the head of the queue,
  with a bounded pause.
- Each command, flag, environment variable, VM id, exit code and `--json` field
  keeps its meaning. A wait that runs out stays `VM_LIMIT_REACHED`, exit 6, and
  its details say that it waited.

The queue serves its waiters in order. Fairness between groves or owners is
part of the issue for durable jobs. The grove ends with a release leaf, as the
earlier groves on this repository did. The human chose to proceed at once, and
rejected a review of the issue drafts before they were filed.

**This session filed the split on 2026-10-25.** Issue #9 holds reuse and the
skill upgrade, #10 holds durable jobs and leases, and #11 holds pools, budgets
and routing. All three are sub-issues of #8, as #7 is. A comment on #4 holds
section 4. Issue #7 is now "Admission queue for macOS guest slots in the
TestAnyware app", and a link replaces each section that moved. A script quoted
each moved line of #7 word for word, and a check found every line of the old
body in a new place.
