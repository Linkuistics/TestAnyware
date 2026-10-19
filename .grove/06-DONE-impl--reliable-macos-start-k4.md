# reliable-macos-start-k4

## Goal

Find why macOS starts on the macOS 27 host fail while the guest's agent is in
fact healthy (issue #3, finding 1), fix it, and prove the fix against the
grove's done line: 10 consecutive healthy starts and a passing Koine
verification run.

## Context

- Decisions in the retired `plan-k1`: D1 (the done line), and D6's live
  checks (c) and (d).
- The diagnostics `start-failure-tart-k2` added: every failed start reports
  its stage and its last probe error with the OS errno, `--keep-on-failure`
  keeps the VM for a post-mortem, and the Taskfile's reliability loop repeats
  starts.
- The root brief's Notes list the hypotheses the first failure's error kind
  should tell apart, and what k2 observed: the first failure was already
  `No route to host (os error 65)` while `/usr/bin/curl` reached the same
  healthy agent — start there.
- Koine: `scripts/vm-verify-lib.sh` (`start_vm`, `guest`) and
  `task app:vm-verify-homebrew`, in `~/Development/Koine`.

## Done when

- The cause of finding 1 is identified from evidence — the probe's error kind
  plus a post-mortem of a kept VM — and fixed, with a regression test at the
  nearest seam where one exists.
- The Taskfile reliability loop passes: 10 consecutive
  `vm start --platform macos` reach a Healthy agent.
- Koine's `task app:vm-verify-homebrew` passes once.
- A healthy macOS start also shows the resolution switch applied
  (`testanyware screen size` reports 1920×1080). `start-failure-tart-k2` could
  not check this live: from its harness no start reached a Healthy agent, so
  the criterion "a normal macOS start still succeeds with the switch applied"
  was handed to this leaf.
- If the failure never reproduces with the diagnostics in place, the leaf says
  so plainly — bar met, root cause unknown — instead of claiming a cause.

## Notes

- Cloning and starting a VM is cheap, so repeat freely, but stop every VM you
  start: the macOS guest limit is two.
- Leave #2 (the VNC-idle crash) alone unless the evidence points at it.

## Decisions (running log)

**Q1 — the cause of finding 1: a non-platform process is denied the guest subnet.**
Measured on 2026-10-19 (macOS 27.0.1, tart 2.40.1) from this harness, against a
guest whose `/health` answered 200 to `/usr/bin/curl`:

- Platform-signed tools connected to `192.168.64.x:8648`: `/usr/bin/curl`,
  `/usr/bin/nc` and `/usr/bin/perl` (`Platform identifier=26`).
- Non-platform tools got `No route to host` (errno 65): Homebrew `curl`, the
  Xcode `python3` that `/usr/bin/python3` shims to (Apple team-signed, but not a
  platform binary), and the Rust CLI.
- The decisive control: a copy of `/usr/bin/nc` re-signed ad hoc is the same code
  and fails with errno 65 to the guest, while it connects to a loopback listener
  and to the host's own LAN address. Only the signature changed.
- A byte-copy of a platform binary without re-signing is killed by the kernel
  (exit 137), so the copy must be re-signed to run at all.

So the property is "is the connecting executable a platform binary", not the
parent chain: every process here shares one `zsh` ancestry. This fits macOS
Local Network privacy: the harness chain ends at a launchd service, so no app
exists that the user can allow. It is not proven that Terminal.app or the
user's terminal is allowed. The earlier hypotheses (stale lease, late agent,
proxy variable, idle VNC) were already excluded by k2's evidence.

**Q2 — the product diagnoses the denial; it does not route around it.** The
user's standing instruction (memory `project-harness-local-network-blocked`) is to
fix the harness's access, not to work around it in product code. A relay through
`/usr/bin/nc` was therefore rejected. Instead, when the agent probe has seen
"host unreachable" five times running, the wait asks `/usr/bin/nc -z` to reach the
same endpoint. If `nc` connects, the CLI's own refusal is proven to be
process-specific, and `vm start` fails at once with `VM_BOOT_TIMEOUT`, cause
`local_network_denied`, instead of waiting 118 s. If `nc` fails, the wait carries on
unchanged (an unanswered ARP also gives errno 65 while a guest boots). QEMU
guests are on loopback and never ask.

**Q3 — Q2 is built and checked; the two live bars stay unmet.** Built:

- `health.rs`: `wait_for_agent_with_oracle`, a new outcome
  `AgentWait::LocalNetworkDenied`, and `wait_with_probe`, the loop over an
  injected probe. `wait_for_agent` keeps its signature for QEMU's loopback.
- `local_network.rs`: `platform_tool_reaches`, which runs `/usr/bin/nc -z`.
- `error.rs`: `BootCause::LocalNetworkDenied`, `details.cause`, the message
  clause and the remedy. The code and the exit stay `VM_BOOT_TIMEOUT` and 7.
- `start_outcome.rs`: the new arm of `decide_start`; `lifecycle.rs` wires the tart
  arm to the oracle.
- ADR-0019, the contract amendment of 2026-10-19, the `Local Network denial` entry
  in `CONTEXT.md`, the evidence note under `docs/research/`, and the
  `task vm:local-network` live check.

Checked: eight new unit tests, all 28 test binaries green, clippy at k2's baseline,
`task scripts:check`, and the controlled-English linter on what I wrote. Live, from
this harness: `vm start` ended in 21.7 s with `cause: local_network_denied` after 5
probes (it ran 118 s before), and `task vm:local-network` passed in 19 s. Both left
no VM.

Not met, and why. Done-when asks for 10 healthy starts, Koine's run and a 1920×1080
screen from a healthy start. Each needs a shell that macOS Local Network privacy
allows onto `192.168.64.x`, and this harness has none (Q1). I cannot make that
true from inside the session. The decision for the human is how to run those
checks: see the hand-back in the session's last message.

**Q4 — the live bars are met, through Terminal.app.** The user chose the Terminal
route ("Try Terminal via open"). I wrote `.command` scripts that write their results
to files and ran them with `open -a Terminal`. Terminal.app is allowed on this host
(no prompt). Results, 2026-10-19, with the debug build of this branch:

- `task vm:local-network`: `this shell may reach guests`, a Healthy agent in 14 s.
- `task vm:start-reliability N=10`: 10 of 10 consecutive starts reached a Healthy
  agent, in 13 to 16 s each, the display switch applied each time.
- `testanyware screen size` after a healthy start: `1920x1080`.
- Koine's `task app:vm-verify-homebrew` passed. Its first run failed on Koine's own
  check (`the cask installed 0.3.0, not this tree's 0.2.0`): Koine's checkout is a
  detached `HEAD` at 0.2.0 content, and the published cask is 0.3.0. I read that
  without changing Koine and ran the task against `git archive v0.3.0` instead,
  with `.build/app/Koine.app` symlinked in because the library insists on a built
  bundle. The run waited out two other sessions' VMs (the macOS guest limit
  refused it ten times, exit 6, naming the holders) and passed on try 11.

Also learned: `require_no_running_vms` was stricter than the reliability loop
needs, because another session's VM held a slot. The loop and `local-network` now
use `require_free_slot`. The slot-exhaustion and keep-on-failure checks keep the
strict precondition.

The cause is the context, not the Host CLI (Q1). The product change is the
diagnosis (Q2): it turns the 118 s unexplained failure into a 20 s explained one.
No VM of mine remains. The Koine checkout stays at 0.2.0 for the user to move.
