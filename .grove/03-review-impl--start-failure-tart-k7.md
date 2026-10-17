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
