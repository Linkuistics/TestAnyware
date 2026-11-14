# process-table-validation-k37

## Goal
Reject untrustworthy process tables at the current controller's observation boundary.

## Context
This independently useful slice of bounded-inspection-k34 integrates directly into
`_group_snapshot`. Read its ancestor briefs and the process-inspection specification.
The synchronous helper and waits remain; this slice makes no bounded-inspection claim.

## Done when
- Require a clean helper exit, empty stderr, complete table lines, unique positive
  PIDs, matching PGID, supported macOS states and the pinned leader.
- Reject more than 256 KiB combined raw output or 4096 rows without accepting a
  truncated prefix. Preserve raw failure evidence and perform no group probe on rejection.
- Check the observation deadline before the group probe and reject late observations.
- Verify both rejection controls and valid boundary cases through the real observation
  function; retain a fresh frozen self-check and update the foundation report.

## Decisions (running log)
- Split strict table acceptance into this first vertical slice because it improves the
  actual controller independently. worker-scheduler-k38 retains every remaining parent
  requirement, including the final full-subsystem review cut.
- Use the installed macOS 27 ps(1) state alphabet (I/R/S/T/U/Z plus documented modifiers).
  The Apple archive URL was unavailable through the web tool; the local official manual
  was read. Unknown future states fail closed.
- TDD rejection controls supply the behavioral disproof attempt. No extra in-session
  reviewer: the parent requires a fresh full-subsystem review after runtime completion.
- Freeze the previous controller with the current tests and supporting sources for
  the RED replay, then use the existing self-check task for both conditions. Keep
  the old run immutable and compare individual JUnit case identities and outcomes.
- Reuse the exact-resource auditor with witnessed absence-predicate controls. Its
  scope is recorded PIDs, exact resource markers and enumerated pytest-session
  scratch; it supplies no escaped-child or helper-time-bound claim.
