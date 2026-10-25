# admission-queue-k5

**Integrates:** admission-queue-k4

## Goal

Triage the findings in the committed review `admission-queue-k4`, and apply the
supported corrections to the admission design before planning consumes it.

## Context

- `admission-queue-k4`: its committed task file holds the findings,
  counterexamples, source evidence and inspection limits.
- The reviewed producer is `admission-queue-k2`, commit
  `05bf97e6cd8bb05db00dacb8d9ea20386041df6e`.
- The root brief and `plan-k1` hold the agreed requirements and test seams.

## Done when

- Every finding of `admission-queue-k4` has a reasoned disposition under the
  integration procedure, including findings that are rejected or accepted as
  trade-offs.
- Supported corrections are reflected coherently in the spec set, ADR set,
  contract and glossary wherever the correction changes their claims.
- The design is ready for `admission-increments-k3`, or the tree names the
  additional design work that must precede planning.

## Notes

This is integration of a design review. The review ran no tests, builds, lint
or formatting, and changed no implementation or test code. Before you decide a
finding, read the evidence and the limits of the review. Its list is input to
triage, not a work list to complete.

## Decisions (running log)

**The findings come from the review's commit `ee2568b`, and its line numbers
still hold.** That commit changed only files under `.grove/`. So each cited
line of the spec, the app's spec and the code is the line that the review read.

**F1 is a real issue: two refusals reverse the order.** The spec admits two
starts at once when the count shows two free slots. Each refused start then
goes "back to place 1". So the start that the framework refuses second goes
ahead of the start that joined first.

The fix keeps the rule of the human, "sends the start back to the head", for one
refusal. When more than one refused start returns, they keep the order in which
they joined. So the queue is in join order always, and a refusal never puts a
start behind a later one.

**F2 is a real issue: the cleanup by name can stop a VM that the app did not
start.** A mismatch of the identity proves only that the recorded process is
gone. It does not prove that the VM of that name is still the clone of the
start. `vm stop` today refuses with `VM_STOP_UNPROVEN` when the pid names
another process. The fix uses the state that tart reports for the name:

- The recorded `tart run` matches in full: the app removes the start, as before.
- tart reports the VM as running, and no recorded process matches: the app
  keeps the VM, its sidecars and its files. It removes only the record, and it
  reports the start with `removed` false. It never stops that VM.
- tart reports the VM as stopped, or no such VM: no `tart run` holds it. So the
  app removes it by name with `tart delete`, and it sends no signal.
- tart does not answer within 5 s: the app removes nothing and keeps the record.
  The next start of the app decides again.

The record goes in the second case too. A record that stayed would let a later
start of the app remove that VM by name after a person stopped it.

**F2 leaves one trade-off, which the spec now records as an open risk.** tart
names no instance of a VM. `tart list` and `tart get` on tart 2.40.1 give a
name, a state, the disk, the size, the OS and an access time, and no identity.
While the app is down, a person can make a stopped VM under the same name. That
VM looks like the clone of the start, and the app removes it.

A proof of the instance would need a read of tart's own storage, which no code
of TestAnyware does today. The app never stops a running VM of that name, so the
requirement "stop no VM that it did not start" holds.

**F3 is a real issue: the expiry rule removes the last try.** The admission
decision fails each expired waiter before it admits. The wait section promises a
last try at the deadline. The retry loop of today makes that try, and
`--wait=0` makes one try.

The fix reverses the order inside one decision. The decision admits first, and
then it fails each waiter whose deadline came and that it did not admit. A
deadline also becomes an event of the decision. So a waiter gets one last
decision at its deadline. `--wait=0` then gets the result of a start without
`--wait`, as today.

**F4 is a real issue: the app's spec gives `macos_slots` the type of a path.**
The rule of an absolute path now names the path settings only. Both specs now
say that the shared evaluator checks `macos_slots` for each process, and that
only the app uses it.

**A stale claim beside F4 is a contract stated unclearly.** The app's spec and
ADR-0021 say that golden creation sends no request to the app. With the queue,
golden creation sends the hold request when the app runs. That request carries
the directories in its headers, as every request does. The fix narrows the
claim. Golden creation sends no request when the app does not run, so its own
check of the directories stays.

`docs/reference/env-vars.md` also repeats the rule of a path for every setting.
The spec now lists it among the documents that change.

**One reviewer read the three fixed rules, and its eight findings are
triaged here.** The reviewer read the spec with the digest `2517c05`. It found
that the order of two refusals and `--wait=0` hold.

- **R1, an acceptance skips the pause, is real.** The app admits A and B
  together, the framework refuses A and accepts B, and contention ends. The
  count then shows a free slot, so A tries again at once. The fix makes the
  pause after a refusal hold whether or not contention holds.
- **R2, two ends of contention, is real.** The text ends contention also when no
  start waits and none is unproven. The flowchart and `CONTEXT.md` end it only
  at an acceptance. The fix keeps the rule of the text, because the other
  rule blocks starts that the limit does not block.
- **R3, a missed last try, is real.** At the deadline of the head, another
  admitted start can still be unproven. The fix states that case, and it no
  longer claims "as the retry of today" for it. `tart_error` now follows row
  271 of the contract: an empty string only when the start ran no `tart run`.
- **R4, the end of the pause is no event, is real.** The fix adds the event.
- **R5, a state diagram without contention, is real and small.** Its labels now
  name the decision.
- **R6, an unclear "unproven", is a contract stated unclearly.** A golden
  creation never gets an answer of the framework through the app. So it could
  hold contention open for its whole run. The fix defines the term, and a
  golden creation is never unproven.
- **R7, an orphan after a crash before the record names `tart run`, is a real
  trade-off.** Row 3 keeps a VM that the record does not prove, and that is the
  safe side. With no meta, only tart can stop that VM. The spec now records the
  risk and its remedy, and the scenario of a crash names its window.
- **R8, small gaps, is real.** The table now decides any state other than
  running. Two labels of the flowchart now agree with the text.

**The fixes of R1, R2 and R6 earn a new `review-design` leaf before planning.**
They change how contention gates an admission, and no executable test covers
them yet. A second read in this session would spend a second reviewer, which
this kind may not do. So the session cuts `review-design` for the
contention rules and the start-record table as this leaf leaves them.
