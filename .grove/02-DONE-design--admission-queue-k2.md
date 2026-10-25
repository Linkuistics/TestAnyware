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

## Decisions (running log)

**The queue gets a spec of its own, `docs/specs/admission-queue.md`.** The
membership test passes: the groves of #9, #10 and #11 must read how the app
admits a macOS guest. The grain test sends it to a spec of its own. The queue is
an area with its own terms, and it spans the app, golden creation and the
command surface. The app's spec has 13,700 words about packaging, the socket,
the protocol and adoption.

The queue spec cites the app's spec for the
protocol and for adoption, and the app's spec points to the queue spec. Neither
one restates the other.

**The spec earns a `review-design` leaf.** The spec is a landed spec that the
planning leaf and its increments build on. Its claims about a cancel and a crash
are the kind that a compiler cannot check, and the human rejected a model.
So this session spends no reviewer of its own. It inserts the review before
`admission-increments-k3`.

**The sources agree that only a refusal proves a full limit.** A survey of the
sources on 2026-10-25 found these facts:

- tart names the holders of a refusal from its own `TART_HOME` only, and it does
  not filter them by OS. `tart list` reads the same storage. A VM runs while its
  `tart run` holds an fcntl lock on its `config.json`, so a crashed `tart run`
  shows as stopped.
- The kernel keeps the count of running guests. No interface reads it, and no
  source shows a way to tell a macOS guest from a Linux guest by its process.
- Orchard and the CI runners on tart use a static count of two. On a refusal,
  they fail the job or try again at once. Orchard serves pending VMs by creation
  time, but a later VM can pass one that does not fit. A worker that restarts
  deletes VMs. It does not adopt them.
- On macOS 26.x the count of the framework sometimes stays up after a guest
  stops. Only a restart of the Mac clears it (Apple forum threads 809815 and
  830119, tart issue 1217). The design must report such a refusal, and must not
  hide it.

**Golden creation waits without a bound.** The human chose this from three
options. When the app runs, `vm create-golden` waits in the queue until the app
admits it, and Ctrl-C ends the wait. stderr shows its place and what holds the
slots, and the command gets no new flag.

A person or an LLM runs golden creation from Terminal, where no tool timeout
applies. The human rejected a bound of 600 s, and a new `--wait`
flag on `vm create-golden`.

**A new read-only command, `vm slots`, shows the queue.** The human chose this
from three options. Its `--json` output holds the slot count, each holder, the
queue and the last refusal. `screen size` and `agent windows` are earlier noun
subcommands of the same kind. The human rejected an item in `doctor`, and no
view beyond stderr and the menu.

**The pid and the working directory name who asked.** The human chose this from
three options. The app reads the pid of the client's socket from the kernel. The
Host CLI states its working directory in the request. An adopted VM shows
neither. The human rejected the working directory alone and the process alone.

**`macos_slots` limits the app's share of the two slots.** The human chose this
from two options. The app holds at most `macos_slots` slots, and each running
macOS guest that it sees also counts against Apple's two. So 1 leaves one slot
for another tool, and 0 refuses every macOS start at once. The human rejected a
count that treats `macos_slots` as the capacity of the Mac.

**The human agreed the spec and ADR-0022 on 2026-10-25.** The spec is
`docs/specs/admission-queue.md`. Within the requirements, this session also
settled six points that the human saw before the agreement:

- After a refusal, the app tries one start at a time until the framework accepts
  one.
- `tart_error` is an empty string when the app refuses without a `tart run`. The
  contract forbids a change of type, and `refused_by` names the side that
  refused.
- A start record lets the app remove a start that a crash of the app
  interrupted. `vm list` and `vm stop` do not read it.
- A quit cancels each start that waits or boots, and waits up to 10 s for the
  cleanup.
- Golden creation tries a refused boot again 3 s later, for 30 s at most.
- Protocol version 2 carries `vm slots` and the hold of golden creation.
