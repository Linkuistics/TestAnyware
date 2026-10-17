# start-failure-qemu-k3

## Goal

Bring the QEMU backend (Windows and Linux-on-qcow2 guests) onto the same
honest-start rule as tart, through the start-outcome decision that
`start-failure-tart-k2` built.

## Context

- Decisions in the retired `plan-k1`: D4 (one rule on every backend), D2, D3,
  and D6's live check (e).
- Builds on `start-failure-tart-k2`: the agent-probe outcome, the
  start-outcome decision, and `--keep-on-failure`.
- `plan-k1` fact 8: today the QEMU arm sends 120 probes 5 s apart to the
  localhost agent forward, then warns and exits 0.

## Done when

- The QEMU arm runs through the start-outcome decision:
  - QEMU process liveness is checked during the agent wait, and a QEMU exit
    yields `QEMU_FAILED` with QEMU's log tail;
  - an agent that never became healthy yields `VM_BOOT_TIMEOUT`;
  - teardown or keep follows k2's definition.
- Unit tests at the decision seam cover the QEMU observations.
- The contract text states the rule for every backend, and no tart-only
  wording from k2 remains.
- Live: one Windows golden `vm start` reaches a Healthy agent (seam check e).
  Killing QEMU during the agent wait returns `QEMU_FAILED` at once and leaves
  no VM.

## Notes

- The 10-minute QEMU agent window is deliberately long because Windows boots
  slowly; keep it unless evidence says otherwise.
- The Windows host harness waits for `accessible: true` after `vm start`. That
  stays its own concern: a Healthy agent need not be accessible.
