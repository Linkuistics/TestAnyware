# TestAnyware.gh-issue-3 — brief

## Goal

Make `testanyware vm start` honest and reliable (Linkuistics/TestAnyware#3).
It succeeds only once its guest has a **Healthy agent**. When it cannot, it
fails with the reason — the stage that failed, the last agent-probe error, the
backend's own log tail — and leaves no VM behind. And macOS starts on the
macOS 27 host stop failing while the guest's agent is in fact healthy, so
Koine's release verification can run again.

## Done when

- `vm start`, on every backend, exits non-zero instead of writing an
  agent-less spec:
  - `VM_BOOT_TIMEOUT` (exit 7), naming the stage and the last probe error, when
    the VM is alive but its agent never became healthy;
  - `TART_FAILED` / `QEMU_FAILED`, carrying the backend's log tail, as soon as
    the backend's process exits;
  - `VM_LIMIT_REACHED` (exit 6), naming the VMs holding the slots, when the
    **macOS guest limit** refuses the start.

  A failed start leaves no VM unless `--keep-on-failure` was given.
- 10 consecutive `vm start --platform macos` on the macOS 27 host reach a
  Healthy agent, and Koine's `task app:vm-verify-homebrew` passes.
- `vm list` shows every VM TestAnyware started, whatever its id.
- `vm start --wait[=<timeout>]` waits out a full macOS guest limit instead of
  failing.
- The CLI design contract, the connection spec, ADR-0014 and `CONTEXT.md`
  describe the new behaviour, and the root `Taskfile.yml` carries the recipes
  the work runs.

## Decomposition

Five `impl` leaves. The first three are the main line; the last two trail it
so neither delays the Koine unblock.

1. `start-failure-tart-k2` — the shared seams (the agent-probe outcome, the
   start-outcome decision), the tart arm, `--keep-on-failure`,
   `VM_LIMIT_REACHED`, the contract text and the first Taskfile. First,
   because every later leaf builds on its seams and the investigation needs
   its diagnostics.
2. `start-failure-qemu-k3` — the QEMU arm onto the same decision. Second, so
   the rule is one rule on every backend before the investigation starts.
3. `reliable-macos-start-k4` — root-cause finding 1 with the new diagnostics,
   fix it, and meet the 10-start bar and the Koine run.
4. `vm-list-sidecar-vms-k5` — `vm list` also lists VMs by their sidecars.
5. `start-wait-for-slot-k6` — `vm start --wait`.

## Pointers

- The settled decisions and their evidence: the retired requirements leaf
  `plan-k1` — its Decisions log (D1–D12) and its Notes (facts 1–10). Each
  leaf names the decisions it implements.
- Issues: #3 (the human's words). #2, the tart crash in `_VZVNCServer` after
  about 60 s of VNC idle, is a different bug. #4 and #5 (more Macs; placement
  by OS and architecture) are outside this grove.
- ADRs: ADR-0010 (host-side framebuffer); ADR-0014 (its resolution switch
  rides "the optionally-degraded agent contract" — reworked in place by k2);
  ADR-0004 (per-command connections).
- Contract: `docs/architecture/cli-design-contract.md` — §4.2 (lifecycle
  codes), §5 (exit codes; 6 becomes used), §7 (help text lists exit codes);
  `docs/reference/connection-spec.md` (the `agent` row).
- Glossary terms in play: Host CLI, In-VM agent, Healthy agent, Golden image,
  macOS guest limit, Host-side framebuffer.

## Test seams (agreed with the human)

1. **Start-outcome decision** — a new pure function at the highest testable
   point. From what the boot observed (VNC URL found; backend process alive or
   exited, with its log tail; guest IP found; the agent-probe outcome with its
   last error kind) it yields success-with-agent, or the exact error, plus
   teardown or keep. Unit tests with no tart or QEMU; the strict rule,
   teardown, the keep flag and `VM_LIMIT_REACHED` are pinned here.
2. **Probes** — the existing unit-test seams, extended. `wait_for_agent` keeps
   its wiremock and dead-port tests but returns an outcome (healthy; the last
   error with its OS errno; VM exited), with liveness injected. The VNC-URL
   poll keeps its temp-log tests, and a dead process returns at once with its
   log tail.
3. **Live, on the macOS 27 host** — (a) with both macOS guest slots taken,
   `vm start` fails within seconds with `VM_LIMIT_REACHED`, exit 6, naming the
   holders, and leaves no clone; (b) `--keep-on-failure` leaves a VM that
   `vm stop` reclaims; (c) 10 consecutive macOS starts reach a Healthy agent;
   (d) Koine's `task app:vm-verify-homebrew` passes; (e) one QEMU (Windows)
   start reaches a Healthy agent.

## Notes

- **Where the rule is written down.** The strict rule lands as an amendment
  to the CLI design contract, the contract's usual way of recording a change,
  rather than as a new ADR. The connection spec's `agent` row and `CONTEXT.md`'s
  example dialogue ("warns and proceeds with `agent: null`") change in the same
  leaf as the code that changes the behaviour, never ahead of it.
- **Finding 1's hypotheses** (for k4; the agent-probe outcome should tell them
  apart on the first failure). The ~126 s starts mean every probe failed fast
  rather than timing out (plan-k1 fact 4), so the 2 s per-request budget is an
  unlikely cause. Candidates:
  - a stale `tart ip` lease: the address came back 9 s after the start, early
    for a macOS guest's DHCP, and the `running` gate does not guard it;
  - macOS Local Network privacy refusing the calling process (an immediate "no
    route to host"; permission belongs to the responsible app, which could
    differ between a manual `curl` and a scripted `vm start`);
  - the agent coming up after the 120 s window on this host;
  - an interaction with the idle VNC server (#2);
  - a proxy environment variable, which reqwest honours and curl does not
    (uppercase `HTTP_PROXY`).
- **What k2's diagnostics showed (2026-10-17, macOS 27.0.1, tart 2.40.1).**
  Finding 1 reproduced on the first start through the new code, and the
  agent-probe outcome settled the hypotheses in one run:
  `VM_BOOT_TIMEOUT`, stage `agent`, 60 attempts over 118 s, last error
  `No route to host (os error 65)` — `EHOSTUNREACH`, not refused and not a
  timeout. That is the Local Network signature. The guest's agent was healthy:
  against the same kept VM, from the same shell, at the same moment,
  `/usr/bin/curl` returned HTTP 200 and `/usr/bin/nc` connected, while the
  Rust CLI (the debug build *and* the installed `/opt/homebrew/bin/testanyware`),
  Homebrew `curl`, `wget` and both Pythons failed with errno 65. So the
  restriction looks **per executable** — it spared `/usr/bin/curl` and
  `/usr/bin/nc` but not ad-hoc-signed binaries, nor even Apple's
  `/usr/bin/python3` — rather than per responsible process (all of these ran
  under one shell). The evidence does not fit a stale `tart ip` lease (the
  address was right), a late agent (curl got 200 minutes in), or a proxy
  variable (none set in that environment; `HTTP_PROXY` was only used later to
  *force* failures). It does not say *which* property of `curl`/`nc` earns the
  exemption. k4 should start from here: what lets a
  non-platform CLI reach a guest on `192.168.64.x` on macOS 27 (signing,
  an entitlement, a different transport, or routing the agent calls through
  something already permitted), and whether `vm start` should detect and
  explain the denial. The VNC side is loopback and unaffected. The 10-start
  bar and Koine's run can only pass once that is solved.
- **Hazards.**
  - A dead `tart run` can be a zombie of the still-running CLI, and
    `kill(pid, 0)` reports zombies alive — check tokio's orphan reaping
    against its source, or reap explicitly.
  - The OS errno is in the reqwest error's `source()` chain, not its
    `Display`.
  - The VNC-URL and IP polls block the async runtime with thread sleeps.
  - tart's limit text is tart's rewording of Virtualization.framework's code 6:
    match its stable part and keep the raw text in the details.
- **Closing #3** is the human's call once the done line is met.
