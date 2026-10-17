# plan-k1

## Goal

Establish, in the human's own words, what this grove builds for GitHub issue
Linkuistics/TestAnyware#3 — "macOS guest: vm start exits 0 with no agent in the
spec, and hides tart's failure reason (not the #2 crash)" — and decide how the
work is cut.

## Context

- The human's words: the body of issue #3
  (`gh issue view 3 --repo Linkuistics/TestAnyware`). Related, and explicitly
  not this grove: #2 (tart crashes in `_VZVNCServer` after ~60 s of VNC idle).
- The code the issue names: the tart start path in `testanyware-vm` — the
  lifecycle's tart start, `health::wait_for_agent`, `TartRunner::start` and its
  VNC-URL poll.
- The downstream consumer: Koine's `scripts/vm-verify-lib.sh` (`start_vm`,
  `guest`, `cleanup`).

## Done when

- The open questions are settled with the human and logged below.
- The root brief states the grove's goal, done-when, decomposition and the
  agreed test seams, and the next leaves are cut.

## Notes

Facts read from the code this session (not decisions):

1. **VNC-URL poll** — 60 polls 1 s apart; never checks whether `tart run` is
   still alive; on timeout it kills the pid, deletes the clone and returns
   `VmBootTimeout { id }` with none of tart's log text. That is finding 3's
   ~59 s and its missing reason.
2. **IP poll** — 30 polls 2 s apart, gated on `tart list` reporting
   `running`. The issue saw `running` from the first poll, so the gate does not
   guard against a stale `tart ip` lease (the "tart ip lies" memory).
3. **Agent wait** — 60 probes, 2 s apart, 2 s per-request timeout, returns a
   bare `bool`. Beneath it, the agent client maps every connect error
   (refused, unreachable) to `ConnectionRefused`, and reqwest's `Display` drops
   the OS errno, so even `AgentError` cannot tell refused from no-route.
4. **Timing evidence** — a ~126 s start is ~9 s to an IP plus ~118 s of
   2 s sleeps, so all 60 probes failed almost instantly; had they timed out the
   start would have taken ~245 s. The 2 s per-request budget is therefore an
   unlikely cause of finding 1: the failure is a fast error.
5. **Liveness hazard** — during `vm start` the detached `tart run` is still the
   CLI's own child; if it exits it can linger as a zombie, and `process_alive`
   (`kill(pid, 0)`) reports zombies alive. Verify tokio's orphan reaping
   against its source before relying on a liveness check.
6. **The contract today** — `VM_BOOT_TIMEOUT` (exit 7) is defined as "VNC
   didn't become reachable within the boot window"; `TART_FAILED` (exit 1)
   carries tart's stderr in `details.tart_error`. `connection-spec.md` says
   `agent` is absent "if the boot wait timed out". ADR-0014 rides "the
   optionally-degraded agent contract" (agent-unreachable skips the resolution
   switch). `CONTEXT.md`'s example dialogue says the CLI design contract
   "warns and proceeds with `agent: null`", but the contract document carries
   no such sentence — the behaviour was ported from Swift `startQEMU`.
7. **Consumers** — every in-repo `vm start` caller (the live-VM gate, the Linux
   and Windows host harnesses) needs an agent and treats its absence as
   failure; the agent-less Linux HUT is cloned through `testanyware_vm::tart`
   directly, not `vm start`. Koine's `start_vm` discards stdout and goes
   straight to agent calls. No known consumer relies on exit 0 without an agent.
8. **QEMU** has the same degradation (120 probes 5 s apart, warn, exit 0).
9. **`vm list`** finds running tart clones only through `tart list` filtered by
   the `testanyware-` prefix. Koine's ids are `koine-verify-$$`, so Koine VMs
   never appear in `vm list`, spec or no spec — the issue's "because its spec
   was never written" does not match the code.
10. **HTTP stack** — reqwest 0.12 with default features off: no macOS
    system-proxy lookup, but proxy environment variables are honoured.

## Decisions (running log)

**D1 — the done line is reliable starts, not just honest failure.** The grove
first makes `vm start` failures honest (tart's reason, each agent probe's error
kind, a non-zero exit), then uses that evidence to root-cause finding 1 on the
macOS 27 host and fix it. Done when `vm start` succeeds reliably on that host
and Koine's `task app:vm-verify-homebrew` can run. Honest failure alone would
leave Koine blocked; reporting goes first because the per-probe error kind is
the cheapest discriminator between finding 1's hypotheses.

**D2 — strict: no healthy agent, no successful start.** `vm start` succeeds
only when the agent reached health; otherwise it exits non-zero with
`VM_BOOT_TIMEOUT` (exit 7), and the error names the stage that failed and the
last probe error. "Fail only if the VM died" was rejected because finding 1 —
Koine's case — is the alive-VM case and would still exit 0; an opt-out flag
was rejected as CLI surface for a use nobody has (fact 7).

**D3 — a failed start tears its VM down, unless asked to keep it.** By default
a failing `vm start` stops and deletes its clone and leaves no sidecars, so no
VM slot leaks (the leak behind finding 3). An opt-in flag (working name
`--keep-on-failure`) instead leaves the VM running with its sidecars, so it can
be inspected and later reclaimed by `vm stop`; the finding-1 investigation uses
it. "Always tear down" was rejected because the live post-mortem is what
produced finding 1's evidence; "leave it running" because callers without a
cleanup trap would leak slots.

**D4 — the rule is `vm start`'s contract, on every backend.** D2's strict rule
and D3's teardown-or-keep apply to the QEMU backend (Windows, Linux-on-qcow2)
as well as tart, so the contract states one rule rather than a backend
exception. The QEMU arm gets the same probe-outcome type, liveness check and
teardown; verifying it costs one live QEMU start.

**D5 — keep tart; the two-VM limit goes to a multi-host follow-up.** (Raised by
the human mid-grilling: replace tart to get past the two-VM limit?) The limit is
Apple's, not tart's: the macOS licence (§2B(iii) of
`/Library/Documentation/License.lpdf`) grants "up to two (2) additional copies
or instances" per Apple-branded computer, and Virtualization.framework enforces
it as `VZErrorVirtualMachineLimitExceeded` (code 6), which tart merely rewords.
Every macOS guest on Apple Silicon runs through that framework, so no tart
replacement lifts it. This grove keeps tart; running more than two macOS guests
at once is a separate multi-host project, filed as Linkuistics/TestAnyware#4.

**D6 — the agreed test seams.** (1) A new pure *start-outcome decision* at the
highest testable point: from what the boot observed (VNC URL found; process
alive or exited, with its log tail; IP found; the agent probe outcome with its
last error kind) it yields success-with-agent, `VM_BOOT_TIMEOUT` carrying the
failed stage and last probe error, or `TART_FAILED` / `QEMU_FAILED` carrying
the backend's log tail when its process exited — plus teardown-or-keep (D3).
Unit-tested with no tart or QEMU; D2–D4 are pinned here. (2) The existing probe
seams, extended: `wait_for_agent` keeps its wiremock and dead-port tests but
returns an outcome (healthy, the last error including its OS errno, or VM
exited) with liveness injected; the VNC-URL poll keeps its temp-log tests and a
dead process returns at once with the log tail. (3) Live checks on the macOS 27
host: with both macOS slots taken, `vm start` fails within seconds with
`TART_FAILED` naming the slot holder and leaves no clone; `--keep-on-failure`
leaves a reclaimable VM; **10 consecutive** macOS starts reach a healthy agent
(D1's reliability bar); Koine's `task app:vm-verify-homebrew` passes; one QEMU
(Windows) start for D4. A fake `tart` on PATH was rejected (the fake drifts
from real tart, and PATH mutation forces serial tests); live-only was rejected
(D2–D4 would be checked by hand only).

**D7 — `vm list` shows every VM TestAnyware started, as a trailing leaf.**
`vm list` also lists VMs that carry TestAnyware sidecars, not only tart clones
whose names start with `testanyware-` (fact 9), so Koine's `koine-verify-*` VMs
and D3's kept VMs with custom ids are findable. It runs after the main line so
it never delays the Koine unblock. A separate issue was rejected because D3's
keep flag would otherwise create VMs `vm list` cannot see; keeping the prefix
filter as intended was rejected for the same reason.

**D8 — TestAnyware gets a root `Taskfile.yml`, grown by this grove.** The first
`impl` leaf adds it with only what the work needs — `cli-rs` test, clippy and
fmt; the live 10-consecutive-starts reliability loop and the two-slot
exhaustion repro (D6); thin wrappers over `scripts/release-*.sh` — and later
leaves run and extend those recipes rather than rebuilding command lines. It
stays local: no CI is reintroduced. go-task 3.54 is installed, and Koine
already uses one.

**D9 — configurable placement is its own issue, outside this grove.** At the
human's request, the generalisation of D5 is filed as
Linkuistics/TestAnyware#5: each guest runs on a machine chosen by OS and
architecture, under the invariant that placement never changes how a guest is
controlled — always the agent's HTTP protocol and the host-side VNC
framebuffer, never a host-specific channel. #4 becomes one case of it, and it
offers #1 (x86_64 under TCG) a native-x86_64 alternative. It records that the
agent's `/exec` has no authentication, so remote endpoints need an
authenticated, encrypted transport. Nothing in this grove depends on it.

**D10 — shared understanding confirmed; the leaves are cut as listed.** The
human agreed to the summary: tart failures → QEMU failures → reliable macOS
starts → `vm list`, with QEMU second so the rule is one rule across backends
before the investigation starts.

**D11 — the two-VM limit is surfaced explicitly, as "try again later".** (The
human, on confirming: "explicitly surface the two-vm limit so that anything
driving testanyware can wait".) A start refused for a full macOS slot gets its
own contract code, `VM_LIMIT_REACHED`, with exit code 6 — the contract's
reserved, so far unused "rate limited / try again later" bucket, which is
exactly this case — instead of `TART_FAILED`. Its details carry the platform,
the limit and the VMs holding the slots (tart's own "(other running VMs: …)"
list). A driver can therefore wait on exit 6 without parsing text. This
replaces D6's live check (a): the full-slot start must now fail within seconds
with `VM_LIMIT_REACHED`, exit 6, naming the holders, and leave no clone.

**D12 — drivers wait with `vm start --wait[=<timeout>]`.** Without the flag a
full-slot start fails at once with D11's error. With it, `vm start` blocks,
retrying the refused start, prints which VMs hold the slots while it waits, and
gives up with the same error at the timeout; the command returning is the
notification. Waiting and starting are one operation, so no other caller can
take the freed slot in between. Rejected: exit 6 alone (every driver would
duplicate the retry loop), a separate wait command (racy between the wait
returning and the start), push notification (scripts and agents cannot easily
wait on it, and it needs a long-lived watcher). It is its own leaf after the
main line, beside D7's, so it never delays the Koine unblock.
