# admission-queue-k10

**Reviews:** admission-queue-k9

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k9`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k9`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k8` and of one reviewer that
  k9 ran.
- `docs/specs/admission-queue.md`: "Solution", "The count" with "When tart does
  not answer" and "Before the first answer", and "The admission decision".
  Also the start record of "A quit, a crash and a restart", with both of its
  tables, "A restart" and the flowchart. Also the details and the remedy of
  `VM_LIMIT_REACHED`, the stderr lines, the `vm slots` table, the test seams
  and the open risks.
- `CONTEXT.md`: *Holder*, *Contention*, *Start record* and *Adoption*.
  ADR-0022, and the amendment of 2026-10-25 in
  `docs/architecture/cli-design-contract.md`.
- The code of today: the adoption decision and its scan, `stop_app`,
  `stop_proven`, `stop_without_signal`, `TartRunner::prepare_in` and `stop_in`.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model of the queue. So these rules rest on prose and
on the seam of pure functions, and no test exists for them yet. This is the
fourth review of the design, after k4, k6 and k8. Report a case only when it is
worse than the spec admits.

**The own holders.** k9 makes a running guest of a meta an own holder unless
the check of the meta's `tart run` finds it gone or finds another process. A
meta that records no identity still counts. Check a VM that the app started in
this run, and an adopted VM. Check a legacy VM and a meta written with no
identity. Check
also a person's VM under the name of a stale meta, and `vm stop` of that id.

**Before the first answer.** k9 admits nothing until tart first answers in a
run of the app. Check a start without `--wait`, `--wait=0`, a golden creation as
the only waiter, and `macos_slots` 0. Check that some event decides again once
an observation answers.

**The kept observation.** While observations fail, the app keeps the seen
holders of its last answer. It removes each VM that the app stopped, or whose
`tart run` it saw exit, after that answer. Check that no order of events makes
the count too low. Check also how long a count that is too high can last.

**The OS of a VM.** The app now asks `tart get` for the OS of each running VM
at each observation, and it keeps no answer. Check a VM that disappears between
`tart list` and `tart get`, and a `tart get` that fails in another way.

**The start record.** Row 3 now keeps the spec and the meta only when the
meta's own `tart run` matches in full. A table states the result and `removed`
for each outcome of the check. Check each outcome against the requirement that
the app never stops a VM that it did not start.

**The trade-offs that k9 accepted.** A tart that does not answer can make a
waiter fail beside a free slot, and a golden creation wait. An orphan VM of the
app counts against the two slots but not against `macos_slots`. Report one of
these only when the case is worse than the spec admits.

**The sets.** The spec, ADR-0022, the contract's amendment of 2026-10-25 and
`CONTEXT.md` must agree with each other.

## Decisions (running log)

**The review is anchored to `admission-queue-k9`, commit `91c79e95`.** The
working copy was empty on entry. The graph CLI refused `list_projects` because
a pre-coordination or unverified generation was active. It supplied no project,
generation or coverage evidence. This review therefore uses direct source reads
and targeted searches, rather than claims about graph completeness.

**An unresolved start record still leaves authority to stop an unproven VM.**
The timeout row keeps the record and both sidecars, then ordinary adoption runs
and the socket accepts commands. The specified stop reads no start record. A
meta whose main process is gone can therefore still remove a replacement VM
by name. Finding F1 records this remaining path around the stricter third row.

**A release of a process is not a release of every VM of its name.** The new
subtraction from a kept observation identifies a VM by name, but the exit it
uses belongs to a particular process. A replacement can already be in the
latest successful observation when the app learns that the old process exited.
Finding F2 records the resulting undercount and its effect on golden creation.

**The two findings warrant integration before planning.** Each has a concrete
event sequence and cited passages or source. The paired integration will read
this review from its commit and triage its findings. It will be inserted before
the first later live sibling, `admission-increments-k3`, so no intervening work
can move the cited passages. This review makes no design or implementation fix.

## Findings

### F1 — High: the unanswered recovery row leaves an unproven VM stoppable

**Passage.** `docs/specs/admission-queue.md:554` removes nothing and retains
the start record when tart does not answer within 5 s. The startup flow then
runs ordinary adoption and serves the socket (lines 635–638). Lines 599–600
explicitly leave `vm stop` reading only the sidecars, with no start-record
check. The stricter sidecar rule at lines 559–570 applies only to row 3, where
tart answered that the VM runs.

**Case.** A start of X writes its meta and crashes before removing its start
record. While the app is down, a person stops and deletes that VM and runs a
new VM under X. At the next app startup, the old recorded process is gone,
but the tart query times out. Row 4 keeps the record, spec and meta. Ordinary
adoption reports the old process as stopped and advises `vm stop X`. When that
command runs and tart answers again, it stops and deletes the person's new VM.
No new `vm start --id X` was requested.

**Source evidence.** The adoption decision returns `Stopped` for a gone main
process (`cli-rs/crates/testanyware-vm/src/adoption.rs:127`). The scan leaves
the sidecars and recommends `vm stop` for that outcome
(`cli-rs/crates/testanyware-app/src/adoption.rs:110`). The stop route takes
the id's gate and calls the lifecycle stop without consulting adoption results
(`cli-rs/crates/testanyware-app/src/stop_delete.rs:59`). `stop_app` passes
`Stopped` to `stop_proven` (`lifecycle.rs:608`). That function refuses only
`Doubt`; with a gone main process it calls `TartRunner::stop_in` with pid 0
(`lifecycle.rs:636`, `lifecycle.rs:649`, `lifecycle.rs:656`). The tart stop
still removes an existing VM by name (`tart.rs:617`). The absence of a signal
does not protect the replacement.

This has the start record whose new recovery contract is meant to protect the
replacement. It is distinct from the accepted adoption hazard for a stale meta
with no start record, and from the accepted availability cost of unanswered
observations. Denying admissions until the first answer does not deny a stop.
It violates the recovery requirement at lines 937–957 and bypasses the
protection that row 3 now gives the same VM when tart answers.

**Smallest useful correction.** Specify how an unresolved start record keeps
its unproven sidecars from authorising a stop after the socket opens. Keeping
the evidence for a later recovery decision is compatible with withholding that
authority. State when recovery is retried or the restriction ends, and cover
the timeout followed by a successful `vm stop` through the existing protocol
seam. A legacy meta beside the unresolved record needs the same check: its
ordinary stop also removes the VM by name.

### F2 — Medium: a late exit can erase a replacement from the kept observation

**Passage.** `docs/specs/admission-queue.md:156` retains the last answered
observation and subtracts each VM whose `tart run` the app saw exit after that
answer. Lines 147–150 already establish that a name can designate a replacement
VM. The release rule does not distinguish the observed instance from the
process that exited. The queue lock orders state changes, but observations run
outside it (lines 152–154 and 253–256).

**Case.** Use the default `macos_slots` of 2.

1. The app has a running start X and an admitted golden creation G. G is
   between boots and holds its slot through its connection.
2. The old `tart run` of X exits. Before the app observes that exit, a person
   replaces X with a running macOS VM of the same name.
3. A new observation answers successfully and reports the replacement X as
   running with OS `darwin`. This is now the app's last answered observation.
4. The app then observes the exit of the old process. Under the new rule, it
   releases the old start's hold and removes X from that observation because
   it saw the exit after the answer. Subsequent observations fail.
5. Only G remains in the count. The app admits a waiting start S, and S boots
   beside the replacement X. G's next boot then receives refusals for 30 s
   and fails, although its hold never ended.

The lock cannot distinguish the two VMs when the subtraction records only
their common name. The existing spawn path also gives no ordering of a child's
exit with tart observations: each detached child is reaped on a separate
thread (`cli-rs/crates/testanyware-vm/src/detached.rs:37`,
`detached.rs:79`). The event order above is permitted by the proposed rules;
this is a counterexample to the prose, not a claim that queue code exists yet.

This case loses occupancy that the latest successful observation actually
reported. It therefore differs from the accepted risk of a guest the app
cannot see, and from keeping too many holders while tart fails. The new
subtraction makes the count too low and defeats the stated reason for keeping
seen holders: protecting G between its boots (lines 1102–1108).

**Smallest useful correction.** Define which observed occupancy an exit is
allowed to remove. Observing an old process's exit after an answer does not
prove that the VM in that answer belonged to that process. When the association
cannot be proved, retain the occupancy or require a fresh successful
observation before treating it as absent. Give observation publication and
release reconciliation one ordering rule. Extend the existing count seam with
same-name replacement, a delayed old exit, and later failed observations; the
test must preserve the replacement's slot and G's hold together.

## Inspection coverage and limits

This review read the committed changes of `admission-queue-k9` at `91c79e95`,
its decision log, the root brief and `plan-k1` requirements. The bounded design
read covered the queue spec, the affected app-spec passages, the amendment of
2026-10-25 in the CLI contract, the glossary terms, and the cited configuration,
admission, guest-connection, recovery and keeper decision records. The
`codebase-design` criteria informed state ownership, ordering and seam checks;
the `decision-records` criteria informed the affected ADR-set check. The
configuration and admission records still own distinct decisions. These
findings need no additional ADR or test seam.

The count checks included current-run and adopted VMs, `Gone`, `Other`, full
match and missing identities, stopped metas after a Mac restart, a VM that
disappears between `tart list` and `tart get`, and other tart failures. The
general failed-observation rule covers a failed `tart get`; a missing VM is
explicitly excluded. No further OS-rule finding is supported by this read.

Before the first answer, a start without `--wait` and one with `--wait=0` both
fail their sole decision; the new startup guard forbids admission. A golden
creation as the sole waiter still causes the 3 s tick, so a later successful
answer reaches another admission decision. The zero-slot rules refuse both
macOS starts and golden hold requests. No additional first-answer progress
issue was established. The `refused_by` table omits the new unavailable-count
cause from its short definition, but the explicit startup rule and remedy
already determine the result; this is not a separate material finding.

For an answered running-VM recovery decision, row 3 now removes the sidecars
for `Gone`, `Other` and no identity, and keeps them only for a full match of
the meta's own main process. Its result and `removed` table close the legacy
exception that k8 identified. F1 concerns the unresolved fourth row instead.
The ordinary no-record adoption hazard, explicit replacement by `--id`, orphan
occupancy outside `macos_slots`, an unbounded golden wait while tart fails,
and a golden boot beside unseen occupancy are accepted trade-offs, not new
findings here.

Source reads followed the adoption decision and scan, the app stop route,
`stop_app`, `stop_proven`, `stop_without_signal`, `TartRunner::prepare_in` and
`stop_in`, the tart meta write, process identity reads and detached-child
reaping. The graph CLI failure described above prevented a project/generation
check and `check_index_coverage`. No graph completeness or repo-wide claim is
made from those source reads.

The repository has `Taskfile.yml`. This inspection-only review ran no test,
build, lint or format command, and changed no production or test code. It ran
no formal model and spawned no reviewer. The cases above are prose
counterexamples supported by source inspection, not execution results. The
paired integration owns triage, fixes and subsequent verification.
