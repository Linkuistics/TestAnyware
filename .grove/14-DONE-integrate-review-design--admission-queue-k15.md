# admission-queue-k15

**Integrates:** admission-queue-k14

## Goal

Triage the committed finding of `admission-queue-k14` and settle the admission
design's deadline policy before planning consumes it.

## Context

- Read `admission-queue-k14` from its own commit. Its task file holds F1, the
  counterexample, supported rules and inspection limits.
- The reviewed producer is `admission-queue-k13`, commit `f90ba1dd`.
- The root brief and `plan-k1` hold the agreed requirements and test seams.
- `docs/specs/admission-queue.md`: the single observer and per-call bound in
  "The count", and the events and expiry order of "The admission decision".
  Also "The wait and its end", the serial recovery batch, the protocol seam and
  risks.
- The CLI contract retains a bounded `--wait` and an immediate refusal without
  it. The queue, app spec, glossary and decision records must remain coherent.

## Done when

- F1 has a reasoned disposition grounded in the committed review and current
  design. Supported corrections are applied across the affected documents.
- The design states how expiry, the last admission decision and an immediate
  refusal proceed while an observation or recovery batch is busy. None of them
  frees the slot of a live guest.
- The test seams cover slow successful calls and a deadline during an
  observation, including a slot that frees only after the deadline.
- The design is ready for `admission-increments-k3`, or the tree names further
  design work that must precede planning.

## Notes

This integrates an inspection-only review. F1 is a counterexample to the prose,
not an executed test. The review changed no production or test code and ran no
test, build, lint, format command or formal model. This session owns triage,
every fix and verification. Keep the human's requested focus on failures worse
than the design's stated risks. The human rejected a Quint model.

## Decisions (running log)

**The finding comes from the review's commit `f2a3b390`, and its line numbers
still hold.** That commit changed only files under `.grove/`. No file outside
`.grove/` changed after `f90ba1dd`, so each cited line is the line that the
reviewer read.

**F1 is a real issue.** Each decision waits for an observation that starts
after its event, and a deadline is one of those events. The bound of 5 s holds
for each call, and an observation makes one `tart get` for each running guest.
So a deadline can wait for the observation in progress, then for the retries of
start records, and then for a whole new observation. No stated bound is
independent of the number of guests. The function admits before it expires, so
a late decision can admit a waiter whose deadline passed long ago.

That contradicts the bound of `--wait` in the CLI contract, and the last
decision at the deadline in the spec. The risk of a tart that does not answer
admits a refusal at a deadline. It admits no deadline that waits for answers.

**The fix: the decision at a deadline waits for no observation.** At the
deadline of a waiter, the app decides at once on the last answer, as it decides
after an observation that failed. The count then keeps each seen holder of that
answer, so it is as conservative as while tart fails. A waiter that this
decision does not admit fails, so no later decision can admit it.

I rejected a bound for the whole observation. k13 rejected it for R4, because
many running guests would then fail every observation. Reads of the OS at the
same time do not fix F1 alone. A deadline still waits for the observation in
progress, and then for the next one. I also rejected a deadline that waits for
the observation in progress up to a bound. It needs a new constant, and the
wait would still exceed `--wait`.

**The last answer needs a rule for its age, and the rule closes a gap beside
F1.** An idle app makes no observation, so the last answer can be hours old. A
short wait whose first observation is slow then gets its last decision on that
answer. Suppose a golden creation holds a slot between its boots, and a person
started a macOS VM after that answer. The app admits the start, the framework
accepts it, and the next boot of golden creation fails. Without the fix, the
start would wait for an answer that shows the person's VM.

So the app now admits a start or a golden creation only on an answer whose
observation started after its join. This is the general case of "Before the
first answer", which it replaces. The rule orders by observations, so it adds
no second clock, which k13 rejected. It also closes the same gap after a
failed observation. Before, a start that joined while tart failed could get an
admission on an answer from before its join.

**A start without `--wait` gets a refusal without an observation when the
refusal needs no answer of tart.** That holds when the queue holds a waiter,
when contention blocks an admission, or when the holders that the app admitted
fill the count. The app decides these on its state at the arrival. A newer
answer could admit the head and empty the queue, but the start must never pass
a waiter, as ADR-0022 states. No answer of tart ends the holds of the third
case. Otherwise its decision waits for an observation that starts after its
join, as before.

**The age rule numbers the observations.** At its join, a start or a golden
creation notes the number of the next observation. An answer is newer than the
join when its observation has that number or a later one. The decision gets
that number as one more input. I rejected a decision at
once on the last answer. An idle app can keep an answer for hours, and a guest
that stopped meanwhile would refuse a start while slots are free.

**`--wait=0` takes the path of a start without `--wait`.** A deadline at the
join would decide at once, and no answer is newer than the join at that moment.
So under the new rules, a deadline at the join would refuse every such start.

**The fix stays in the queue spec.** ADR-0022, the glossary, the app's spec and
the CLI amendment state no rule of the observer or of a deadline. The contract
keeps "`--wait=<seconds>` sets the bound", and the fix makes it true. The
deadline rule is cheap to reverse and has no rival that needs a record, so it is
not an ADR.

**One reviewer read the fixes, and its four items are triaged here.** It got
the spec, the done-when of the root brief, the CLI amendments, ADR-0022 and the
case of F1. It got no decision of this log. It found no case beyond the open
risks for the deadline rule, the age rule, contention, the pause, the stop read
or the numbering.

- **R1, a start without `--wait` still waits for slow calls, is a real
  trade-off.** A full count of seen or adopted guests needs a count, and the
  count needs tart. So the decision can wait for the observation in progress,
  the retries of start records and one more observation. "At once" in the
  contract and the brief means
  that the start never waits for a slot, and that stays true. Two wording
  faults are real: "as it delays the clone" understates the cost, and the open
  risks named no delayed refusal. The spec now states both, with the case
  where a short wait fails while `--wait=0` gets its slot later.
- **I rejected a decision of such a start on the observation in progress.**
  It halves the worst case, but it admits on a list older than the join. The
  age rule exists to stop that.
- **R2, the state diagram puts a start out of reach of its deadline, is real.**
  The text makes a start with a wait a waiter from its join, and the diagram
  kept it in `Preparing` until its first decision. Now a start with a wait goes
  to `Waiting` at its join, under the lock, so the join orders it. A start
  without `--wait`, or with `--wait=0`, never becomes a waiter. Its one decision
  goes from `Preparing`, so no expiry step can fail it on an older answer.
- **R3a and R3b are contracts stated unclearly.** "The app observes again before
  each decision to admit" now names its two exceptions. The sentence on a start
  without `--wait` before the first answer now says that its observation failed.

**These fixes need no further review leaf.** They bring the diagram and two
sentences into line with the rules that the reviewer read, and they state a
cost. None adds a mechanism. The reviewer found the deadline rule and the age
rule sound, so the design goes to `admission-increments-k3`.
