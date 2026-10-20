# app-vm-start-k27

## Goal

Run `vm start` in the app. The CLI sends the options and prints the progress and the result as it does today.

## Context

- `docs/specs/testanyware-app.md`, sections "The protocol", "Lifecycle and adoption" and "The Local Network diagnosis".
- ADR-0017, ADR-0018 and ADR-0019.
- The start, the health wait and the keeper code of the vm crate.
- The amendment of the CLI contract for `vm start --wait`, which writes progress to stderr.

## Done when

- `POST /v1/vms` answers with progress events as NDJSON. A result event or an error event ends the stream.
- The CLI prints the progress lines to stderr as today, and it prints the result with the same stdout and exit code. `--quiet` and `--json` keep their meaning.
- The app spawns `tart run`, QEMU, swtpm and the keeper detached, each in its own session. The keeper runs from the app's own executable.
- The app reaps each child that exits. A test shows that no zombie stays.
- When the client closes its connection before the result, the app cancels the start and cleans up as after a failed start. `--wait` ends its wait for a slot.
- The health wait keeps its rule of five `host_unreachable` probes and then the `nc` oracle. A denial ends the start with `VM_BOOT_TIMEOUT`, cause `local_network_denied`. The remedy names the TestAnyware app.
- Unit tests cover the cancellation and the denial with scripted probes.
- The leaf records one live start of a macOS guest through the app from Terminal.

## Notes

- Start `swtpm` as the direct path does today. The `terminate` option and the identity records come in `app-adoption-k14`.
- The direct path of Linux and Windows hosts keeps its remedy text for a denial. The text names the app only on a macOS host.
- The app accepts client sockets and spawns `tart`, QEMU and `swtpm` from several threads. On macOS a new socket gets close-on-exec in a second call, so a spawn on another thread can hand a child that socket. `app-protocol-core-k9` saw this in its tests. Spawn the detached children so that they hold no client socket, for example from one thread that owns the spawning, and test that a detached child holds no socket of the app.
- The `keep-on-failure` live check of `scripts/live-start-checks.sh` still has no fault. `cli-app-agent-routing-k24` could not port it: the check starts a real guest, and its fake guests are agents with no VM, while `vm start` stays direct until this leaf. A fault that starves the agent probes of a real start needs a hook on the app's start path, such as a test-mode argument. Add it here, and port the check.
- `app-default-routing-k26` changed the harness (`crates/testanyware-cli/tests/common/mod.rs`). `cli_direct` is now `cli_default`: it runs the CLI without `TESTANYWARE_APP_SOCKET`. A macOS host sends such a run to the default socket of `HOME`, so the tests that need the direct path of a Linux or Windows host check `HOST_GOES_DIRECT`. `TestApp::start_on_the_default_socket()` starts the app on that socket. Its `HOME` is a short directory in `/tmp`, because the default path adds 58 bytes and a socket path has 103 at most.
- `vm start` and the other lifecycle commands stay direct on macOS until this leaf. The ignored live harnesses (`live-vm-gate.rs` and the two host harnesses) start a guest with `vm start` and then run agent and VNC commands. Those commands now need the app running. Each file says so in its header, and none has run since the default changed.

## Decisions (running log)

- The start takes a `StartContext` (paths, tools, progress sink, cancel token, agent port, signal policy) in place of loose arguments. `VmLifecycle::start(opts, paths)` builds the direct context, so the direct path keeps its behaviour: process `PATH`, stderr progress, SIGINT/SIGTERM cleanup of a `--wait`. The app builds its own: built-in tool search, an NDJSON progress sink, a cancel token, and no signal handlers (a handler in the app would end the app). The rejected option was a process-global tool search, because parallel tests need different fake tools.
- `Progress` replaces each `eprintln!` on the start path (the display switch, the slot-wait lines, the settle warnings, the keeper warning). No new line appears, so the output stays "as today".
- The wire form of `POST /v1/vms` is a JSON body of the resolved options (platform, base, id, display, logical, keep_on_failure, wait_secs). The CLI generates the id and the default base, so the app reads nothing from the CLI. The response is `application/x-ndjson`: `progress` (a `line`), then one `result` (the data of the `vm start` output) or one `error` (the error object). A request that fails before the stream starts is a plain JSON error object with its HTTP status, as for the other routes.
- The app runs each start on its own thread with a current-thread runtime. The start calls `tart clone` and other blocking tools, and a worker of the shared runtime must not wait on them. The cancel token is the client's connection: the response body owns the receiver, and the token fires when hyper drops it.
- The detached children (`tart run`, QEMU, the keeper, and `swtpm`) start with `posix_spawn` and `POSIX_SPAWN_CLOEXEC_DEFAULT | POSIX_SPAWN_SETSID` on macOS. The kernel then closes every descriptor that a file action does not name, so no client socket can reach a child, whatever thread accepted it. A single spawning thread would not close the window, because `fork` copies the whole descriptor table. A reaper thread per child waits for it, in the CLI and in the app alike.
- `--quiet` has no flag in the CLI today (the contract lists it, the parser has none), so there is nothing to keep. The progress lines go to stderr in both output modes, as now.
- The test hook for the `keep-on-failure` live check is `--test-agent-port` of the app in test mode. It moves the agent probes and the agent endpoint of the spec to that port. A closed port starves the probes of a real start, as the old `HTTP_PROXY` did. A fake agent on loopback also uses it, so a fake `tart` can drive a whole start in the integration tests.
- hyper drops the response body when the client's connection goes away, even when the app has sent nothing for a while, so no heartbeat event is needed. A test kills the CLI mid-start and the app removes the clone, the process and the sidecars. The body owns the cancel token and cancels it in `Drop`. A start that has ended ignores a later cancel.
- A cancelled start always tears the VM down, and `--keep-on-failure` does not keep it, because no client is left to reclaim a kept VM. The start reads the cancel at each wait and after each step that runs to its end (`tart clone`, the QEMU launch, the sidecar writes). A cancel that arrives as the start succeeds stops the VM again.
- The error event carries `VmError::code()`, `to_string()`, `remediation()` and `details()`, and the CLI prints exactly those with the exit code of its table. It adds no default remedy from `remediation_for`, because that table gives `IO_ERROR` a hint that the direct command does not print.
- `StartCancelled` is a new `VmError` (`START_CANCELLED`). No client reads it, so it has no remedy and no event.
- The Local Network remedy names the TestAnyware app on a macOS host and keeps its old text on other hosts. `error.rs` selects it with `cfg(target_os = "macos")`.
- The denial is tested at the seams the leaf names: `wait_with_probe` with scripted probes (five `host_unreachable`, then the oracle) in `health.rs`, the decision in `start_outcome.rs`, and the remedy text in `error.rs`. A full start cannot fake `EHOSTUNREACH`, so no integration test claims it.
- `--platform linux` on a tart golden drives the integration tests. It takes the same tart arm as a macOS guest and skips only the guest display switch, which compiles Swift. The macOS guest runs in the live check.
- Mutation checks, each seen to fail and then restored: no cleanup in `abandon_tart` (3 vm tests fail), no reaper thread (the vm test and the app test fail), no cancel in `EventBody::drop` (the app test times out). The descriptor test has a control: a plain `fork` spawn hands the descriptor over, and `posix_spawn` does not.
- One start per VM id at a time. A start waits for the earlier start of its id, cleanup included (`gate_start.rs`), and a start that is cancelled while it waits never begins. Without it, a client that cancels and retries with the same `--id` starts while the first start still removes its VM, and the cleanup could delete the new clone. The wait polls every 100 ms, because a cancel token cannot wake a condition variable.
- The live check `keep-on-failure` runs its own app in test mode with `--test-agent-port 9` on the real state and data directories, and the CLI reaches it through `TESTANYWARE_APP_SOCKET`. Its first run failed on this Mac, for a reason worth keeping: the app that the check starts is a plain binary of a shell that macOS Local Network privacy denies, so the probe of the guest address got `host_unreachable` (`No route to host`) and not `connection_refused`. The old `HTTP_PROXY` fault probed loopback, which privacy never touches. The check now accepts either kind and says which it got, as `local-network` does for the two kinds of shell. Its precondition is one free slot instead of an empty Mac, because it starts one VM at a time.
- `slot-wait` and the other live checks that end a start by a signal now wait for the app's cleanup: `assert_vm_gone` polls up to `MAX_CLEANUP_SECS` (20 s). Through the app, the clone goes a moment after the CLI has ended.

## Live record (2026-10-20, this Mac, macOS 27 host)

The shell was the session of Claude Code, not Terminal. It has no Local Network access, so every success below is the app's grant at work, which is the point of the leaf. The development bundle (`TestAnyware Dev.app`, Developer ID of team TA43A4RUP3) already held its row.

- **Start.** `testanyware vm start --platform macos --id k27-live --json`, with no `TESTANYWARE_APP_SOCKET`, went to the default socket and returned in 34 s with exit 0. The stdout was the JSON of the direct command (`id`, `platform`, `base`, `vnc`, `agent` 192.168.64.3:8648, `spec_path`, `meta_path`). The two progress lines of the display switch arrived on stderr through the app.
- **Processes.** `tart run` was a child of the app (ppid = the app), the VNC keeper was `.../TestAnyware Dev.app/Contents/MacOS/testanyware-app _vnc-keeper --id k27-live ...`, the app had 0 zombie children, and neither child shared a Unix socket with the app.
- **Use.** `testanyware exec --vm k27-live 'sw_vers -productVersion'` printed `27.0.1` through the app. `vm list` listed the guest. `vm stop k27-live` removed it, with the sidecars.
- **Cancel.** `vm start --platform macos --id k27-cancel` was killed with SIGTERM as soon as tart reported it running (CLI status 143). The app removed the clone, ended `tart run` and left no keeper and no sidecar. Only the `.tart.log` remained, as after the direct command.
- **Live check.** `task vm:keep-on-failure` passed both cases against real tart (5 min): `VM_BOOT_TIMEOUT`, exit 7, stage `agent`, torn down by default, kept and reclaimable under `--keep-on-failure`.
- The app that I started (pid in the bundle) was quit afterwards. The `k26-live` guest of another session ran through all of this and still runs.
- **Denial, live.** `app-default-routing-k26` asked this leaf whether `nc`, as a child of the app, shares the app's state. From this shell, which macOS denies, a test-mode app started `vm start --platform macos --id k27-denied --json` against a healthy guest. It failed in 22 s with exit 7, `VM_BOOT_TIMEOUT`, stage `agent`, `details.cause` `local_network_denied`, `last_error.kind` `host_unreachable`, `kept` false, and the remedy "Allow TestAnyware in System Settings, Privacy & Security, Local Network. ..." The clone was gone. So in a plain denial the `nc` child of the app connects while the app is refused, and the diagnosis works from inside the start. The k26 case, an alert that is still pending, did not occur here, because no alert was open. A run that finds `host_unreachable` with `cause` null while an alert is open still needs a person.

## Review of this leaf (one in-session reviewer, the leaf's allowance)

The claim was that the cancel, cleanup and spawn code is correct under concurrency in an app that runs for days. A fresh context got the contract and the files, and a prompt to find what is wrong. It reported seven defects and no more. All seven were valid and actionable, and none was noise:

1. **A client that leaves after `start_in` returns.** The VM ran on and nobody had its id. The start thread now stops the VM again when the event cannot be sent, or when the cancel fired (`VmLifecycle::stop_with`, which takes the tool search).
2. **The cancel cleanup dropped the sidecars even when the teardown failed.** `abandon_*` now go through `settle_failed_start`, which writes the meta when the VM survives. A test with a `stuck` fake `tart` shows the handle stays and `stop_with` then removes the VM.
3. **A cancel during the display switch left `display-<id>/` with a compiled helper.** `abandon_tart` removes it.
4. **A stale pid after a refused slot attempt.** The reaper frees the pid at once, so a cancel during `--wait` could signal another process. `wait_for_slot` stores 0 before the retry sleep.
5. **A panic skipped every cleanup.** The start thread now stops the VM by id and removes the tart VM before it sends the `INTERNAL` event.
6. **A cancel that arrived as a start failed under `--keep-on-failure` kept the VM.** Both arms check the cancel on the error path and abandon the VM.
7. **`posix_spawn` kept an ignored SIGPIPE and the signal mask.** `std::process::Command` resets both, and so the new spawn resets both (`POSIX_SPAWN_SETSIGDEF` and `SETSIGMASK`). A test reads the child's SIGPIPE through Perl: the mutation without the flags prints `IGNORE`.

Nothing was left as a visible trade-off. After the fixes, the 44 test binaries pass, clippy shows no new warning, and a second live start of a macOS guest through the rebuilt development bundle took 25 s and ran `exec`. The reviewer found no defect in the FFI memory handling, the descriptor actions, the reaper race with `child_alive`, `StartTrack` atomicity, the gate, the event body and the CLI parity.

## Not verified

- The Linux and Windows builds. The toolchain here has no standard library for those targets, and I installed none. The new code is gated with `cfg(unix)` and `cfg(target_os = "macos")` in the places that need it, and a non-Unix `spawn_session` exists.
- A start that the person cancels with Ctrl-C in a terminal. The tests kill the CLI process, which is the same event for the app (a closed connection).
- The QEMU arm through the app (Windows or Linux-on-qcow2 guests). Its cancel and cleanup paths follow the tart arm and are not run by any test here.
