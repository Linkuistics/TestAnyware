# admission-queue-k12

**Reviews:** admission-queue-k11

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k11`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k11`, and the decision log in its task file.
  The log triages the findings of `admission-queue-k10` and of one reviewer
  that k11 ran.
- `docs/specs/admission-queue.md`: "The count" with "When tart does not answer"
  and "Before the first answer". Also the start record of "A quit, a crash and a
  restart", with both of its tables, "A record that tart did not decide" and the
  flowchart. Also the `vm slots` table, the scenarios of "A crash or a restart
  keeps the count true", the test seams and the open risks.
- `CONTEXT.md`: *Start record* and *Holder*. The start record in "Lifecycle and
  adoption" of `docs/specs/testanyware-app.md`.
- The code of today: `vm_exists_in`, `TartRunner::stop_in` and `prepare_in`,
  `stop_app`, `stop_proven`, `stop_without_signal`, the adoption decision and
  its scan, the stop route, and the reaping of detached children.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

This is the fifth review of the design, after k4, k6, k8 and k10. The human
asked for focus, so report a case only when it is worse than the spec admits.
The human rejected a Quint model of the queue, and no test exists for these
rules yet.

**The retry of the fourth row.** When tart does not answer at the start of the
app, the app now decides again for the record after each observation that tart
answers. That decision runs while the app serves its socket, under the gate of
the VM id.

Check the retry beside a `vm stop` and a new `vm start` of the same id, a quit,
and a crash during the retry. Check a tart that answers an observation but not
the retry. The third row at the start of the app would keep some VMs.
Check that the second row of a retry never removes one of them.

**The read of a stop.** A stop now removes a VM from the kept answer only
through its last `tart list`. That list must answer without the VM. It must
also start after the last call of the observation returns.

Check that order against concurrent observations and stops. Check that the read of a stop changes neither `observed_at` nor
`unanswered_since`, and is never the first answer. Today `vm_exists_in` reads a
failed `tart list` as no VM. Check that the spec makes the implementer close
that gap.

**The exit.** The exit of a `tart run` now ends only the hold of the admission
that started it, and it removes no VM from the kept answer. Check a start that
the framework refused and that runs `tart run` again on its clone.

**A failed `tart get`.** The app now reads `tart list` again after a failed
`tart get`. Check what a `tart get` that keeps failing does to the count and to
the waiters.

**QEMU.** The table of the start record now decides for a start on tart only.
Check that no row can reach a start on QEMU.

**The trade-off that k11 made visible.** A meta that outlives its `tart run`
lets `vm stop` remove a VM of that name. k7 accepted that hazard of adoption.
Report it only when a case is worse than the new open risk states.

**The sets.** The spec, `CONTEXT.md`, the app's spec, ADR-0022 and the
amendment of 2026-10-25 in `docs/architecture/cli-design-contract.md` must agree
with each other. k11 changed neither ADR-0022 nor the contract.

## Decisions (running log)

**The review is anchored to `admission-queue-k11`, commit `2be43a96`.** The
working copy was empty, directly above that commit. I read its diff and its
decision log against the current design and the agreed requirements of
`plan-k1`. This is an inspection-only review; no test, build, lint or format
command runs here.

**A successful observation can fail to settle a retained start record.** The
new retry asks tart again after the observation. If that query times out, the
record retains authority to delete by name, although the observation already
showed an unproven running VM that the third row would preserve. A later retry
can delete that VM after it stops. F1 records this case, which occurs after
tart's first answer and differs from the accepted deletion risk before that
answer.

**The time of the final call does not order the occupancy lists.** Two
observations can overlap under the specified locking rule. An observation
whose initial list is older can finish its OS queries later, and become the
last answered observation. It can then discard a guest that the intervening
observation saw. F2 records the resulting undercount beside a golden creation.
This case needs neither a failed observation nor an invisible guest.

**Both findings warrant integration before planning.** Each has a concrete
failure beyond the spec's stated risks. The paired integration will read this
review from its own commit and triage the findings; this review applies no fix.

## Findings

### F1 — Medium: a retry can delete a VM after an observation showed it running

**Passage.** `docs/specs/admission-queue.md:623` starts a new recovery decision
after each answered observation and gives that decision its own 5 s bound.
The fourth row at line 577 keeps the record when that decision gets no answer.
The second row at line 575 later deletes a stopped VM by name. Nothing carries
the running-VM evidence of the triggering observation into that decision or
settles the record from that evidence.

**Case.** Let X be the clone named by an interrupted start record.

1. While the app is down, a person removes that clone and runs a different VM
   under X. The recorded `tart run` is gone.
2. At startup, tart does not answer. Row 4 keeps the record, removes the
   unproven sidecars, and leaves the person's running VM alone.
3. An observation now answers in full: `tart list` reports X running and
   `tart get` reports `darwin`. The app has its first answer, and X counts as
   external occupancy.
4. That observation triggers the retry, but the retry's own query times out.
   Row 4 keeps the record again. The same sequence can recur while X runs.
5. Later, the person shuts down X normally. A subsequent observation and
   retry answer with X stopped. Row 2 deletes the person's VM and its disk by
   name, although no person requested a stop, delete or new start of X.

Had the app used the answered running state in step 3 for the recovery
decision, row 3 would have kept X and removed the record. The later shutdown
would then give recovery no authority over its disk. The per-id gate does not
resolve this case: there is no concurrent Host CLI command for X.

This exceeds the risk at `docs/specs/admission-queue.md:1131`. That risk admits
deleting a replacement that is stopped while the app is down or before tart
answers after startup. Here the app has already observed the replacement
running, after startup, and only the extra query loses that protection. It
also differs from the stale-meta risk at line 1169: the sidecars are gone,
`vm stop` cannot act, and recovery deletes the disk automatically. A quit or
crash after step 4 leaves the same record available for row 2 at the next
startup.

**Source boundary.** These are proposed recovery rules, not implemented queue
behaviour. Today's `TartRunner::prepare_in` clears sidecars and replaces a
same-id VM only for an explicit new start
(`cli-rs/crates/testanyware-vm/src/tart.rs:562`). Today's stop route takes the
id gate and then requires a meta
(`cli-rs/crates/testanyware-app/src/stop_delete.rs:59`,
`cli-rs/crates/testanyware-vm/src/lifecycle.rs:615`). Neither path grants this
automatic deletion authority after the sidecars are gone.

**Smallest useful correction.** Specify how a successful observation that
shows this unresolved record's VM running settles the preservation decision,
even when a subsequent query fails. Apply the evidence to the same retained
record under the id gate, and preserve that outcome across a restart. A later
stopped answer must not restore deletion authority over a VM already
preserved. Extend the existing protocol seam with the trace above, including
a quit or crash between the first successful observation and the later
stopped answer. No new test seam or formal model is needed.

### F2 — Medium: final-call time can publish an older occupancy list as newer

**Passage.** `docs/specs/admission-queue.md:140` obtains the running names from
`tart list`, then obtains their OS with separate `tart get` calls. Lines 156
to 163 allow observations for admission events and `vm slots`, outside the
queue lock, and define an observation's time by its final call's return.
Lines 273 to 276 lock the admission decision only after its observation.
These rules do not order overlapping observations or reject a late result
whose running-name list is older than the published list.

**Case.** Use `macos_slots` 2, a golden creation G holding its slot between
boots, and a running tart Linux guest L, which takes no macOS slot. A macOS
start S waits. Each observation below finishes within 5 s.

1. S's admission event starts observation A. It reads a list containing only
   L. Its `tart get L` is slow.
2. A person starts macOS guest X with tart, outside the app. X keeps running.
3. Observation B, for `vm slots`, reads L and X. Its OS queries answer in
   full. It publishes X as a seen holder. X and G fill the app's count, so S
   must wait.
4. A's `tart get L` now returns `linux`. A has no macOS guest in its earlier
   list. Its final call returned after B's final call, so A is the last
   answered observation under the specified time rule. Publishing A removes
   X from the seen holders.
5. A's admission decision now counts G alone and admits S. S boots beside X,
   taking the second physical slot. G's next boot is refused for 30 s and
   golden creation fails.

The queue lock orders publication and admission, but it cannot make A's list
newer than B's. The stop-read rule at line 165 does not help: no stop and no
process-exit subtraction occurred. The same stale publication can precede
failed observations and remain the kept answer.

This is more than the accepted cost of failed observations: both A and B
answered. It also exceeds the golden-creation risk at line 1151, which needs
occupancy the app cannot see. X is a same-user tart VM that a completed
observation already counted. An older list erases known occupancy and defeats
the stated protection for golden creation beside a visible guest.

**Smallest useful correction.** Give observation collection and publication
one ordering rule that prevents an older running-name list from replacing a
newer one. One observation in flight is an option; rejecting superseded
collections is another. Keep tart outside the queue lock, and define the
interaction with the existing stop-read ordering and `observed_at` rule.
This coordination belongs with the observation state, so callers need not
reconstruct its ordering. Add the delayed-L trace to the existing protocol
seam and count tests, and assert that X remains counted and S waits.

## Inspection coverage and limits

The bounded review covered the producer's committed diff and decision log,
the root brief and `plan-k1` requirements, the queue spec, the affected
lifecycle and adoption passages of the app spec, the glossary, the CLI
contract amendment of 2026-10-25 and the decision records the brief cites.
The `codebase-design` criteria informed state ownership, timing and seam
checks. The `decision-records` criteria informed the affected ADR-set check.
The configuration and admission records still own separate decisions; these
findings establish no need for another ADR or another test seam.

The per-id gate orders a retry with a stop or a new start of that id. A new
start replaces the record before its clone, as specified. No separate finding
is supported merely by those operations sharing the gate. F1 remains when
there is no competing operation, and a crash can retain its deletion
authority. Row 4 now removes an unproven meta, including a legacy meta,
before the socket is served, closing the previous review's sidecar case.

The stop-read rule now requires a successful final list that omits the VM,
whose call starts after the observation's last call returns. An overlapping
stop cannot use an earlier or failed list as evidence for subtraction. A
stop's read changes neither observation timestamp and cannot supply the first
answer. `vm_exists_in` still maps a failed list to false
(`cli-rs/crates/testanyware-vm/src/tart.rs:261`), but the spec explicitly names
that implementation gap and forbids relying on it. This is an implementation
obligation already recorded, not another design finding. F2 concerns two
observations, rather than a stop's subtraction.

The exit rule ties the release to the admission that started the process,
which distinguishes retries on one clone. Detached children are independently
reaped (`cli-rs/crates/testanyware-vm/src/detached.rs:25` and its background
reaper). An old exit therefore cannot subtract a replacement from the kept
observation under the revised rule. A repeatedly failing `tart get` fails the
observation unless a new list shows that VM no longer running. That can keep
the count too high, or prevent the first admission, but the failed-observation
risk already admits that availability cost.

The recovery table now expressly applies to tart only. The QEMU branch asks
no tart question, so its sidecars cannot reach row 4 through that table. The
ordinary stale-meta stop hazard, explicit replacement by `--id`, orphan
occupancy outside `macos_slots`, an unbounded golden wait while tart fails
and a golden boot beside invisible occupancy remain accepted trade-offs.
No separate finding is made about them. The glossary, app spec, admission
decision record and CLI amendment agree on their affected responsibilities;
the detailed retry and observation gaps above belong to the queue spec.

Source inspection followed `vm_exists_in`, `TartRunner::prepare_in` and
`stop_in`, `stop_app`, `stop_proven`, `stop_without_signal`, the adoption
decision and scan, the stop route, process identity reads and detached-child
reaping. `codebase-memory-mcp cli list_projects` failed because a
pre-coordination or unverified CBM generation was active. No graph tools were
exposed by this session's tool inventory. I used direct source reads and
targeted `rg` discovery instead; no project/generation confirmation or
`check_index_coverage` was possible, and no graph-completeness or repo-wide
claim follows from these reads.

The repository has `Taskfile.yml`. This review ran no test, build, lint or
format command, changed no production or test code, ran no formal model and
spawned no reviewer. The cases are counterexamples to the prose, supported
by source inspection, rather than execution results. Both warrant a paired
integration before planning; that session owns triage, every fix and its
verification.
