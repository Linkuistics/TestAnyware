# admission-queue-k11

**Integrates:** admission-queue-k10

## Goal

Triage the committed findings of `admission-queue-k10` and apply supported
corrections to the admission design before planning consumes it.

## Context

- Read `admission-queue-k10` from its own commit. Its task file holds the
  findings, cases, source evidence and inspection limits.
- The reviewed producer is `admission-queue-k9`, commit `91c79e95`.
- The root brief and the decision log of `plan-k1` hold the agreed requirements
  and test seams. The design consists of the queue spec, affected app-spec
  passages, decision records, CLI contract amendment and glossary.

## Done when

- Every finding has a reasoned disposition, including rejected findings and
  accepted trade-offs.
- Supported corrections agree across the affected design documents, contract
  and glossary.
- The design is ready for `admission-increments-k3`, or the tree names further
  design work that must precede planning.

## Notes

This integrates a design review. The findings are input to triage, rather than
a work list to apply without checking. The review changed no production or
test code and ran no test, build, lint or format command. Its counterexamples
come from prose and source inspection. Read its inspection limits before
deciding what the evidence establishes.

## Decisions (running log)

**The findings come from the review's commit `9ac3292`, and its line numbers
still hold.** That commit changed only files under `.grove/`. No document or
source file changed after `91c79e95`. So each cited line is the line that the
reviewer read.

**F1 is a real issue.** The case holds in the code. `decide` returns `Stopped`
for a meta whose main process is gone. The scan of the app leaves the sidecars
of that VM, and its detail advises `vm stop`.

The stop route calls `stop_app`, which passes `Stopped` to `stop_proven`. That function refuses only `Doubt`. For
a tart guest it calls `TartRunner::stop_in` with pid 0, and `stop_in` removes
each VM of that name. So the fourth row lets a stale meta remove a person's VM,
which the third row protects when tart answers.

The fix applies the check of the meta in the fourth row too. That check reads
the process only, and it needs no answer from tart. So the app keeps the spec
and the meta only when the meta's own `tart run` matches in full.

The app keeps the VM and the record, and the next start of the app decides again, as for a
removal that fails. The start has `removed` false. After the socket opens, no
unproven sidecar of that id remains, and `vm stop` of the id fails with
`VM_NOT_FOUND`. The same rule removes an older meta, as the review asks.

The fix loses nothing that the later decision needs. That decision keeps a meta
only when its `tart run` matches in full, and the fix keeps such a meta. The
cost is a clone that stays on disk until the next start of the app. Today a
clone of an interrupted start stays until a stop or a new start of its id.

I rejected a second decision while the app serves, at the next answer of tart
or before a stop of the id. It would add a decision path that runs beside
the starts and the stops of the same id. The rule of this leaf needs no such
path.

**F2 is a real issue.** The case holds. An exit is a fact about one process, and
the kept answer names a VM by its name only. The reaper of a `tart run` runs on
its own thread, so no rule orders the exit that the app learns with an answer
of tart. When the exit comes before the answer and the app learns of it after
the answer, the subtraction removes a replacement that the answer counted.

A check of the process after each answer cannot prove the order either. The
probe reads the start time with `proc_pidinfo`. I could not establish from the
source whether that call reports a zombie child as live. Only the reaper proves
the exit, and the reaper has no order with the observations.

The fix lets only a newer answer of tart remove a VM from the kept answer. The
stop of the app gives such an answer, because it succeeds only when tart reports
the VM gone. That report is newer than the kept answer when the stop began after
tart answered. The exit of a `tart run` still ends the hold of its start. It
removes no VM from the kept answer, so the VM of that name counts as seen until
tart answers again. In the case of F2, the replacement and the golden creation
fill both slots, and the start waits.

The cost is a count that stays too high for a guest of the app that stops by
itself while tart does not answer. The open risk "A tart that does not answer"
already states that cost for a seen guest. The spec now names this case too.

An identity of each instance of a VM would remove the cost. The lock of tart is
an fcntl lock (ADR-0022), and such a lock can name the pid of its holder. But a
read of that lock reads the storage of tart. The design rejected that read before, so it stays out.

**One reviewer read the two new rules, and its seven findings are triaged
here.** The reviewer got the passages, a contract of four items and the code to
check. It got no decision of this log. It found nothing false in `removed`, or in
a legacy meta of the fourth row. It found nothing false in a quit or a second
crash while a record stays, or in stops that overlap observations.

- **R1, a stop that succeeds does not prove that tart reported the VM gone, is
  real.** The code shows it. `vm_exists_in` returns false when `tart list`
  fails. So `stop_in` succeeds when its last `tart list` fails, and a stop by
  name sends no signal. The fix removes a VM from the kept answer only when the
  last `tart list` of the stop answers and does not name the VM. The spec names
  the gap in today's code, so the increment that builds the count closes it.
- **R2, the fourth row waits for the next start of the app, is real.** The
  wait was older than this leaf, and I chose to keep it. A person's VM that ran
  at the start of the app can stop before the next start. Then the second row
  removes it by name with `tart delete`, although the third row would have kept it. So I reverse
  my earlier decision against a second decision while the app serves. After
  each observation that tart answers, the app decides again for each start
  record that stays, under the gate of its VM id. The first row cannot hold
  then, because a process that did not match cannot match later.
- **R3, a kept meta loses its proof when its `tart run` exits later, is the
  trade-off that k7 accepted.** `vm stop` of such a meta still removes a VM of
  that name. That is the hazard of adoption for every stale meta, not a gap of
  a start record. The spec did not state it as an open risk, and now it does.
- **R4, the wording of the kept answer is not precise, is real.** The
  `unanswered_since` row said "removed after that answer", and the rule said
  "began after the answer". Neither named the time of an answer. The fix orders
  by the calls to tart: the last `tart list` of the stop must start after the
  last call of the observation returns. The read of a stop is not an
  observation. It changes neither `observed_at` nor `unanswered_since`, and it
  is not the first answer of a run.
- **R5, the exit of an earlier run can end the hold of a later run, is real and
  small.** A start that the framework refused runs `tart run` again on its
  clone. So the exit now ends the hold of the admission that started that
  `tart run`.
- **R6, a start record of QEMU can fall under the rows of tart, is real and
  small.** The sentence about QEMU came after the table and did not take
  precedence. With the new fourth row, that reading removes the sidecars of a
  live QEMU guest. The spec now says that the table decides for a start on tart
  only, and that a start on QEMU never waits for tart.
- **R7, a `tart get` that fails can mean a VM that has gone or a tart that
  fails, is a contract stated unclearly.** The fix reads `tart list` again after
  a failed `tart get`. The VM does not count when that list answers and does
  not report it running. Otherwise the observation fails.

**R2 is a substantive fix, so this leaf cuts a `review-design` leaf before
planning.** This session spent its one reviewer. The retry adds a decision path
that runs while the app serves, and no reviewer read it. The other fixes state
a condition more precisely, or they state an accepted trade-off.

**ADR-0022 and the contract amendment need no change.** Neither states a rule
of the kept answer or of the fourth row. The glossary entry *Start record* and
the start record in the app's spec now state the retry. A record stays until
the app decides for it, once tart answers.
