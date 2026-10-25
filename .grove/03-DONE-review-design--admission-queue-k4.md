# admission-queue-k4

**Reviews:** admission-queue-k2

## Goal

Read the design of the admission queue adversarially, and report findings
without fixes. The design is `docs/specs/admission-queue.md` and ADR-0022. It
also includes the edits that `admission-queue-k2` made to ADR-0021, to the
contract, to the app's spec and to `CONTEXT.md`.

## Context

- The commit of `admission-queue-k2`, and the decision log in its task file.
- `docs/specs/testanyware-app.md`: "The protocol", "Lifecycle and adoption" and
  "Version skew", which the queue extends.
- `docs/architecture/cli-design-contract.md`: §3.2 and the `VM_LIMIT_REACHED`
  row of §4.2. Also the amendments of 2026-10-19 (`--wait`) and 2026-10-25.
- The code of today: `run_until_slot` and `wait_for_slot` in the VM lifecycle
  crate, and the app's start handler with its gate for each VM id. Also
  `provision_boot1`, and the recovery cycle of ADR-0008.

## Done when

- Each finding names the passage, the failure that it allows, and a case that
  shows it.
- The review either cuts an `integrate-review-design` leaf for its findings, or
  states that it found nothing worth acting on.

## Notes

The human rejected a Quint model, because one process behind one lock runs the
queue. So the claims about a cancel and a crash rest on prose. The producer
could not settle these doubts alone.

**Every interleaving.** The events are a join, a leave, an admission, a refusal,
an acceptance, a release, an expiry and a quit. Look for an order of them that
loses a slot or leaves a VM. Look also for one that frees the slot of a live
guest. These cases need the closest look:

- a cancel at the admission
- a quit during a boot
- a crash between the removal of the start record and the result
- a quit while golden creation holds its slot.

**Contention.** Can the rule of one start at a time block starts that the limit
does not block? Can a start that the framework never answers hold the
contention open? The spec relies on the VNC stage of the boot to end such a
start.

**The count.** Check the rule for a guest that two holders name. Check an
adopted VM of an older meta. Check the fall-back to the own holders when tart
does not answer within 5 s.

**Compatibility.** Check each new member of `VM_LIMIT_REACHED` against §3.2, and
the empty `tart_error`. Check the rules of protocol version 2 against an older
Host CLI and an older app. An older Host CLI fails on an event that it does not
know, so check that the queue adds none to `POST /v1/vms`.

**The sets.** The spec and the app's spec must not restate each other. ADR-0022
and ADR-0021 must be a minimum coherent set. The contract's amendment of
2026-10-25 must agree with the spec.

## Decisions (running log)

**Use the mandated Grove 22.6.0 executable.** The executable on `PATH` refused
the tree because it expected a newer format witness. The installed executable
at `/opt/homebrew/Cellar/grove/22.6.0/bin/grove-llm` resolved this live leaf.
No tree format was changed.

**Use source reads for this review.** The code graph refused `list_projects`
because a pre-coordination or unverified generation was active. No graph
generation or coverage could be established, so source discovery and exact
source reads supply the code evidence. This is an inspection-only review; no
test, build, lint or format command runs.

**Report the order of multiple refusals as an actionable finding.** Two starts
can be admitted together. Returning each refused start to place 1 reverses
their order when the earlier start is refused first. This differs from the
already documented risk that a later concurrent boot can win before contention
starts: both starts can be refused, and the reversed queue then persists into
contention. The integration must triage this against the FIFO requirement.

**Report startup cleanup after an identity mismatch as an actionable finding.**
The start-record rule falls back to `tart stop` and `tart delete` by name when
the recorded process does not match. A new, unrelated guest can have taken that
name while the app was down. This bypasses the existing stop rule that preserves
the VM and sidecars on an identity mismatch, and can stop a guest the app did
not start.

**Report the deadline rules as contradictory.** The admission function expires
waiters before admitting them, but the wait section promises a final try at the
deadline. The existing retry loop makes that final try, and the CLI accepts
`--wait=0`, which still gets an initial try today. These cases need one explicit
transition rule before implementation.

**Report the configuration type contradiction.** The app's spec now lists
`macos_slots` among its settings but still requires each setting to be a string
with an absolute path. The queue spec requires an integer. The shared evaluator
cannot satisfy both readings.

## Findings

### F1 — P1: Multiple refusals reverse the original queue order

**Passage:** `docs/specs/admission-queue.md:243–255`, together with the two
simultaneous admissions at `:163–164`. The FIFO requirement is at `:646–649`;
ADR-0022's decision also promises that no later waiter takes a freed slot first.

**Case:** Two guests that the app cannot see fill the Mac. The app sees no
holder and admits A, then B, together. Both are refused. A's refusal arrives
first, so A goes to place 1. B's refusal arrives next, so B goes to place 1 ahead
of A. When an external guest stops, contention retries B alone and B gets the
slot. A can run out of time behind a request that joined after it.

The one lock serializes these transitions but does not preserve their original
order. This case is distinct from the already recorded risk at `:848–851`:
neither initial try succeeds, and the reversed order persists after contention
has started. Preserve the original admission order when several outstanding
tries return to the queue, and specify this interleaving for the pure decision
and protocol seams. Reconcile the unconditional FIFO claims in the spec and
ADR with the resulting rule.

### F2 — P1: Startup cleanup can stop a replacement VM that the app did not start

**Passage:** `docs/specs/admission-queue.md:450–455`, especially the fallback
to `tart stop` and `tart delete` by name when the process identity does not match.

**Case:** The app crashes while starting A, leaving a start record. While the
app is down, a person stops and deletes that VM, then starts a different VM
under the same name A. On restart, the recorded identity is gone or differs,
so the new rule stops and deletes the replacement by name. A VM name does not
prove that it still identifies the instance created by the interrupted start.

This violates the requirement at `:749–753` to stop no VM the app did not
start. It also bypasses the existing rule in
`docs/specs/testanyware-app.md:717–720`, which preserves the VM and its files
when an identity does not match. The current implementation makes that refusal
before deleting anything (`cli-rs/crates/testanyware-vm/src/lifecycle.rs:636–647`).
Distinguish an unavailable identity from contradictory ownership evidence, and
require proof of the VM instance before destructive cleanup by name. Preserve
and report an ambiguous record instead of treating its name as authority to
delete a replacement. Include replacement of a recorded VM while the app is
down in the restart seam.

### F3 — P2: Expiry before admission removes the promised final try

**Passage:** `docs/specs/admission-queue.md:210–212` and its diagram at
`:221–232`, compared with the final try at the deadline at `:314–319`.

**Case:** A has `--wait=2`, is refused by the framework, and its shortened pause
ends at its deadline. Its next admission decision first expires it, so it
fails with `refused_by: app` without making the final try promised by the wait
section. If the unseen guest stopped just before that deadline, the final try
would have succeeded. The existing loop makes the try before testing the next
refusal (`cli-rs/crates/testanyware-vm/src/lifecycle.rs:519–538` and
`cli-rs/crates/testanyware-vm/src/start_outcome.rs:304–313`).

There is also no stated exception to the expiry-first rule for an initial
`--wait=0` request when a slot is free. The CLI accepts zero
(`cli-rs/crates/testanyware-cli/src/main.rs:1905–1906`), and the existing loop
still makes its initial attempt. Define one rule for initial admission, a due
final retry and expiry, including their order at the deadline. Update the
diagram and acceptance cases to agree with the compatibility promise.

### F4 — P2: The two specs require incompatible types for `macos_slots`

**Passage:** `docs/specs/testanyware-app.md:865–873` adds `macos_slots` to
the settings, then says that each setting is a string holding an absolute path.
`docs/specs/admission-queue.md:507–515` requires an integer 0, 1 or 2 and rejects
every other type. ADR-0021 also describes the numeric slot count.

**Case:** A file returns `macos_slots: 1`. The queue spec accepts it, while the
app spec's type rule rejects it. Following the latter would make the Host CLI
fail with `CONFIG_INVALID`; the app's fallback on invalid configuration could
instead run with the default of two slots. A caller cannot select a value that
satisfies both documents.

Restrict the absolute-path rule to the path settings, and make the app's
configuration rules agree with the integer setting and the Host CLI's stated
handling of it.

## Inspection coverage and limits

The reviewed producer is commit `05bf97e6cd8bb05db00dacb8d9ea20386041df6e`.
Its changed documents, the requirements decision log and the cited ADRs were
read. Source reads covered the app's start and per-id gate, its stop handler,
the VM lifecycle's retry and cancellation paths, the error payload, golden
boot 1, the IPSW base boot and the recovery boot. No implementation or test code
was changed, and no test, build, lint or format command was run.

The VNC acceptance signal is supported by the pinned
[tart Run source](https://github.com/openai/tart/blob/95a0dac/Sources/tart/Commands/Run.swift#L567-L605):
VM start completes before the VNC URL is printed. The web reader could not
fetch that page, but a direct read of the pinned raw source succeeded. The local
VNC observer has a bounded poll and reports a VNC-stage timeout when a live
process never produces a URL (`lifecycle.rs:1166–1190`). The retained-VM and
cleanup-failure cases must continue to hold their slots, as the spec states.

The optional `cwd`, existing progress/result/error events, final-only JSON and
string `tart_error` preserve the inspected protocol and payload types. ADR-0021
owns configuration; ADR-0022 owns admission. Their responsibilities remain
distinct. The app spec references the queue's mechanics rather than repeating
them, apart from the conflicting configuration rule in F4.

The source of own occupancy for an older meta is the running guest observed
through tart, not a recycled pid. The tart timeout fallback is an explicit
choice to try with the known own holders; it does not prove that external
occupancy disappeared. The framework remains the final check. These rules and
name deduplication need the stated pure-function and protocol cases; this
inspection supplies no execution evidence for their eventual implementation.

The initial race between two concurrent tries, the crash window between removal
of the start record and delivery of the result, and a golden creation continuing
after the app quits are disclosed in the agreed spec's Open risks. They were
treated as recorded trade-offs rather than new findings. No formal model or
additional reviewer was introduced. The unavailable graph prevents graph-based
coverage claims; this review makes no exhaustive claim about the repository.
