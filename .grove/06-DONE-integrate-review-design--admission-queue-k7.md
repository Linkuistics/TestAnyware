# admission-queue-k7

**Integrates:** admission-queue-k6

## Goal

Triage the findings in the committed review `admission-queue-k6`, and apply
the supported corrections to the admission design before planning consumes it.

## Context

- `admission-queue-k6`: its committed task file holds the findings, cases,
  source evidence and inspection limits. Read its commit before deciding.
- The reviewed producer is `admission-queue-k5`, commit `d66d5cbf`.
- The root brief and `plan-k1` hold the agreed requirements and test seams.

## Done when

- Every finding has a reasoned disposition, including any rejected finding or
  accepted trade-off.
- Supported corrections agree across the spec set, ADR set, contract and
  glossary wherever they change a claim.
- The design is ready for `admission-increments-k3`, or the tree names the
  additional design work that must precede planning.

## Notes

This is integration of a design review. The review ran no tests, builds, lint
or formatting, and changed no production or test code. Its findings are input
to triage, not a work list to complete. Read the review's coverage and limits.

## Decisions (running log)

**The findings come from the review's commit `8e32a29`, and its line numbers
still hold.** That commit changed only files under `.grove/`. No document or
source file changed after `d66d5cbf`. So each cited line is the line that the
reviewer read.

**F1 is a real trade-off, and the spec stated it as a guarantee.** The case
holds. An admitted golden creation is never unproven. So in contention the app
can admit a start beside it before its first boot, or between two boots. Beside
a guest that the app cannot see, that start can take the one free slot. The
next boot of golden creation then fails after 30 s.

The app cannot see when a boot of golden creation meets the framework. So no
rule of the count can close this case cheaply:

- A golden creation that is unproven for its whole hold reverses R6 of k5.
  Then a refusal while a golden creation holds blocks each waiter for the whole
  golden creation. The open risk "A slot that frees late" makes such a refusal
  common after a release. A waiter with 600 s would then fail while a slot is
  free.
- The case also happens outside contention. The first try of a start beside a
  golden creation can take the slot before any refusal. So a rule for
  contention alone cannot protect golden creation.
- A full fix needs evidence of each boot. Golden creation could report its
  boots over its hold, or the app could trust the lock of tart as such
  evidence. Each is a new protocol or a new claim about tart, and no reviewer
  has read it. So it is redesign, and not the work of this session.

The cost is small. The case needs a guest that the app cannot see: a guest of
another user, of another tool, or the stuck count of the framework. Golden
creation then fails with `VM_LIMIT_REACHED` and `refused_by` `framework`. It
removes its setup VM, and a person runs it again. Golden creation fails in the
same way today, without the app.

So the fix narrows each claim to a true count, and it records the case as an
open risk. "Out of scope" names the evidence of each boot, with the reason.
Golden creation stays never unproven.

**F2 is a real issue: an acceptance of an earlier try ends contention too
soon.** The case holds. B was admitted before the refusal of A. Its acceptance
proves only that one slot was free when B tried. It proves nothing about the
retry of A, which still waits.

The fix ends contention only at the acceptance of a start that the app admitted
in contention. In contention the app admits the head alone, and only while no
admitted start is unproven. So at that acceptance no refused start waits ahead
of a later waiter. The second end stays: contention also ends when no start
waits and none is unproven.

The fix costs little. After an acceptance of an earlier try, the app still
tries one start at a time, until one of those tries gets an acceptance. Each
such try costs one boot to the VNC URL, a few seconds.

**F3 is a real issue: the kept meta gives the app authority over a VM that it
cannot prove.** The case holds in the code. `decide` gives `Stopped` for a gone
main process. `stop_proven` refuses only `Doubt`, and `stop_in` then removes
the VM by name. The adoption report of the app also names `vm stop` for a gone
process.

The meta and the start record coexist only after the start wrote its sidecars.
Those sidecars name the recorded `tart run`. So in row 3 the meta always names
a process that is gone, and it describes no VM that the app can prove.

The fix removes the spec and meta sidecars with the record in row 3, and it
keeps the VM. The kept VM then counts as a seen holder. `vm stop` of its id
fails with `VM_NOT_FOUND`, because the stop needs a meta. Only `tart stop` and
`tart delete` remove it, as the report states.

**F4 is a real issue: the table leaves one case undecided.** The case holds.
The keeper is a separate process, and it exits only when its next connect
fails (ADR-0018). So for a short time it can match while the main process is
gone.

The fix makes rows 2 and 3 depend on the recorded `tart run` alone. A keeper
never proves the VM. In rows 2 and 3 the app sends no signal, and a keeper of
the start ends by itself when its port refuses. The flowchart and the glossary
now use the same predicate.

**A related hazard of adoption stays as it is, because this design did not
make it.** Any meta can outlive its `tart run`, with no start record and no
crash of the app. When tart then reports a VM of that name as running, the
count holds it as own. `vm stop` also removes it by name, as the adoption
report advises.

The name is the id of a TestAnyware start. Only a person who runs a new VM
under that id meets the hazard. The adoption of the app's spec owns that
path, and the queue does not change it. So the fix stays in row 3, where the
spec promises to keep the VM, and the final report names the hazard to the
human.

**One reviewer read the revised spec, and its eight findings are triaged
here.** The reviewer got the spec and the requirements, and no decision of
this log. It tried the sequence of F2, and contention held. So the fix of F2
stands. It also found that the spec states the exception of golden creation.

- **R1, the clone step removes a VM of the same id, is real in two parts.**
  `prepare_in` clears the sidecars of the id and removes each tart VM of that
  name. So a `vm start --id` removes an external VM of that id, as it does
  today. The command surface must keep that meaning, so the spec now states it
  as the one exception. My sentence "Only `tart stop` and `tart delete` remove
  it" was false, and it now names that start.
- The second part is a fault of my fix of F3. A second start of the same id
  writes its record before its clone step. A crash in that gap leaves the VM
  of the first start, and its meta proves it. Row 3 would remove that meta. So
  row 3 now keeps the sidecars when adoption adopts the VM by its meta.
- **R2, "a start waits" leaves out a golden creation, is a contract stated
  unclearly.** The rules of the tick, the observation, the decision, the end
  of contention and a start without `--wait` mean every waiter. Each one now
  says "waiter", and so do the glossary, the contract and ADR-0022.
- **R3, a slow tart drops the seen holders, is real.** The count then allows a
  try that a full count forbids, and it can pass `macos_slots`. The fix keeps
  the last observation that tart answered. Before the first answer, the app
  counts each VM of a meta as own.
- **R4, `refused_by` `app` has two meanings, is a contract stated unclearly.**
  At a deadline in contention, the app refuses although the count allows. The
  member now covers both causes. The remedy of such a refusal names the two
  causes of a refusal by the framework, as "Open risks" already promised.
- **R5, the pause fails a later waiter beside free slots, is a real
  trade-off.** Only a waiter with a wait under 3 s meets it. The pause exists
  so that a refusal does not repeat at once. The spec now states the case.
- **R6, the diagrams and the event list disagree with the text, is real and
  small.** The state diagram gains the missing failures. The event list gains a
  leave, and the tests gain the end of a pause and a tick.
- **R7, a golden creation that waits while the app ends, is real.** No rule
  covered it. It now fails as a waiting start does, before it changes anything.
- **R8, the app keeps an OS answer for ever, is real and small.** The app now
  forgets the answer when tart no longer lists the name.

**The fixes of R1, R3 and R4 earn a new `review-design` leaf before planning.**
They change the start-record table, the count and the error details again, and
no test covers them yet. This session spent its one reviewer. So it cuts
`review-design` for the rules that this leaf changed, and the leaf names each
doubt.
