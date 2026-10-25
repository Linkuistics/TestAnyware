# admission-queue-k6

**Reviews:** admission-queue-k5

## Goal

Read adversarially the rules of the admission queue that `admission-queue-k5`
changed, and report findings without fixes.

## Context

- The commit of `admission-queue-k5`, and the decision log in its task file. The
  log triages the findings of `admission-queue-k4` and of one reviewer that k5
  ran.
- `docs/specs/admission-queue.md`: "The order", "The admission decision", "A
  refusal by the framework", "A start without `--wait`", "The wait and its end"
  and "A quit, a crash and a restart". Also its requirements, its test seams and
  its open risks.
- `CONTEXT.md`: *Contention*, *Unproven* and *Start record*.
- The code of today: `run_until_slot` and `wait_for_slot` in the VM lifecycle
  crate, and `next_slot_step`. Also the adoption decision, `stop_proven`, and
  the point where a start writes its meta.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model of the queue. So these rules rest on prose and
on the seam of pure functions, and no test exists for them yet.

**Contention.** k5 made the pause after a refusal hold apart from contention. It
defined an unproven start. It also ends contention when no start waits and none
is unproven.

Look for an order of events that lets two starts try together after a refusal.
Look for one that blocks a start that the limit does not block. Look also for
one that holds contention open with no end. These cases need the closest look:

- a golden creation at the head in contention
- a start that ends with `VM_BOOT_TIMEOUT` while its `tart run` lives
- an acceptance while a refused start waits.

**The deadline.** The decision admits first, and then it fails each waiter whose
deadline came. Check a deadline during the pause, and a deadline while another
start is unproven. Check `--wait=0` against a start without `--wait` in each
state of the queue.

**The start record.** A table decides by the identity check and by the state
that tart reports for the VM. Check each row against "the app never stops a VM
that it did not start" and "no slot is lost". Check the two open risks that k5
accepted:

- a VM that does not run, under the name of a start record
- a crash before the start record names its `tart run`.

**The sets.** The spec, ADR-0022, the contract's amendment of 2026-10-25, the
app's spec and `CONTEXT.md` must agree with each other.

## Decisions (running log)

**Use the mandated Grove 22.6.0 executable.** The executable on `PATH`
expected a newer tree format and refused the missing format witness. The
installed executable at `/opt/homebrew/Cellar/grove/22.6.0/bin/grove-llm`
resolved this live leaf. No tree format changed.

**Use source reads for the code evidence.** The code graph refused
`list_projects` because a pre-coordination or unverified generation was active.
No project generation or index coverage could be established. Exact source
reads supply the evidence, with no exhaustive repository claim. This review
runs no test, build, lint or format command.

**Report two ways to reopen concurrent tries after a refusal.** An admitted
golden creation is never unproven, so a later decision can admit another start
while the golden creation has no answer from the framework. Also, acceptance
of an outstanding later start ends contention while an earlier refused start
still waits. A release can then admit a later waiter while the earlier retry
is unproven. Both cases escape the stated serialization after a refusal.

**Report retained sidecars as an incomplete ownership correction.** A crash
after writing the meta but before removing the start record leaves both. The
new preservation row keeps that meta when a replacement VM runs under the
name. A subsequent stop can use the stale meta to stop the replacement by
name when the recorded main process is gone. Preserving the start record's
VM requires reconciling every sidecar that can still authorize its stop.

**Report the missing mixed-identity row.** The table decides a running VM when
no recorded process matches, but the start also records the independent VNC
keeper. A matching keeper and a gone main process satisfy no row when tart
answers that a replacement VM runs. The decision needs a defined outcome for
that combination; a keeper does not prove ownership of the main VM.

## Findings

### F1 — P1: An admitted golden creation does not guard its try in contention

**Passage:** `docs/specs/admission-queue.md:269–272` excludes golden creation
from unproven starts. The admission events and decision at `:209–219` and
`:238–241` can therefore admit the next waiter before that golden creation
gets an answer from the framework. Its hold protocol at `:609–618` reports no
boot outcome to the app.

**Case:** Two unseen guests fill the Mac. A is refused, and G (a golden
creation) and B wait behind A. A cancels, and one unseen guest stops. After the
pause, the app admits G alone. G holds one slot in the app's count, but it is
not unproven. G can still be preparing its base or setup VM when the next tick
or join causes another decision. The app sees one own holder, no seen holder
and no unproven start, so it admits B. B can take the one real free slot before
G's boot. G then gets framework refusals and can fail after 30 s, although it
was admitted ahead of B and still holds its reservation.

This contradicts the claim at `:279–281` that two waiters never try together
after a refusal and that the later one cannot take the freed slot first. It
also defeats the promised protection of golden creation's boots at `:373–375`.
The initial simultaneous-admission risk in Open risks does not describe this
case: contention has already begun, and separate decisions admit G and B.

Define what protects an admitted golden creation during contention, including
the interval before its first boot and between its boots. The existing hold
connection supplies no acceptance event; merely excluding G from unproven
starts removes the protection. Specify this case at the pure decision and hold
protocol seams, and reconcile the guarantees in the spec, glossary and ADR.

### F2 — P1: A later outstanding acceptance clears contention before the head is proven

**Passage:** `docs/specs/admission-queue.md:275–281` ends contention on any
acceptance. The general decision at `:216–219` then admits by the count even
when the head's retry is still unproven. Keeping the pause at `:264–267` does
not preserve serialization after that pause ends.

**Case:** X and Y are unseen guests. The app admits A and B together. A is
refused first and returns to the head. Y stops before B's outstanding try is
answered, so B is accepted. That acceptance clears contention while A still
waits. C joins behind A. After the 3 s pause, the app retries A; B and A then
fill the own count. B stops before A's retry has an answer. That release leaves
one own holder, no seen holder and no contention, so the app admits C while A
is unproven. X still holds one real slot. C can win the other slot, and A is
refused again behind the later start.

The one lock orders every admission in this case, and every pause is honored.
The loss of order comes from forgetting contention while an earlier refused
request remains unresolved. It is a new race after the refusal, beyond the
accepted initial race recorded in Open risks.

Give the acceptance that ends contention an explicit relationship to the
refused head and the outstanding tries. Either preserve the promised retry
order for this sequence, or explicitly triage the weaker guarantee against the
requirement. Add this sequence to the pure decision and protocol cases, and
keep the spec, the decision "The TestAnyware app admits macOS guests through
one ordered queue", and the *Contention* definition consistent.

### F3 — P1: Keeping the old meta preserves authority over a replacement VM

**Passage:** `docs/specs/admission-queue.md:495–509` preserves the VM and its
sidecars and removes only the start record. It then says that the app never
stops that VM. The success ordering at `:476–477` permits a meta and a start
record to coexist before a crash. The count at `:121–125` treats a running
guest with a meta as an own holder.

**Case:** The app writes A's meta, then crashes before removing A's start
record. While the app is down, a person stops and deletes A, then runs an
unrelated replacement under the name A. The original main process and keeper
are gone, and their pids have not been reused. On restart, row 3 preserves the
replacement and the old meta, and removes the start record.

The old meta now causes two failures. With `macos_slots: 1`, the replacement
uses the app's whole share, although it should count as external occupancy and
leave the other real slot available to a new app start. Also, `vm stop A` can
still stop and delete the replacement by name. Adoption classifies a gone
recorded main process as `Stopped`, not `Doubt`
(`cli-rs/crates/testanyware-vm/src/adoption.rs:123–134`). `stop_proven` refuses
only `Doubt`; for `Stopped` it passes pid 0 to `TartRunner::stop_in`
(`lifecycle.rs:636–656`). Pid 0 suppresses the signal, but `stop_in` still
removes the named VM (`tart.rs:617–625`). A missing process therefore does not
neutralize the preserved meta's authority. The app's adoption report even
recommends `vm stop` to remove the records of such a gone process
(`cli-rs/crates/testanyware-app/src/adoption.rs:110–117`), and the app's stop
handler calls this lifecycle path (`stop_delete.rs:61–67`).

This is distinct from the accepted risk of deleting a stopped replacement:
the replacement runs, and the new preservation row deliberately keeps it.
Reconcile that row with meta-based counting, adoption and stop authority.
Preserving the ambiguous VM must not leave a sidecar that classifies it as
owned or authorizes its later removal. Cover the coexistence of a meta and a
start record in the startup decision and protocol seam.

### F4 — P2: The startup table has no outcome when only the keeper matches

**Passage:** `docs/specs/admission-queue.md:472–484` records and checks each
process. The first row at `:493` requires a matching `tart run`, whereas the
preservation row at `:495` requires that no recorded process match.

**Case:** A start has recorded both its main process and its VNC keeper. The
app crashes. The main process ends, and a replacement VM runs under the same
name while the old keeper is still alive. tart answers that the name runs.
The main process does not match, so row 1 fails. The name runs, so row 2 fails.
The keeper matches, so row 3 fails. tart answered, so row 4 fails. No row
decides, although the function is required to decide for every start record
before the socket is served.

The keeper is an independent detached process. Its exit follows connection
closure and a retry delay; it is not atomic with the main process's exit
(`cli-rs/crates/testanyware-vm/src/keeper.rs:147–151,192–234`). The current
adoption decision explicitly uses the main process alone
(`cli-rs/crates/testanyware-vm/src/adoption.rs:7–10`).

Make ownership depend on the relevant main process and define the outcome of
mixed process checks. A matching keeper must not prove ownership of the VM.
Include this combination in the table's pure-function cases and align the
matching flowchart label at `:540` with the chosen predicate.

## Inspection coverage and limits

The reviewed producer is commit `d66d5cbf`, identified by
`admission-queue-k5`. Its committed diff and decision log were read against the
current documents. The review read the requirements log, root brief, queue
spec, affected passages of the app spec and CLI design contract, the glossary
and the cited ADRs. "One Rhai file configures the TestAnyware app and the Host
CLI" owns configuration; "The TestAnyware app admits macOS guests through one
ordered queue" owns admission. Their responsibilities remain distinct. No
additional module or test seam is
needed to state the findings: the agreed pure functions and app protocol can
cover these cases.

Source inspection followed `run_until_slot`, `wait_for_slot`,
`run_and_observe`, the bounded VNC observation, start settlement, the meta
write, process identity checks, adoption, `stop_proven`, the tart stop helper,
the VNC keeper and the app start handler. The retry of today calls
`run_and_observe` before checking `next_slot_step`, so its final try and its
initial `--wait=0` try support the revised admission-before-expiry rule.
Under the written pause exception and contention gate, `--wait=0` and a start
without `--wait` have the same admission result. Another unproven start can
still deny the final try, as the revised spec now discloses.

A live `tart run` that ends its start with `VM_BOOT_TIMEOUT` does not remain
unproven under the revised definition. `--keep-on-failure`, or a failed
teardown, can retain that VM and its meta (`lifecycle.rs:1380–1421`). It must
remain a holder until its guest is gone; ending unproven status does not
release that hold. This is consistent with the stated lifetime rule and
does not itself establish an endless contention state.

The stopped replacement and the spawn-to-record gap are explicit accepted
risks from k5. They were not reported again as findings. The initial race
before contention, the gap between removing the start record and sending the
result, and golden creation continuing after the app quits also remain
disclosed trade-offs.

This is a findings-only inspection. It changed no production or test code and
ran no test, build, lint or format command. The graph CLI refusal prevented
project-generation and coverage evidence; source reads were used instead.
The event sequences are prose counterexamples, not execution results or a
formal proof. No subagent reviewer or formal model was introduced. The leaf
`admission-queue-k7` will triage these findings before planning.
