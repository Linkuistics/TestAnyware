# start-failure-tart-k7

**Reviews:** start-failure-tart-k2

## Goal

An adversarial read of `start-failure-tart-k2`: does tart-backed `vm start` now
fail honestly — the stage, the last probe error, tart's log tail, the macOS
guest limit — and leave no VM behind unless asked? Findings only; no fixes.

## Context

- The producer's commit is named by its handle, `start-failure-tart-k2`; its
  Decisions log (I1–I10) records what was settled and why.
- This changes a CLI contract that downstream scripts (Koine) depend on, which
  is why a review is scheduled before the QEMU leaf builds on the same seams.
- Where to look hardest, because the compiler cannot establish it:
  - **Liveness.** `process::child_alive` reaps with `waitpid(WNOHANG)` and falls
    back to `kill(pid, 0)`. Is it right against tokio's orphan queue (it can
    reap a child tokio still holds)? What does a recycled pid do? Is the
    decision-time process sample (`observe_tart_boot`, last step) ordered so a
    death during any stage is what the decision sees?
  - **The decision.** `start_outcome::decide_start`: is its priority order
    complete? Is there an observation that yields success without a Healthy
    agent? Is `Keep` ever returned for a VM whose process is gone, or `Teardown`
    leaked as a clone, a sidecar or a running `tart run`?
  - **Teardown and keep** in `lifecycle::settle_failed_tart_start` and
    `TartRunner::launch`: does every failure after `tart clone` remove the
    clone? Does a kept VM always have the meta `vm stop` needs?
  - **The errno path.** `AgentError::ConnectionRefused { message, os_errno }`
    and `ProbeError::from_agent_error`: is the errno mapped to the right kind
    on macOS, and does the variant change break any consumer's `code()` or
    message?
  - **The text.** The contract amendment, the `vm start` help, the
    connection-spec row, ADR-0014 and `CONTEXT.md` must describe the code as it
    is, including the stated limits: QEMU still degrades, and `--keep-on-failure`
    is honoured by tart only.
  - **The live script.** `scripts/live-start-checks.sh` once leaked VMs because a
    `$(...)` subshell lost its cleanup registration. Are there other paths where
    a failed check leaves a VM running?
- Known and accepted, not findings: the macOS 27 Local Network failure
  (`No route to host`, errno 65) that stops a Healthy-agent start on this host
  is k4's investigation, and the pre-existing clippy and rustfmt drift.

## Done when

- Findings are written for the integrating session to triage, each with the
  file and line it anchors to; a review that finds nothing worth acting on
  creates nothing and retires.

## Notes

## Findings

### F1 — [P2] Report a failed teardown instead of losing the remaining VM

Anchor: `cli-rs/crates/testanyware-vm/src/lifecycle.rs:652–654`.

The new default-failure path ignores the boolean returned by
`TartRunner::stop`, then removes both sidecars and returns the original boot
error. That boolean is false when the clone still exists after the attempted
stop/delete (`tart.rs:450–458`). For example, if `tart delete` fails, the
failed start leaves a clone behind, removes its management metadata, and gives
the caller no indication that teardown failed. `vm stop` subsequently requires
that missing meta (`lifecycle.rs:374–377`). This violates the new no-leftovers
contract precisely on the cleanup failure path. Preserve/report the remaining
resource and cleanup failure rather than treating an attempted teardown as a
completed one. Integration should exercise a failed stop/delete, including
reclaiming the remainder.

### F2 — [P2] A kept VM must have usable metadata before it is reported as kept

Anchor: `cli-rs/crates/testanyware-vm/src/lifecycle.rs:682–683`.

If the meta's atomic write fails (for example, the state volume fills or the
destination cannot be replaced), settlement only prints a warning and leaves
the VM running. The caller still returns the original `VM_BOOT_TIMEOUT` with
`details.kept: true` and remediation saying `testanyware vm stop <id>` will
reclaim it. However, `VmLifecycle::stop` returns `VM_NOT_FOUND` without a meta
file (`lifecycle.rs:374–377`). The stderr warning offers raw tart commands, but
the structured result and the documented keep contract remain false. Make the
meta persistence failure part of the result and settle the VM accordingly;
integration should cover a failed meta write on the keep path, including the
VNC-stage, meta-only case.

### F3 — [P2] Register failure-check VMs before running the command under test

Anchor: `scripts/live-start-checks.sh:154–161`; also `:137–147`.

The default-teardown check never adds `$torn` to `started_ids`. If the proxy
does not force a failure (for example, `NO_PROXY` bypasses it) and start succeeds,
the exit-code assertion aborts with a running VM that the EXIT trap cannot
stop. The same leak occurs when the very cleanup regression being tested
leaves `$torn` alive. The slot-exhaustion probe likewise never registers
`$probe`; if a holder exits and the third start unexpectedly succeeds, the
assertion aborts and only the two holders are cleaned up. Register known ids
before every such start, as the kept case already does at line 164. The
reliability mode's explicitly documented decision to retain its failed VM for
inspection is a separate, intentional behavior. Integration should verify
cleanup when the command unexpectedly succeeds and when an assertion fails.

### F4 — [P2] Remove stale sidecars when a same-id replacement fails to launch

Anchor: `cli-rs/crates/testanyware-vm/src/lifecycle.rs:255–256`;
`cli-rs/crates/testanyware-vm/src/tart.rs:427–442`.

A same-id start reclaims the old tart VM in `launch`, but leaves its spec/meta
in the state directory. If cloning, setting the new display, or spawning the
new run then fails, the `?` returns before `settle_failed_tart_start`, so neither
sidecar is removed. A concrete case is restarting an existing id with an
invalid plain `--display bogus` (the no-`@` form passes through the CLI parser):
`tart set` fails and the replacement clone is deleted, but
the old sidecars still advertise the deleted VM's endpoints and pid. This
breaks the new promise that a failed start leaves no sidecars and can direct
later commands at obsolete endpoints. Reconcile sidecars on the launch-error
path as well as on readiness failures; integration should cover an existing id
followed by an early replacement failure.

## Review evidence and limits

- Inspected producer commit `c7c3fe32e440de7eba457dcafa5c4ab45de234c1`
  (`start-failure-tart-k2`) against its parent, with the current source equal
  to that committed artifact at review start.
- Tier 2 graph verification: project
  `Users-antony-Development-TestAnyware.gh-issue-3`, fast-index generation
  `2026-10-17T11:32:00Z`. Symbol searches completed their relevant pages;
  decision, launch and health-wait callers/callees were traced and material
  functions were read exactly. Graph call resolution contains omissions and
  false positives around cfg-gated functions and common method names, so the
  findings rely on the actual source/control flow, not absence of graph edges.
- Coverage checked for all inspected repository evidence paths. No recorded
  gap or stale metadata was reported for the Rust evidence. `docs/` and
  `scripts/` are excluded by the fast index and were inspected directly.
  Coverage is best-effort, not a proof of completeness.
- Inspected the decision and probe tests, the help/contract tests, changed
  documentation and the live-check script. The decision's success branch
  requires `AgentWait::Healthy`; the documented QEMU degradation and
  tart-only keep flag match this increment. Tokio 1.50.0's installed
  `process/unix/{orphan,reap}.rs` confirms that an externally reaped child's
  `ECHILD` is removed from the orphan queue. This does not establish process
  identity after pid reuse, and the PID-only liveness check should not be read
  as such a guarantee.
- The producer's commit records passing live slot-exhaustion and keep checks,
  plus the errno-65 limitation carried to k4. Those runs were not reproduced.
  No test, build, lint, format or live-VM commands were run in this
  inspection-only review. The findings above are source-derived failure paths.

## Decisions (running log)

**R1 — Cleanup ownership is the actionable gap.** The four findings above
concern settlement, early launch failure, and the live script's ownership of
VMs on assertion failure. They belong to integration of this producer. The
known Local Network problem and pre-existing formatting/lint drift remain
outside this review's findings, as chartered.
