# complete-worker-scheduler-k130

**Reviews:** complete-worker-scheduler-k126

## Goal

Independently challenge the parent-side worker request transport integration
with the shared turn driver. Read the committed producer and its evidence at
`experiments/exec-reliability/evidence/request-turns-k126/`. Verify the narrowed
contract against actual production callbacks and the retained aggregate scope.
Report findings; do not fix implementation in this review leaf.

## Done when

- Trace actual persistent and standalone capture_helper query and retirement
  registrations. Challenge ordinary/safety priority with competing ready work,
  turn I/O bounds, zero-timeout readiness, terminal callbacks and wake behavior.
- Check fixed query/shutdown deadlines, original first-reap and lifetime
  admission guards, late cleanup-only identities, unknown creation obligations,
  sticky failure and persistent close against the retained k129 charter. Do not
  infer concurrent production ownership or teardown overlap from this seam.
- Inspect frozen inputs, every retained failed sample, mutation failure causes,
  external fixture identity audit, before/after hashes and per-item parent/full
  comparisons. Challenge whether excluding observer I/O preserves meaningful
  transport read-cap and scheduling evidence. Separate inherited failures from
  any regression introduced here.
- Record actionable findings with exact committed source and evidence anchors.
  If findings require implementation, insert an integrate-impl sibling directly
  before complete-worker-scheduler-k129. The full aggregate, historical failures
  and final k28 review stay owned by k129; k28/k17 remain open.

## Decisions (running log)

- 2026-11-15: Flat review before the next producer because this seam controls
  deadlines and process ownership. Independent challenge belongs in a fresh
  Grove review session. The producer session supplied no current provider
  evidence, so no Creator line is recorded.
