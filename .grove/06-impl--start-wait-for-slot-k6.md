# start-wait-for-slot-k6

## Goal

Let a driver wait out a full macOS guest limit: `vm start --wait[=<timeout>]`
blocks until a slot frees and then starts, instead of failing with
`VM_LIMIT_REACHED`.

## Context

- Decisions in the retired `plan-k1`: D11 (the error and exit 6), D12 (the
  flag, and the alternatives it beat), D3 (no leaked clones).
- Builds on the `VM_LIMIT_REACHED` detection in `start-failure-tart-k2`.

## Done when

- Without `--wait` nothing changes: the start fails at once with exit 6.
- With `--wait`, `vm start` retries the refused start until it succeeds or the
  timeout passes, then fails with the same error. While waiting it reports on
  stderr which VMs hold the slots; the `--json` envelope stays the final
  result only.
- Retrying leaks no clone and does not re-clone needlessly.
- The contract and help text document the flag, its default timeout and its
  exit codes.
- Live: with both slots held, a `--wait` start begins within seconds of one
  holder stopping.

## Notes

- The authoritative signal is Virtualization.framework's refusal. `tart list`
  cannot see macOS VMs run by other tools, so a count of slots is a hint at
  most.
- Several waiters get no first-come-first-served order unless they queue on a
  host-wide lock; decide whether that is needed.
- An unbounded default wait can hang an unattended driver, so pick a bounded
  default unless the contract's conventions say otherwise.
