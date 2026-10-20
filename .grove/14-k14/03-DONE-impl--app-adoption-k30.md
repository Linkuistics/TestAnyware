# app-adoption-k30

## Goal

When the app starts, adopt each VM whose identity it can prove. Report the others, and signal nothing without proof.

## Context

- `docs/specs/testanyware-app.md`, section "Lifecycle and adoption", the flowchart and the rules.
- `vm-process-identity-k15`, which records the identity.
- `qemu-legacy-stop-k31`, which stops an older QEMU guest through its control sockets. It lands before this leaf.
- The requirement scenarios of "VMs outlive the app".

## Done when

- A pure function takes a meta and the live process facts. It returns one outcome: adopt, stopped, doubt, or legacy.
- At start, the app reads each sidecar and applies the function. It serves its socket first, and it probes the agent and the VNC banner of each survivor later, with a deadline. A timeout means not answering, never dead.
- `vm stop` signals a process only on a full match of the pid, the start time and the boot session.
- When the identity does not match, `vm stop` signals nothing, and it also removes neither the clone, the session directory nor the sidecars. It fails with `VM_STOP_FAILED` and a remedy.
- A legacy VM gets streams. The app stops a tart legacy guest by name with `tart stop` and `tart delete`. It stops a legacy QEMU guest with the stop of `qemu-legacy-stop-k31`.
- `GET /v1/status` and the menu report each VM that the app did not adopt, with the reason.
- A test shows that a meta with a reused pid signals nothing and reports the VM as not adopted.
- A test shows that an older QEMU meta reaches the stop of `qemu-legacy-stop-k31` and never the signal path.

## Notes

- Before this leaf, the shared teardown still signals a QEMU guest with identity. This leaf closes that path.

## Decisions (running log)

- **The decision looks at the main process only.** `adoption::decide` takes a meta and a `ProcessProbe` and returns `Adopt`, `Stopped`, `Doubt { pid, mismatch }` or `Legacy`. A meta with no `identity` is `Legacy`, whatever the helpers record. A pid that is gone is `Stopped`. A live pid whose start time or boot session differs, or a host that names no boot session, is `Doubt`. The keeper and swtpm are compared one by one at the moment of a signal (`verified_pid`), so an earlier decision never authorises a signal.
- **`is_older_qemu_meta` now means "no identity for QEMU itself".** A meta with only a keeper or swtpm identity proves nothing about QEMU, so it gets the stop that signals nothing. The test of `qemu-legacy-stop-k31` that pinned the three-field rule changed with it.
- **The guard sits in the shared stop, `VmLifecycle::stop_with`, as well as in the app's `stop_with_probes`.** A meta with an identity signals only on a full match on both paths. A meta with none keeps today's stop on the direct path (a Linux or Windows host), and gets no signal in the app: a QEMU guest through its sockets, a tart guest by `tart stop` and `tart delete` with pid 0 and no keeper signal.
- **A `Doubt` stop fails with `VmStopUnproven`, code `VM_STOP_FAILED`, exit code 1.** It signals nothing and removes nothing: not the clone, the session directory, nor the sidecars. The details carry `vm_id`, `pid`, `reason` and `records`. The remedy names `tart stop`/`tart delete` or a hand quit of QEMU, then the two records to remove. It is boxed, so `VmError` keeps its size.
- **A swtpm with an identity is stopped through its control socket, then by a verified signal.** `qemu::teardown_verified` replaces the `pgrep` backstop for a meta with identity, since `pgrep` signals a process found by its command line and not by proof. The `pgrep` backstop stays in `teardown`, for a failed start and for a meta with no identity on the direct path.
- **The VNC probe at adoption skips a tart guest with no keeper.** ADR-0018: a VNC connect 60 s or more after the last client left crashes tart on macOS 27. A probe of a tart guest that no keeper holds could be that connect.
- **The app adopts after it can answer.** `adoption::run` starts after the listener is bound and the accept loop runs, on a blocking task for the scan, then one task per probe. `GET /v1/status` gains an `adoption` object (`finished`, `adopted`, `not_adopted`). A reader that lacks it still reads the body (`#[serde(default)]`). A test shows the status answering while a probe still waits.
- **A probe has a deadline of 8 s, dial included, and a timeout means "not answering".** The agent probe sends `GET /health` and reads `HTTP/`. The VNC probe reads the 12-byte banner and sends nothing, so its close leaves no unread data (ADR-0018, the RST trigger). `--test-probe-deadline-ms` shortens the deadline in test mode.
- **The menu lists each VM that was not adopted,** under the heading "Not adopted (the app signals nothing):", with the reason as words.
- **The app's own cleanup after a start uses the no-signal stop.** `start.rs` called `stop_with` when a client left as a start finished, and after a panic. It now calls `stop_with_probes`, so a VM that a start could not give an identity stops by name or through its sockets.
- **The cleanup of a failed start keeps its signal.** `abandon_qemu`, `abandon_tart` and `QemuRunner::start_in` signal a process that the same call spawned and holds, so no pid is recycled in between. The brief's guard concerns `vm stop`.
- **The older-meta rule, the pgrep backstop and the direct path.** The direct path of the Host CLI on a Linux or Windows host keeps today's stop for a meta with no identity. That path has no sockets stop, and the brief confines it to the app.
- **Test seams.** Pure: `adoption::decide`, `check_process`, `verified_pid` with fake probes. Real processes and the real executable: `testanyware-app/tests/adoption.rs` (seven tests: adopt with both probes answering, a reused pid, a gone process, an older meta, no VNC probe for a tart guest without a keeper, a silent guest, status served while a probe waits) and the routing tests of `app-vm-stop-delete-routing.rs` (a reused pid for QEMU and tart, an older tart guest, with the existing older-QEMU tests for `qemu-legacy-stop-k31`). The guard's controls were seen to fail: with `check_process` always `Match`, five unit tests and two app tests failed.
- **No in-session reviewer.** Tests with real processes cover each claim, including that nothing is signalled.
