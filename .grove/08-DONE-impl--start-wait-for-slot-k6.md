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

## Decisions (running log)

- **Retry `tart run` on one clone.** `TartRunner::launch` splits into `prepare`
  (reclaim, clone, set display) and `run`. A `--wait` start prepares once and
  re-runs; the clone is removed only when the start finally fails (D3). The
  refusal is the authoritative signal, so each retry is a real `tart run`.
- **Units and default.** `--wait[=<seconds>]`, seconds as in the other timeout
  flags, `=` required so a bare `--wait` cannot swallow the next argument.
  Bare means 600 s: bounded, so an unattended driver cannot hang.
- **Retry cadence.** 3 s between tries; the last pause ends at the deadline so
  one final try is made at the timeout. Pure `next_slot_step` pins this.
- **Reporting.** `SlotWaitReporter` (pure) speaks on the first refusal, when the
  holders change, and every 30 s; stderr only, the `--json` envelope stays final.
- **No host-wide queue.** Waiters race; whichever retries first after a slot
  frees wins. A lock would serialise starts the limit does not, and the help
  text and contract say so.
- **Interrupts.** SIGINT/SIGTERM during a `--wait` start removes the clone
  (and the running `tart run`) and then dies by the signal, so a driver's
  `timeout` cannot leak an idle clone. Scoped to `--wait`; the default start's
  signal behaviour is unchanged.
- **Wait budget covers refusals only.** Boot time after the slot is won does
  not count against `--wait`.
- **Live (macOS 27, via Terminal .command; another session held one slot):**
  `scripts/live-start-checks.sh slot-wait` passes: a wait runs out in 9 s with
  exit 6 naming the holders and no clone; SIGTERM mid-wait exits 143 with no
  clone; a holder stopping lets the waiting start begin within 0–2 s and reach a
  Healthy agent, with one clone throughout. Recipe: `task vm:slot-wait`.
