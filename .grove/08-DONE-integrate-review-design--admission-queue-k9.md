# admission-queue-k9

**Integrates:** admission-queue-k8

## Goal

Triage the committed findings of `admission-queue-k8` and apply the supported
corrections to the admission design before planning consumes it.

## Context

- Read `admission-queue-k8` from its own commit: its task file holds the
  findings, cases, source evidence and inspection limits.
- The reviewed producer is `admission-queue-k7`, commit `c7e15f04`.
- The root brief and the decision log of `plan-k1` hold the agreed requirements
  and test seams. The current design is the admission-queue spec, its affected
  app-spec passages, the configuration and admission decision records, the CLI
  contract amendment and the glossary.

## Done when

- Every finding has a reasoned disposition, including any rejected finding or
  accepted trade-off.
- Supported corrections agree across the affected design documents, contract
  and glossary.
- The design is ready for `admission-increments-k3`, or the tree names the
  additional design work that must precede planning.

## Notes

This integrates a design review. Its findings are input to triage, rather than
a work list to apply without checking. The review changed no production or
test code and ran no test, build, lint or format command. Read its coverage
limits before deciding what the evidence establishes.

## Decisions (running log)

**The findings come from the review's commit `32e91d2`, and its line numbers
still hold.** That commit changed only files under `.grove/`. No document or
source file changed after `c7e15f04`. So each cited line is the line that the
reviewer read.

**F1 is a real issue: a legacy meta does not prove the VM that row 3 keeps.**
The case holds in the code. `decide` returns `Legacy` for a meta with no
identity, and it checks no process. The scan of the app puts `Legacy` among
its survivors, and the stop of a legacy tart meta removes the VM by name. So
the exception "unless adoption adopts the VM by its meta" gives the app
authority over a VM that it cannot prove.

The fix keeps the sidecars in row 3 only when the meta's own `tart run`
matches in full. Only then does the meta prove an earlier start of the same id
whose `tart run` still runs. For each other outcome of adoption, the app
removes the spec and the meta, keeps the VM and reports `removed` false. The
spec now states the result for `Adopt`, `Stopped`, `Doubt` and `Legacy`.

The cost is small. The second start of that id wrote its record, so its clone
step was about to remove that VM by name. A legacy VM that still runs under the
id now counts as external, and only `tart stop` and `tart delete` remove it.
The protection wins over legacy compatibility, because the app must never stop
a VM that it did not start.

**F2 is a real issue, and it also occurs inside TestAnyware.** The case holds.
The app observes only on its events, so two observations can be hours apart.

`prepare_in` removes a VM of the same id and clones it again. A Linux start
joins no queue, so it causes no observation. So a `vm start --id X` of a Linux
guest after a macOS guest X can leave the answer `darwin` for X. The count then
holds the Linux guest as an own macOS holder.

The fix keeps no OS answer between observations. At each observation, the app
asks `tart get` for the OS of each VM that `tart list` reports running. A VM
that does not run takes no slot, so it needs no answer. The cost is one
`tart get` for each running VM at each observation. That is a visible
trade-off. An identity for each instance of a VM would need evidence about the
storage of tart, and no source gives it.

**F3 is a real issue in two parts.** First, the kept observation ignores what
the app knows. The app reaps each `tart run` that it starts. Its own stop
succeeds only when tart reports that the VM no longer exists. So a release that
the app saw after the last answer is a fact, and the kept answer is not.
The fix removes from the kept observation each VM that the app stopped, or
whose `tart run` it saw exit, after that answer.

Second, the fallback before the first answer counts stopped metas. After a
restart of the Mac, each meta with an identity fails the check of its boot
session. So the fix counts, before the first answer, only a macOS guest of a
meta whose `tart run` matches in full or that records no identity. The second
case keeps a legacy VM from passing `macos_slots`. The old fallback also
counted a QEMU guest of a meta, which takes no slot, so the fix reads the
platform from the spec sidecar.

**The app still keeps the seen holders of its last answer while tart does not
answer, and the spec now states the cost.** To drop them would let a start take
the slot of a golden creation between its boots, beside a guest that the app
can see. So a waiter can fail at its deadline while a slot is free, and a
golden creation can wait until tart answers. The spec states this as an open
risk. `vm slots` gains `unanswered_since`, and a waiting start says on stderr
that tart does not answer. The remedy of `VM_LIMIT_REACHED` names the case.

**"A full count is a fact" now holds only while tart answers.** The spec and
ADR-0022 stated it without that condition. A count from a kept answer can be
stale, so both now state the condition. The contract amendment and the
glossary state no rule of the count, so they need no change.

**One reviewer read the revised passages, and its seven findings are triaged
here.** The reviewer got the passages, a contract of six items and the code
to check. It got no decision of this log. It found no fault in the kept
observation, the OS rule or the table of row 3, apart from R6.

- **R3, before the first answer a visible guest's slot shows as free, is real.**
  My fallback counted no seen holder. So a start can take the slot of a golden
  creation between its boots, beside a guest that tart would show. The fix
  admits nothing until tart first answers. That removes the fallback count, and
  so also the gap of R7 for a meta with no spec. The cost is the open risk of a
  tart that does not answer, which the spec already states.
- **R1, a stale meta makes a person's VM an own holder, is real and narrow.**
  k7 accepted it as a hazard of adoption. The count is the queue's, though, and
  the check of F1 fixes it at no cost. So a running guest of a meta is own
  unless the check of the meta's `tart run` finds it gone or finds another
  process. Then the VM of a person counts as seen. The stop of such a VM stays
  with adoption, as k7 decided.
- **R2, a crash in the clone step orphans a VM of the app, is a contract stated
  unclearly.** `prepare_in` clears the sidecars before it removes the VM of the
  id. A crash between the two leaves a running VM that no meta names, so it
  counts as external. That VM counts against the two slots but not against
  `macos_slots`. The open risk now names this gap and that effect. To change
  the order of the clone step would change today's code, and no reviewer read
  that.
- **R4, the remedy names a narrower case than "External occupancy", is real and
  small.** The remedy now applies when the count shows a free slot.
- **R6, the decision for a start record misses an input, and `removed` reads
  two ways, is real and small.** The inputs now include the check of the meta.
  `removed` is false when the app keeps a running VM that no meta proves.
- **R7, four undecided cases, is real and small.** An admitted start that fails
  has the place that it would take back. `unanswered_since` also counts from the
  start of the app. `contention` is `null` outside contention.
- **R5, `--id` stops a VM that the app did not start, is the exception that the
  spec chose.** My contract for the reviewer left it out. A setup VM of golden
  creation meets it only when a start names that VM by its id, as today.

**The fixes of R1 and R3 earn a new `review-design` leaf before planning.**
They change the count again, and no test covers it yet. This session spent its
one reviewer. So it cuts `review-design` for the rules that this leaf changed,
and the leaf names each doubt.
