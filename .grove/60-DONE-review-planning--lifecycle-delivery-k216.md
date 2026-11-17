# lifecycle-delivery-k216

**Reviews:** lifecycle-delivery-k213

## Goal

Challenge the delivery plan of lifecycle-delivery-k213 before leaf 61 starts.
Find each requirement that has no owner. Find each order that leaves the
product broken. Find each leaf that cannot fit one session.

## Context

- The plan is the leaves 61 to 71 and the root brief section "k213 delivery
  plan".
- The design is `docs/specs/guest-command-lifecycle.md`. Read its sections
  "Implementation candidates", "Test seams and required controls", "Bounds"
  and "Validation route".
- The planning input is in lifecycle-evidence-k215, under its planning
  section.
- The root brief holds the acceptance inventory, the byte oracle, the ceilings
  and the required continuation.

## Done when

- Check that each part of candidates A to D has exactly one owning leaf.
- Check that each row of the controls table has an owning leaf. Check that
  each new row names its old failing path or states that none exists.
- Check that the record reader lands directly after the first record writer.
  Check that the cancel types land with `file cancel`, their only reader.
- Check that each implementation leaf leaves every existing test green and the
  Host CLI text output unchanged.
- Check that each implementation leaf fits one session. Name each leaf that
  needs a split.
- Check that the preparation, acceptance and handoff leaves keep every root
  case, byte oracle, ceiling and bound. Check that the freeze comes before any
  measurement.
- Check that the release leaf carries every Host CLI and agent change.
- Record each finding with its evidence in this leaf.
- If a finding exists, insert an `integrate-review-planning` leaf directly
  after this leaf.

## Notes

Review the plan, not the design. Lifecycle-evidence-k214 and k215 settled the
design. Raise a design defect only when a leaf cannot meet the spec as
written.

## Decisions (running log)

- Reviewed the committed plan at `7be5b751` against the spec, ADR-0023 to
  ADR-0026, k215's planning input and the root acceptance contract. This was
  an inspection-only review; no tests, builds, lint or format commands ran.
- Four findings below meet the acceptance or material-risk threshold. Leave
  the implementation tasks and design unchanged here; their disposition
  belongs to the immediately following planning integration.
- Inserted `lifecycle-delivery-k228` immediately before k217. It consumes
  this review's committed findings and may accept or reject each one under
  the integration threshold. The insertion renumbers k217 to k227 without
  editing their bodies; the coordinates below intentionally retain the
  reviewed producer's paths.

## Findings

Coordinates below refer to producer commit `7be5b751`, before insertion of
the integration renumbers the later tasks. Handles identify the tasks across
that insertion.

### R1 — The control completion conditions require false old failures

Evidence: `.grove/61-impl--linux-agent-lifecycle-k217.md:35`,
`.grove/63-impl--macos-agent-lifecycle-k219.md:30`,
`.grove/64-impl--linux-agent-cancellation-k220.md:31` and
`.grove/65-impl--macos-agent-cancellation-k221.md:27` require every listed
control to fail on the old path and pass on the corrected one.
`docs/specs/guest-command-lifecycle.md:677` and
`docs/specs/guest-command-lifecycle.md:678` explicitly designate unavailable
leader status and stalled capture as new behaviour with no old failing path.
`docs/specs/guest-command-lifecycle.md:683` does the same for cancellation.
`docs/specs/guest-command-lifecycle.md:690` uses today's legacy replies as
the equality reference, not a failing implementation.

Those are different proof obligations. The legacy projection should agree
with its old reference; it must not require that reference to fail. An
unsupported old cancellation endpoint does not exercise the new arbitration
and tombstone semantics. The unchanged disconnect policy likewise need not
be an old failure. As written, these leaves cannot satisfy their completion
conditions honestly. The plan needs an explicit per-control distinction
between an established old failure, new behaviour with no old failing path,
and a compatibility reference. Keep the actual old-fail controls.

### R2 — The CLI leaf narrows the agreed interruption contract

Evidence: `.grove/66-impl--host-cli-control-k222.md:25` requires a cancel on
SIGTERM followed by exit within 2 s and refers to a further SIGKILL period;
`.grove/66-impl--host-cli-control-k222.md:30` requires download cleanup only
on SIGTERM. Its loopback tests cover those named cases.
`docs/specs/guest-command-lifecycle.md:301` requires both SIGINT and SIGTERM,
one cancel on a new connection with `waitMs: 0`, a wait of at most 1.5 s,
restoration of the default action and re-raising the same signal.
`docs/specs/guest-command-lifecycle.md:605` also requires download cleanup
and signal re-raising for both signals.

An implementation can meet the leaf's explicit checks while leaving Ctrl-C
uncancelled, leaking its download temporary file, or returning an ordinary
exit code instead of the parent's existing signal-death observation. The
root's outer-controller SIGTERM/SIGKILL escalation does not prove these CLI
semantics. Make the full two-signal contract and its observable signal death
an explicit responsibility of k222; distinguish its 1.5 s cancellation wait
from the controller's 2 s plus 2 s escalation. Retain the old-agent 404 path.

### R3 — Neither agent split assigns bounded shutdown

Evidence: the lifecycle responsibilities in
`.grove/61-impl--linux-agent-lifecycle-k217.md:25` and
`.grove/63-impl--macos-agent-lifecycle-k219.md:25` describe per-command
termination. The remaining responsibilities in
`.grove/64-impl--linux-agent-cancellation-k220.md:22` and
`.grove/65-impl--macos-agent-cancellation-k221.md:21` describe HTTP
cancellation, retention and download. None names agent shutdown, and their
selected spec sections omit "Agent shutdown".
`docs/specs/guest-command-lifecycle.md:245` requires SIGTERM to stop request
admission, cancel every running command, wait only for bounded record
publication and exit even when leader status remains unavailable.

This is a server-to-owner integration responsibility, not just the owner
ending one request. Without a chosen owner in each platform's split, the
plan can finish its listed controls while shutdown abandons groups or waits
indefinitely for an unreapable leader. Assign that contract and a bounded
shutdown check to one leaf per platform. VM disposal is explicitly not its
substitute. This finding does not reopen the settled shutdown design.

### R4 — Linux threading omits the required accessibility serialization

Evidence: `.grove/64-impl--linux-agent-cancellation-k220.md:22` changes the
server to one thread per request and checks health responsiveness. Its
remaining controls cover exec, cancel, disconnect and download.
`docs/specs/guest-command-lifecycle.md:319` requires all accessibility
endpoints to share one lock because AT-SPI is not thread-safe, while exec,
cancel, health, upload and download never take that lock.

The plan assigns the concurrency change without its required guard. Its
listed controls can pass while simultaneous existing accessibility requests
regress; applying a server-wide lock instead would restore the blocked
health and cancellation defect. Make the lock boundary and a check of that
boundary part of k220's ownership. This preserves existing product behaviour
as the threaded server lands.

## Other checks

The writer/reader order is sound: k218 directly follows the first writer
k217, and k222 owns cancel types with their consumer. A to D and all rows of
the required-control table have identifiable owners; R1 concerns their proof
conditions. The root explicitly requires every existing test green and text
output unchanged for each change. The leaf scopes are coherent enough that
this inspection establishes no mandatory session-size split.

The preparation, acceptance, release and handoff tasks inherit the full root
inventory, byte oracle, ceilings and resource rules. They retain the freeze
before trial, sequential five macOS then five Linux clones, separate abrupt
interruption, failure preservation and a new trial for a revised candidate.
k226 carries all agent and Host CLI changes; k227 maps the root completion
items, including the downstream rerun, to evidence. No repair or acceptance
claim follows from this planning review.
