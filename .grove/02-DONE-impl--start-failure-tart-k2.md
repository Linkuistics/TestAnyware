# start-failure-tart-k2

## Goal

Make tart-backed `vm start` (macOS and Linux-on-tart guests) honest: it
succeeds only with a Healthy agent, and otherwise fails with the reason and
leaves no VM. Build the shared seams that the QEMU leaf and the investigation
reuse.

## Context

- Decisions in the retired `plan-k1`: D2 (strict rule), D3 (teardown, and
  `--keep-on-failure`), D6 (the seams), D8 (the Taskfile), D11
  (`VM_LIMIT_REACHED`, exit 6).
- Facts in `plan-k1`'s Notes, 1–7: the start path's polls and their budgets,
  the timing evidence, the zombie-liveness hazard, the contract as it stands,
  and the consumers (none relies on an agent-less success).
- Code: the lifecycle's tart start, `health::wait_for_agent`,
  `TartRunner::start` with its VNC-URL and IP polls, `VmError`, and the
  `vm start` handler and its clap surface.

## Done when

- `wait_for_agent` returns an outcome rather than a `bool`: healthy; the last
  error, classified with its OS errno (refused, no route or host down,
  timeout, HTTP status, decode); or VM exited. Tested at its existing seam.
- The VNC-URL poll and the agent wait both notice a dead `tart run` at once
  and return the current run's log tail.
- A pure start-outcome decision maps what the boot observed to one of:
  - success-with-agent;
  - `VM_BOOT_TIMEOUT` (exit 7; details: stage, polled endpoint, last error,
    attempts, elapsed);
  - `TART_FAILED` (tart's log tail in `details.tart_error`);
  - `VM_LIMIT_REACHED` (exit 6; details: platform, limit, holders);

  each with teardown or keep. It is unit-tested with no tart.
- The tart arm runs through that decision. It never writes an agent-less spec.
  On failure the clone is stopped and deleted and no sidecars remain, unless
  `--keep-on-failure` leaves the VM running with both sidecars.
- The contract text is updated:
  - a CLI design contract amendment: `VM_BOOT_TIMEOUT` covers the agent stage,
    `VM_LIMIT_REACHED` is added, exit 6 is used, and the new flag appears in
    the `vm start` help text with its exit codes;
  - the connection spec's `agent` row;
  - ADR-0014's degraded-agent wording, reworked in place;
  - `CONTEXT.md`'s example dialogue.

  The `cli-contract.rs` gate still passes.
- A root `Taskfile.yml` holds what this leaf ran: the `cli-rs` test, clippy
  and fmt recipes, the live slot-exhaustion repro, the 10-start reliability
  loop k4 will use, and thin wrappers over `scripts/release-*.sh`. Each recipe
  is verified by running it.
- Live, on the macOS 27 host: seam checks (a) and (b) pass, and a normal macOS
  start still succeeds with the resolution switch applied.

## Notes

- Write the contract text for tart as it now behaves, not ahead of the code.
  k3 generalises it to QEMU.
- To fill both slots for check (a), start two macOS VMs, then start a third.
  Every refused attempt must leave no clone (D3), and check (a) proves it.
- If time allows, run the reliability loop once and write what it shows into
  the root brief's Notes for k4. Do not chase finding 1 here.
- This changes a CLI contract that downstream scripts (Koine) depend on, so a
  `review-impl` is a reasonable last act.

## Decisions (running log)

**I1 — tart's refusal text, observed live (macOS 27.0.1, tart 2.40.1).** A
third `tart run` with two macOS guests running exits **1 within a second**
and prints exactly `The number of VMs exceeds the system limit (other running
VMs: k2probe-b, k2probe-a)` (one line). So the limit is detected from the
exited process's log, not from a timeout; match the stable part `exceeds the
system limit`, parse holders from `(other running VMs: …)`, keep the raw
line in the details (brief hazard: tart rewords Virtualization.framework).

**I2 — liveness is `waitpid(WNOHANG)` first, `kill(pid, 0)` second.** Checked
against tokio 1.50 source (`process/unix/reap.rs`, `orphan.rs`,
`runtime/process.rs`): a dropped, still-running `Child` goes on a global
orphan queue that is drained only when the runtime's driver parks *and* a
SIGCHLD was observed. So a dead `tart run` can sit as a zombie, which
`kill(pid, 0)` reports alive, for an unbounded time. `child_alive` reaps it
itself; `ECHILD` (tokio got there first, or not our child) falls back to
`kill(pid, 0)`.

**I3 — the OS errno rides on `AgentError::ConnectionRefused`.** The errno is
in the reqwest error's `source()` chain, not its `Display` (plan-k1 fact 3), so
the conversion site is the only place it can be read. The variant becomes
`ConnectionRefused { message, os_errno }`, `message` carrying the whole chain.
Classification to refused / unreachable / timeout happens in `health.rs` from
`io::Error::from_raw_os_error(errno).kind()`, which is cross-platform.

**I4 — `--keep-on-failure` only keeps a VM that is still running.** When the
backend process has exited (tart's limit refusal, a crash) nothing is running
to inspect, and the log tail is already in the error, so the clone is
torn down whatever the flag says. Where the VM is alive (VNC, IP or agent
stage timed out) the flag leaves it running with both sidecars; the spec
names the agent endpoint when the guest IP is known, so commands against the
kept VM report the real connection error rather than "no agent".

**I5 — the existing sync `poll_vnc_url` / `poll_ip` stay.** The golden,
recovery and live-harness flows call them with their own error semantics and
are not part of #3. `vm start` gets liveness-aware async variants beside them
that share the log-scanning helper.

**I6 — a kept VM's sidecars are written as far as the boot got (refines I4).**
`--keep-on-failure` always writes the meta (it is all `vm stop` needs); the spec
only once a VNC endpoint is known, because a spec cannot exist without one.
So a VNC-stage timeout keeps a meta-only VM that `vm stop` still reclaims.

**I7 — the agent-stage failure is forced live by `HTTP_PROXY`, not by touching
an image.** reqwest honours `HTTP_PROXY` (plan-k1 fact 10); pointing it at a
closed port makes every agent probe fail while the guest is perfectly healthy,
so the live `--keep-on-failure` check needs no agent-less image (which would
mean modifying a VM image beyond what is essential). The same trick doubles as
a check that the probe error kind reaches the error details.

**I8 — fmt is never repo-wide.** `cargo fmt --check` reports ~650 pre-existing
diffs (there is no `rustfmt.toml`; the repo does not run rustfmt), and clippy
carries pre-existing warnings in `testanyware-rfb` plus two large-`Err` sites
that predate this leaf (`VmError` is pinned at ≤ 56 bytes by a test, so this
leaf did not cause them). So `task cli:fmt` checks only the files it is given,
and `task cli:clippy` reports without gating. Files this leaf created
(`start_outcome.rs`, the rewritten `health.rs`) are rustfmt-clean.

**I9 — QEMU keeps degrading until k3, and the text says so.** The contract
amendment, the help text and the connection-spec row are written for tart as
it now behaves and state plainly that QEMU guests still warn and exit 0. k3
removes those sentences when it generalises the rule.

**I10 — `website/docs/` is a mirror and is not edited here.** It already
differs from `docs/` (the CLI reference), and nothing in the repo regenerates
it; `docs/` is the source. Its `error-codes.md` and `connection-spec.md` are
now stale until whatever syncs them runs.
