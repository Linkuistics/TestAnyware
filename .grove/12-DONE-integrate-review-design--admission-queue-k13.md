# admission-queue-k13

**Integrates:** admission-queue-k12

## Goal

Triage the committed findings of `admission-queue-k12` and apply supported
corrections to the admission design before planning consumes it.

## Context

- Read `admission-queue-k12` from its own commit. Its task file holds the
  findings, cases, source boundaries and inspection limits.
- The reviewed producer is `admission-queue-k11`, commit `2be43a96`.
- The root brief and `plan-k1` hold the agreed requirements and test seams.
  The design consists of the queue spec, affected app-spec passages,
  decision records, CLI contract amendment and glossary.

## Done when

- Every finding has a reasoned disposition, including rejected findings and
  accepted trade-offs.
- Supported corrections agree across the affected design documents,
  contract and glossary.
- The design is ready for `admission-increments-k3`, or the tree names
  further design work that must precede planning.

## Notes

This integrates an inspection-only design review. Its findings are input to
triage, rather than a work list to apply without checking. The review ran no
test, build, lint or format command and changed no production or test code.
Its cases are counterexamples to the prose, supported by source inspection.
The integration owns every fix and its verification.

## Decisions (running log)

**The findings come from the review's commit `14657c50`, and its line numbers
still hold.** That commit changed only files under `.grove/`. No file outside
`.grove/` changed after `2be43a96`. So each cited line is the line that the
reviewer read.

**F1 is a real issue.** The retry uses "the same function, with the same bound
of 5 s", so it asks tart again after the observation. When that call fails, the
fourth row keeps the record. A later answer that shows the VM stopped then gives
the second row, and the app removes the person's VM by name. The open risk
admits this only while the app is down or before tart answers. Here the app saw
the VM running after its start.

**The fix for F1 takes the state of the VM from the list of the observation.**
When that list reports the VM running, the third row decides at once, and the
app asks tart nothing more. The second row removes a VM by name, so it still
needs a newer read under the gate. A stale "running" can only keep a VM, and
the third row already accepts that cost. The function never gives the second
row from the list of the observation.

I take every `tart list` of an observation as the trigger, not only an
observation that answers in full. A list can report the VM running while a
later `tart get` of another VM fails the observation. That list is the same
evidence, and discarding it reopens the case of F1. The record still waits for
a list that answers, as before.

**The triage found a gap beside F1, and the fix closes it.** The app observes
only for an event of the queue, a tick while a waiter waits, or `vm slots`. So
an idle app can keep a fourth-row record for hours after tart recovers. The
person's VM can stop meanwhile, and the next answer removes it. That defeats
the purpose of the retry that k11 added for R2. So the app now observes at
least every 3 s while such a record stays.

**F2 is a real issue.** The case holds in the prose. Nothing orders two
observations, and the time of each is the return of its last call. So an
observation whose `tart list` is older can finish later and replace a newer
answer. In the case, it removes a visible guest from the count, and a start
takes the slot of a golden creation. The spec states that the app keeps such holders to protect
golden creation, and this case defeats that.

**The fix for F2 is one observer that makes one observation at a time.** An
observation starts only after the one before it ended. So each answer comes from
a newer `tart list` than the one before it, and the time rule stays true. A
request that comes during an observation waits for the next one, and the
requests that wait together share it. So each decision reads tart after its
event, and concurrent `vm slots` requests cost one observation.

I rejected a rule that discards an answer whose `tart list` is older than the
kept one. It keeps observations concurrent, but it orders by a second time, the
start of the list. Then `observed_at`, the stop read and the order would use two
clocks. A single flight needs one rule. Its cost is a slow observation that
delays the next one by 5 s at most.

**A newer answer replaces the last answer whole, with each removal of a stop.**
The read of a stop can overlap an observation. When that observation reports
the VM running, its list started before the read of the stop. The VM then
counts until the next answer. That is one observation of a count that is too
high, and it frees the slot of no live guest.

**ADR-0022, the contract amendment, the glossary and the app's spec need no
change.** None of them states a rule of the order of observations or of the
retry. The glossary entry *Start record* and the app's spec say that a record
stays until the app decides for it, and that stays true. One observer is not an
ADR. It is cheap to reverse, and its one rejected option is in this log.

**One reviewer read the fixes, and its four items are triaged here.** It got
the passages, the contract of the root brief and six claims, but no decision of
this log. It found that claims 1 and 3 hold.

- **R1, the retry can stop the whole queue, is real.** The spec did not say who
  runs the retry or whether it waits for the gate. A start keeps its gate permit
  for its whole run (`start.rs:192`), its wait in the queue included. An
  observer that waits for that gate blocks each later observation, and so each
  decision. The single observer made this certain. The fix lets the observer run
  each retry after it publishes an answer. It tries the gate, never waits, and
  holds no lock of the queue meanwhile.
- **R2, two retries of one record can overlap, is real.** With a retry on each
  thread, an older retry can take the gate after a newer list saw the VM
  running. Its own read can then see the VM stopped and remove it. The fix runs
  every retry on the observer, in the order of the lists. The app also notes in
  memory each list that reports the VM running. The third row then decides at
  the first retry that gets the gate, whatever later lists report.
- **R3, "that costs one observation" is false, is real and small.** When tart
  stops answering after that answer, the VM counts for the whole failure. The
  open risk "A tart that does not answer" already admits that cost. The
  sentence now says only that the count can be too high.
- **R4, two readings of the bound of 5 s, is a contract stated unclearly.**
  Under a bound for the whole observation, many running guests can fail every
  observation, and the app then admits nothing. The bound now applies to each
  call.
- **R5, two readings of the names in a golden hold, is a contract stated
  unclearly.** An IPSW base build boots `<base>-building` and renames it later
  (`ipsw_base.rs:281`). Golden creation never boots the finished base
  (`golden.rs:345`). So the hold names each VM that golden creation boots: the
  setup VM, and the `-building` VM of a base build.

**R1 and R2 need fixes that are not mechanical, so this leaf cuts a
`review-design` leaf before planning.** This session spent its one reviewer,
and no reviewer read the fixes for R1 and R2. The other fixes state a bound, a
cost or a name more precisely.
