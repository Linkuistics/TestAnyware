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

## Decisions (running log)

**Q1 — settlement becomes backend-neutral, not copied.** k2's
`settle_failed_start` (with k8's fixes: surviving-clone meta, keep-needs-meta,
sidecar clearing) is the one place that knows teardown-or-keep. Rather than
fork it for QEMU, the backend-specific parts become inputs: the meta and
optional spec to write, a `teardown` closure, a `clone_exists` probe and a
manual-reclaim hint. Tart and QEMU each build those and call the same function,
so "QEMU follows k2's definition" holds by construction and k8's tests cover
both.

**Q2 — a QEMU arm observes the same `BootObservation`.** `QemuRunner::start`
already finds the VNC port and the agent forward (it fails with its own error if
either is missing), so the VNC and address stages are `Found`; only the agent
stage and the process sample are observed afterwards. `StartArtifacts` gains the
log path (the log lives inside the clone dir, which teardown deletes, so the tail
must be read first) and `agent_port` stops being an `Option` — `start` already
refused `None`.

**Q3 — QEMU's death is checked with `child_alive`, not `process_alive`.**
`spawn_detached` drops the tokio `Child`, so QEMU is the CLI's own child and an
exited QEMU is a zombie that `kill(pid, 0)` reports alive (k2's I2, same
mechanism). This applies to the agent wait, the final sample, and the existing
"did QEMU stay up after launch" check inside `QemuRunner::start`.

**Q4 — QEMU failures carry the log tail in `details.qemu_error`.** `QemuFailed`
had no details; it gets `{"qemu_error": …}` beside `TartFailed`'s
`{"tart_error": …}` so a driver reads the backend's reason the same way on both.
The early exit inside `QemuRunner::start` (QEMU gone within a second of launch —
the usual bad-argument or bad-firmware case) now carries the tail too, and so
does a monitor-discovery failure when QEMU turns out to have died.

**Q5 — the macOS-guest-limit match is tart-only.** `decide_start` ran
`parse_limit_refusal` on any backend's log tail. The limit is Apple's
Virtualization.framework's, so only a tart exit can be a limit refusal; a QEMU
log that happened to contain the phrase must stay `QEMU_FAILED`.

**Q6 — `VmStartResult.agent_unreachable` is removed.** Its only reader was the
CLI's degraded-start warning. With no degrading arm left it can never be true,
and a field that cannot vary is a lie waiting to be believed.

**Q7 — the 10-minute QEMU agent window stays** (120 probes, 5 s apart), per the
brief. No evidence says otherwise.

**Q8 — the agent wait notices a death between probes (a `health.rs` change).**
The first QEMU seam test failed: `wait_for_agent` asked `vm_alive` only before
each probe, so a QEMU that died just after a probe was found one interval later —
up to 5 s on QEMU, not "at once". The inter-probe sleep now runs in 250 ms
slices and re-asks liveness between them (`sleep_while_alive`). An interval of
one slice or less stays a single sleep, which leaves the existing test that pins
"liveness is asked before each probe" exact. Tart's 2 s interval gains the same
promptness.

**Q9 — an enumerate-then-classify sweep found two more documents stating the old
rule.** The targeted edits (CLI help, contract, connection spec, ADR-0014,
CONTEXT.md) missed `docs/reference/cli-commands.md` ("Tart-backed guests only")
and the `vm-start.json` output schema (`agent` nullable "only for a QEMU-backed
guest"). Both are updated; the schema's `agent` narrows to `object` because a
successful start now always has one and the dry-run receipt omits it. The
`cli-contract.rs` gate now also requires `QEMU_FAILED` in the help and refuses
the stale phrases. `website/docs/` stays a mirror that is not edited (k2's I10).

**Q10 — live, macOS 27.0.1 host, QEMU 11.1.2 (HVF), Windows golden.** What ran and
what it showed:

- *Honest failure, observed through the new code.* The first `task
  vm:qemu-start` start did not get a QEMU that stays up: `QEMU_FAILED` with
  `details.qemu_error` = `QEMU exited right after launch: qemu-system-aarch64:
  -device tpm-tis-device,tpmdev=tpm0: Error: ret = HV_BAD_ARGUMENT (0xfae94003,
  at ../accel/hvf/hvf-all.c:123)`. Before this leaf the only text was "QEMU did
  not remain running after launch". Nothing was left behind (no clone, session
  dir, sidecars, QEMU, swtpm). A later run with an over-long test id surfaced
  QEMU's "UNIX socket path … is too long" the same way.
- *The TPM abort is not this leaf's and not its doing.* Reproduced by hand with
  the host profile's arguments: `-machine virt,highmem=on,gic-version=3 -accel
  hvf -cpu host …` stays up; adding only `-device tpm-tis-device` aborts with the
  identical error. `build_qemu_args` adds the TPM device for every guest, so on
  this host **no** QEMU guest starts. This leaf did not touch the argument
  vector.
- *The strict rule, live.* With the three TPM arguments stripped from a local,
  uncommitted build (restored afterwards; sha256 of `qemu.rs` verified equal to
  the pre-experiment copy), a Windows start ran the whole agent wait and failed
  `VM_BOOT_TIMEOUT` (exit 7): stage `agent`, endpoint
  `http://localhost:<port>/health`, 120 attempts, 835 s, last error `timeout`,
  `kept: false`, and nothing left running. Before this leaf that start exited 0
  with `agent: null`. That guest never became healthy in 14 minutes, so this is
  not a Healthy-agent result; whether the missing TPM or the host is to blame is
  unknown.
- *Seam check (e), second sentence — passes.* `task vm:qemu-death`: `kill -9` of
  QEMU during the agent wait returns `QEMU_FAILED` (exit 1, QEMU's own log line in
  `details.qemu_error`) 0 s after the kill, and leaves no clone, session dir,
  sidecars, QEMU or swtpm — by default and under `--keep-on-failure`.
- *Seam check (e), first sentence — NOT met.* No Windows start reached a Healthy
  agent, because the committed argument vector cannot start QEMU on this host.

**Q11 — the QEMU agent window is 10–14 minutes, not 10.** The 835 s above is 120
probes × (5 s sleep + the 2 s request timeout a vanished packet costs). Q7's
"keep it" stands; the code comment, help text and contract now say what the
numbers are.

**Q12 — the leaf is left live, not retired.** Its Done-when line "one Windows
golden `vm start` reaches a Healthy agent" has not been seen to hold, and the
cause is outside this leaf (Q10). Retiring would claim a check that did not run.
Everything else in Done-when is met and verified; see the handback.

**Q13 — tart regression after the settlement refactor.** `task vm:slot-exhaustion`
on the same host: both slots filled by kept VMs, the third start refused in 1 s
with `VM_LIMIT_REACHED` (exit 6) naming both holders, no clone and no sidecars
left for it, and no VM left running afterwards. `task cli:test` (609 tests),
`task scripts:check` pass; clippy adds no warnings (only the two large-`Err` ones
k2's I8 recorded).
