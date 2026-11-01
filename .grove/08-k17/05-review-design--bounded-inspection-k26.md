# bounded-inspection-k26

**Reviews:** bounded-inspection-k24

## Goal

Adversarially review the proposed inspection boundary before planning turns it
into code and retained-evidence-k21 relies on it.

## Context

Read the producer commit by handle, `docs/specs/diagnostic-process-inspection.md`,
the three editable visual views and the foundation report's bounded-inspection
section. Compare against k23-final-01, review k22 and integration k23. The current
runtime has not been changed; no inspection or storage guarantee is delivered
by this design. k25 is already live after this review and before k21.

## Done when

- Try to construct a helper creation, exec, capture, signal denial or post-KILL
  wait that can block the outer controller, including protocol backpressure,
  partial frames, destructor/exit hooks and concurrent worker cleanup timers.
  Check whether single-threaded pre-acquisition bootstrap and its readiness
  handshake cover actual Python/macOS behavior without an unacknowledged owner.
- Challenge target and worker group pinning independently: a late or stale
  inspector/callback must not signal a reaped/reused group. Check direct-helper
  PID ownership, no helper before readiness/registration, unknown creation races,
  finite shutdown, and residual cleanup after an unresponsive worker.
- Trace normal exit, TERM exit, grace expiry, PID-only interruption and permission
  failure through the stated absolute deadlines. Seek hidden extra allowances,
  an inspection that defers KILL, a final result from the wrong epoch, a dropped
  sticky failure or an unjustified absence claim. The reaping allocation is not
  evidence of ps latency. PID-only scope and live-final-group failure must hold.
- Check whether deterministic controls can demonstrate real delayed inspection,
  actual KILL followed by a stalled wait path, genuine EPERM plus uncertainty,
  stale/contradictory handoff and independent itemized helper cleanup. Injected
  permission errors and simulated unreapability must not pass as kernel evidence.
  Unknown real permission prerequisites must remain visible.
- Verify the worker contract leaves blocked evidence storage with k18, identifies
  workload spawn as a separate limit, and gives k21 immutable observation/residual
  inputs without weakening surviving-writer failure. Challenge unnecessary
  caller obligations or avoidable complexity against the named alternatives.
- Check spec/diagram agreement, source-backed claims and the concrete k25
  continuation. Produce findings only. If findings warrant integration, insert
  it before bounded-inspection-k25; a clean review creates nothing. Do not create
  an empty integration or take over implementation.

## Notes

No live guest, app quit, unsafe process suite or acceptance claim. This session
is the independent review; no competing in-session reviewer is needed. Static
analysis is sufficient unless a narrowly frozen, independently supervised local
experiment is necessary. Any such experiment retains every failed condition and
audits each exact owned identity. No provider evidence was present for k24, so
the producer intentionally wrote no Creator line.
