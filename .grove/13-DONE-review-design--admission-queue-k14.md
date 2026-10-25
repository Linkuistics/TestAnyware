# admission-queue-k14

**Reviews:** admission-queue-k13

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k13`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k13`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k12` and of one reviewer
  that k13 ran.
- `docs/specs/admission-queue.md`: "The count" with "When tart does not
  answer", the lock of "The admission decision", and the gates of "A cancel".
  Also "A record that tart did not decide", the hold of golden creation and
  `POST /v1/slots`. Also the new scenario of "External occupancy counts", the
  test seams and the open risks.
- The code of today: `StartGate` and the start thread that holds its permit,
  the stop route, `vm_exists_in`, and the IPSW build of a base.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

This is the sixth review of the design, after k4, k6, k8, k10 and k12. The
human asked for focus, so report a case only when it is worse than the spec
admits. The human rejected a Quint model of the queue, and no test exists for
these rules yet.

**One observer.** The app now makes one observation at a time. A request that
comes during an observation waits for the next one, and the requests that wait
together share it. A newer answer replaces the last answer whole, with each
removal of a stop. Check that no older `tart list` can become the last answer.
Check a stop that overlaps an observation, and an observation that fails late.

**The retry on the observer.** The observer now runs each retry of a
fourth-row record after an observation whose `tart list` answered. It tries
the gate of the VM id and never waits for it. A start of the id holds that gate
while it waits in the queue. Check for a deadlock with a start, a stop and the
queue lock. Check a start of the id that replaces the record.

**The note of a running VM.** A list that reports the VM running leaves a note
in memory. The third row then decides at the first retry that gets the gate,
whatever later lists report. The second row needs a newer read under the gate,
and only when no list of the run reported the VM running. Check a gate that a
stop holds when such a list comes. Check a quit and a crash between the note
and the decision.

**The tick.** The app now observes every 3 s while a fourth-row record stays.
Check that the tick ends when the record goes, also when a start of the id
replaces it.

**Smaller changes.** The bound of 5 s now applies to each call to tart. A
golden hold names each VM that golden creation boots, `<base>-building` of an
IPSW build included. Check that the per-call bound keeps the wait of a waiter
bounded. Check that a person's run of the finished base still counts.

**The sets.** k13 changed only the queue spec. The glossary, the app's spec,
ADR-0022 and the amendment of 2026-10-25 in
`docs/architecture/cli-design-contract.md` must still agree with it.

## Decisions (running log)

**The review is anchored to `admission-queue-k13`, commit `f90ba1dd`.** The
working copy was empty directly above that commit. I read its queue-spec diff,
its decision log, the root brief and the agreed requirements in `plan-k1`.
This is an inspection-only review: it runs no test, build, lint or format
command and makes no production or test-code change.

**The gate rule closes the reported lock cycle.** Today's start thread keeps
its id permit through the whole start, including cleanup. The revised observer
tries that gate without waiting and holds no queue lock during a retry. A start
waiting in the queue therefore cannot stop the observer by holding its id.
The running-VM note also survives a skipped retry, until the first retry that
gets the gate. These rules do not establish a bound on the observer's I/O.

**A bound on each tart call does not bound a waiter's deadline decision.**
Each event must obtain an observation, and an event during an observation waits
for the next one. The observation may make several calls, each lasting almost
5 s. Recovery retries also run before the next observation. F1 records how this
can postpone expiry and permit an admission long after the configured deadline.
The failed-observation risk admits a conservative refusal at a deadline, not a
deadline that waits behind successful I/O. This warrants integration before
planning.

**`admission-queue-k15` integrates F1 next.** It was inserted before the first
later sibling with live work, `admission-increments-k3`, so planning cannot
intervene between these findings and their triage. This review applies no fix.

## Findings

### F1 — Medium: the observer can postpone expiry beyond the configured wait

**Passage.** `docs/specs/admission-queue.md:158` requires another observation
before each admission decision. Lines 163–168 make an event during an
observation wait for the next one. Lines 170–175 bound each individual tart
call, with no bound for the whole observation. The decision's events include
expiry at line 283, and lines 286–289 admit before expiring remaining waiters.
Lines 643–648 also put every retained-record retry before the next observation.

**Case.** Two macOS guests hold both slots, and A is an already queued
`vm start --wait=2`. The same user's tart storage also has several running Linux
guests, which take no macOS slot but still need OS queries under lines 142–147.

1. A `vm slots` observation starts just before A's deadline. Its `tart list` answers,
   and it reads the OS of each running guest. Each call answers successfully in
   4 s. Reading these guests in sequence is permitted by the spec, so ten OS
   reads take about 40 s without any failed observation or timed-out call.
2. A's deadline arrives during that observation. Its decision event must wait
   for the next observation, which cannot start until this observation and its
   recovery retries end. The next queue tick also waits for that next
   observation. The event has no specified way to expire A meanwhile.
3. If both slots stay occupied, A remains waiting well beyond 2 s before it
   can get `VM_LIMIT_REACHED`. More Linux guests or retained records increase
   that delay; no bound independent of their number is stated.
4. Alternatively, a macOS guest stops during the delay. The next observation
   sees a free slot. The decision admits A before expiring the remaining
   waiters, although A's deadline passed tens of seconds earlier.

This contradicts the last decision **at** the deadline in lines 421–432, the
last try at the deadline **at the latest** in lines 987–992, and the configured
bound retained by the CLI contract at
`docs/architecture/cli-design-contract.md:351`. The same observation dependency
also delays an immediate refusal of a start without `--wait` at lines 398–408.
Even concurrent OS reads need an explicit deadline policy: an event can still
wait behind the current observation and then the next one, each containing
several dependent calls. There is no failure here to which the risk at lines
1236–1243 applies. That risk permits a refusal at a deadline from retained
occupancy while tart fails, rather than postponing the deadline itself.

**Smallest useful correction.** Specify how the queue handles a deadline while
the observer is busy, so expiry and its last admission decision do not depend
on completing another observation or recovery batch. The policy must state
which existing observation that decision may use and preserve the conservative
count when freshness is unavailable. Define the corresponding immediate path
for a start without `--wait`. Extend the protocol seam with slow successful
calls, multiple running Linux guests and a deadline that arrives mid-observation;
check both a full count and a slot that frees only after the deadline. This
review leaves the policy and all edits to integration.

## Inspection boundaries and supported rules

I applied `codebase-design` to the state ownership, lock order and test seams,
and `decision-records` to the affected decision set. The queue decision remains
a useful pure boundary; the fake-tart protocol seam can expose F1 through real
clients. The missing guarantee is in the timing contract between that decision
and the observer, rather than a need for another module or public interface.
The queue, configuration and guest-connection decisions own distinct trade-offs;
no additional ADR or consolidation is justified by the changes in k13.

The single observer prevents an older observation from completing after a newer
one and replacing it. A stop's overlapping subtraction can leave the count too
high until another answer, including throughout a later tart failure; k13
expressly accepts that cost. A late call after its observation failed is
expressly ignored. These rules support no further finding about stale answers.
They do not make a tart snapshot atomic with a later framework start. I read
the new slow-observation scenario as protection against replacing a newer
answer, rather than proof that an external guest cannot start after the list.

The recovery note is made when a list reports the VM running, including an
observation whose later call fails. It survives a gate held by a stop and gives
row 3 at the first retry that gets that gate, even if the VM has stopped by
then. Row 2 requires a fresh read under the gate and no earlier running note.
A new start of the same id replaces the record under that gate, so its explicit
replacement is a different authority from recovery. A quit or crash can lose
the in-memory note while the record remains; the spec expressly limits the note
to one app run and accepts deletion of a stopped replacement after restart.
I report none of those accepted costs as another defect.

The recovery-only tick is needed while a fourth-row record remains. Removing
that record or replacing it with a live start's record removes that reason for
the tick; a queued start can independently keep the ordinary queue tick alive.
There is no requirement to retain a fourth-row retry for the replacement record.

Source inspection confirmed that `StartGate::acquire` waits for an id permit
(`cli-rs/crates/testanyware-app/src/gate_start.rs:32`), the start thread keeps
the permit through its result and cleanup (`start.rs:192`), and the stop route
takes the same gate before entering the lifecycle (`stop_delete.rs:59`). The
new non-waiting gate operation is an implementation obligation; it does not
exist in today's gate. The proposed queue and observer are not implemented yet.
Today's retry loop checks elapsed time after an individual refused attempt
(`cli-rs/crates/testanyware-vm/src/lifecycle.rs:519`); it has no shared observer
or batch of other VMs' OS queries in front of its next decision.

`vm_exists_in` treats a failed list as false
(`cli-rs/crates/testanyware-vm/src/tart.rs:261`), and `TartRunner::stop_in`
relies on that helper (line 617). The spec explicitly forbids using that false
result to release observed occupancy; closing that implementation gap is
already required. `prepare_in` clears sidecars before replacing a same-id VM
(line 562), which supports the stated explicit-start boundary.

The golden hold now names the setup VM and the IPSW build's actual boot name.
`ensure_base_vm` creates `<base>-building` and renames it only after
configuration (`cli-rs/crates/testanyware-vm/src/ipsw_base.rs:281`);
`provision_boot1` clones the finished base and boots the setup clone
(`cli-rs/crates/testanyware-vm/src/golden.rs:345`). A person's run of the
finished base therefore remains a separate seen holder under the revised rule.
The glossary, the app's lifecycle spec, the queue ADR and the CLI amendment
agree on the affected ownership and protocol responsibilities. None supplies
the missing deadline policy in F1.

The graph CLI rejected `list_projects` because a pre-coordination or unverified
generation was active, and no graph MCP tools were exposed. I used targeted
`rg` discovery and direct source reads instead. Project/generation confirmation
and `check_index_coverage` were unavailable; this review makes no claim of graph
completeness or exhaustive repository coverage.

`Taskfile.yml` exists. This review ran no test, build, lint or format command,
no formal model and no in-session reviewer. F1 is a counterexample to the prose,
not an execution result. The paired integration owns its triage, every fix and
verification.
