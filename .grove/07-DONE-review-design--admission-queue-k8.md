# admission-queue-k8

**Reviews:** admission-queue-k7

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k7`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k7`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k6` and of one reviewer that
  k7 ran.
- `docs/specs/admission-queue.md`: "The count", "The admission decision", "A
  refusal by the framework" and "A start without `--wait`". Also "The wait and
  its end", "Golden creation", "A quit, a crash and a restart" and "External
  occupancy". Also the details of `VM_LIMIT_REACHED`, its requirements, its
  test seams, its open risks and its "Out of scope".
- `CONTEXT.md`: *Contention*, *Unproven* and *Start record*. ADR-0022, and the
  amendment of 2026-10-25 in `docs/architecture/cli-design-contract.md`.
- The code of today: `TartRunner::prepare_in`, the adoption decision,
  `stop_proven` and the meta write of a tart start.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model of the queue. So these rules rest on prose and
on the seam of pure functions, and no test exists for them yet.

**The end of contention.** k7 ends contention only at an acceptance of a start
that the app admitted in contention. Contention also ends when the queue holds
no waiter and no start is unproven. Look for an order of events that holds
contention with no end. Look for one that lets two starts try together while it
holds. Look also for one that blocks a start that the limit does not block.

**The waiter.** k7 changed "a start waits" to "the queue holds a waiter" in five
rules. They are the tick, the observation, the decision, the end of contention
and a start without `--wait`. Check each rule with a golden creation as the
only waiter. Check also a golden creation that waits while the app quits or
crashes.

**The count when tart is slow.** The app now keeps its last observation that
tart answered. Before the first answer, it counts each VM of a meta as own.
Check this against "a full count is a fact", `macos_slots` and the hold of
golden creation. Check also a stale observation that hides a free slot.

**The start record.** Row 3 now removes the spec and meta sidecars, unless
adoption adopts the VM by its meta. Check each outcome of adoption, and a
second start of the same id that crashes before its clone step. Check the
report of `removed` for each case.

**The error details.** `refused_by` `app` now covers a full count and a try
that contention held back. Check the remedy, `limit` and `holders` for each
cause.

**The trade-offs that k7 accepted.** A golden creation beside a guest that the
app cannot see can fail. A waiter behind the head can fail at a deadline during
the pause. A `vm start --id` removes a VM of that id, as today. Report one of
these only when the case is worse than the spec admits.

**The sets.** The spec, ADR-0022, the contract's amendment of 2026-10-25 and
`CONTEXT.md` must agree with each other.

## Decisions (running log)

**The review uses the installed Grove 22.6.0.** The first bootstrap command
selected the obsolete Cargo installation of Grove 18.2.0 and refused the tree
for a missing `FORMAT`. The matching installed 22.6.0 resolved this leaf
without a format witness. At the human's request, this session uninstalled the
obsolete Cargo package. Both ordinary Grove commands now report 22.6.0. The
review resumed from the unchanged producer commit `c7e15f04`.

**The sidecar exception needs proof stronger than adoption alone.** Adoption
also serves a meta with no process identity as `Legacy`. That outcome cannot
prove that a running VM belongs to an earlier start of the same id. The new
exception in row 3 therefore leaves an unproven VM subject to `vm stop`.
Finding F1 records the case; integration owns its disposition.

**A listed name does not identify one VM for the OS cache.** A VM can be
replaced between observations without its name disappearing from any answer
of `tart list`. The revised cache can then retain the old OS indefinitely.
Finding F2 records the resulting false full count.

**A retained observation needs a rule for a later confirmed release.** The
new timeout fallback can keep a deleted VM in the count after the app has
released its hold and removed its sidecars. No age bound or reconciliation
rule ends that hold without another successful tart observation. Finding F3
records the availability consequence, distinct from contention caused by an
unseen guest.

**The findings warrant integration before planning.** Each finding has a
passage and a concrete case. A paired `integrate-review-design` leaf will read
this review's committed findings and triage them, including the option to
accept a stated trade-off. It goes immediately before `admission-increments-k3`
so intervening design changes cannot silently move the cited passages.

## Findings

### F1 — High: legacy adoption does not prove the VM that row 3 preserves

**Passage.** `docs/specs/admission-queue.md:520` keeps the spec and meta when
adoption adopts the VM. Lines 536–541 infer that such a meta proves an earlier
start whose `tart run` still runs, and report `removed` true.

**Case.** An earlier start of id X left a meta with no process identity. A
second start writes its start record and crashes before its clone step clears
that meta. While the app is down, a person replaces the old VM with a running
VM under X. The second start's record names no matching `tart run`, so row 3
applies. Adoption nevertheless accepts the old meta as `Legacy`. The exception
keeps the sidecars, classifies the replacement as own, and leaves `vm stop X`
able to stop and delete it by name. The recovery report can also say `removed`
true even though the VM is unproven.

**Evidence.** `cli-rs/crates/testanyware-vm/src/adoption.rs:123` returns
`Legacy` for a meta with no identity without checking a process. The app puts
both `Adopt` and `Legacy` in its survivors
(`cli-rs/crates/testanyware-app/src/adoption.rs:107`). Its legacy stop calls
`stop_without_signal` (`cli-rs/crates/testanyware-vm/src/lifecycle.rs:608`),
which calls the tart stop by name at line 749. Sending no signal does not
protect the replacement VM. This is also an outcome for a newly written meta:
`read_identity` is optional (`identity.rs:39`), and the tart start writes that
optional value into its meta (`lifecycle.rs:450`).

This case has a start record and enters the newly changed exception. It is
distinct from k7's accepted adoption hazard for a meta with no start record.
It contradicts the passage's ownership proof and the promise that the kept VM
becomes external and cannot be stopped through the app.

**Smallest useful correction.** Make the preservation exception depend on a
positive match of the earlier meta's main process, rather than on membership
in adoption's survivors. State the result and the `removed` value for `Legacy`
as well as `Adopt`, `Stopped` and `Doubt`. If legacy compatibility intentionally
wins over the protection, record that exception explicitly. The existing pure
recovery decision and protocol seam can cover this sequence.

### F2 — Medium: replacement between observations defeats OS-cache invalidation

**Passage.** `docs/specs/admission-queue.md:135` asks for the OS once per new
name and keeps it while `tart list` continues to report that name.

**Case.** Two external macOS VMs X and Y run, and the app caches `darwin` for
both. Between two observations, a person replaces X with a Linux VM under the
same name. Every list answer still contains X and Y, and both now run. The app
therefore never invalidates X's OS, even though every subsequent observation
answers successfully. It counts two macOS guests and never tries the waiting
macOS start. In fact only Y holds a macOS slot. A bounded waiter expires, and
a golden creation can wait without end. The inverse replacement hides a
visible macOS guest from `vm slots` and leaves only the framework refusal to
detect it.

The absence of a name from an observation is not evidence that must occur
between two instances of that name. The revised rule therefore still permits
an indefinitely wrong count. It breaks the promises that Linux guests take no
slot and that the app counts visible external macOS occupancy. The current
start path also demonstrates that name reuse is supported:
`TartRunner::prepare_in` removes and reclones the same id
(`cli-rs/crates/testanyware-vm/src/tart.rs:573`).

**Smallest useful correction.** Give the OS observation a revalidation rule
that reaches replacement under a continuously listed name. Re-reading the OS
for running names is one sufficient policy; an instance-aware cache needs
evidence for its identity. Cover both OS changes at the existing observation
and count seams, with no intervening list answer in which X is absent.

### F3 — Medium: the timeout fallback can retain a confirmed released holder

**Passage.** `docs/specs/admission-queue.md:139` retains the last successful
observation until another answers. It gives neither an age bound nor a rule
for reconciling that observation with a subsequent confirmed release. Before
the first answer, it counts every VM of a meta as own.

**Case.** The last answer shows the app's macOS guests A and B running. The app
then stops A, confirms its `tart run` has exited, removes its sidecars and
releases its hold. Every later tart observation times out. The retained answer
still says A runs. Counting it as own retains the released hold; rebuilding
the classification from current sidecars instead makes it a seen holder. Both
readings keep the total at two. The waiting start never gets a try and expires,
although a slot freed through the app's own successful stop. A golden creation
as the only waiter can wait without end. The related first-answer case occurs
after a Mac restart: stopped VMs retain metas, and the fallback counts them
despite the claim at lines 569–575 that nothing holds a slot after that restart.

Retaining uncertain external occupancy can be a deliberate conservative
choice. It does not make a stale full count a current fact, or establish the
stated progress after a stop or restart. Unlike F2, this case needs repeated
observation failures; it is not a framework refusal or a stuck framework
count, and the existing risks and remedy do not describe it.

**Smallest useful correction.** Define how confirmed releases and process
checks reconcile with cached occupancy and with the first-answer fallback.
For occupancy that remains uncertain, state the availability policy and expose
the observation failure, rather than reporting stale holders as current
without explanation. If refusing until tart answers is intentional, disclose
the resulting waiter failures and unbounded golden wait. Cover the last full
answer followed by a release and repeated timeouts, and the stopped-meta
startup case, through the existing pure count and protocol seams.

## Inspection coverage and limits

The reviewed producer is `admission-queue-k7`, commit `c7e15f04`. The working
copy was empty on entry. This review read its committed changes and decision
log against the requirements log of `plan-k1`, the root brief, the queue spec,
the affected app-spec passages, the contract amendment, the glossary and the
cited decision records. The configuration and admission records still own
distinct decisions; these findings require no additional ADR or test seam.
The `codebase-design` criteria informed the ownership, state and seam checks,
and `decision-records` informed the check of the affected ADR set.

The contention cases inspected included the acceptance of a pre-refusal try,
two refused tries, an unproven try that ends in another way, cancellation of
the last waiter, and a golden creation as the only waiter. No further
actionable contention issue was established in that bounded read. The accepted
risks of a golden boot beside unseen occupancy, a later waiter meeting its
deadline during the pause, and explicit replacement through `vm start --id`
were not reported again.

Source inspection followed `TartRunner::prepare_in`, the tart meta write,
`read_identity`, the adoption decision and scan, and the app's legacy and
proven stop paths. The graph CLI refused even `list_projects` because a
pre-coordination or unverified generation was active. It supplied no project,
generation or coverage evidence. The source evidence above comes from direct
reads and targeted searches instead; this is not an exhaustive codebase claim.

This review changed no production or test code, ran no test, build, lint or
format command, and introduced no subagent reviewer or formal model. The
event sequences are prose counterexamples, not execution results. The paired
integration leaf will triage them and owns any fix and subsequent verification.
