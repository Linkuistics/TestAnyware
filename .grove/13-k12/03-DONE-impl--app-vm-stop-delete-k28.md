# app-vm-stop-delete-k28

## Goal

Run `vm stop` and `vm delete` in the app.

## Context

- `docs/specs/testanyware-app.md`, section "The protocol".
- The stop and delete code of the vm crate and the CLI.
- The harness of `cli-app-agent-routing-k24`.

## Done when

- `POST /v1/vms/{id}/stop` and `POST /v1/goldens/{name}/delete` return the results that the commands print today.
- With the routing on, the two commands ask the app. The text, the `--json` fields and the exit codes match the direct commands.
- A stop signals the same processes as the direct command does today. The identity guard comes later.
- A test with injected process control shows each outcome, including a missing VM.

## Notes

- After this leaf, every lifecycle command except golden creation goes through the app on a macOS host.

## Handed over by app-vm-start-k27

- `VmLifecycle::stop_with(id, paths, tools)` already stops a VM with the app's tool search, and `TartRunner::stop_in` takes the search too. `stop` delegates to it. The start uses `stop_with` when its client left as it succeeded.
- A start and a stop of one id should not overlap. The app serialises starts per id with `StartGate` (`testanyware-app/src/gate_start.rs`). A stop of an id that is starting is not gated yet.
- The detached children are reaped by threads of `detached::spawn_detached`, and `child_alive` still works beside them (`ECHILD` falls back to the signal-0 probe).
- Live checks that stop a VM by signal or `vm stop` see the app's cleanup a moment after the CLI has gone. `assert_vm_gone` in `scripts/live-start-checks.sh` polls for it.

## Decisions (running log)

- `vm stop` and `vm delete` ask the app through two small POST routes. A stop has no body. A delete has the body `{"force": bool}`. The results are the data that the commands print (`{id, stopped}` and `{name, deleted}`), and an error is the `AppError` object that the start route already uses. The client got `post_json` and the two methods `stop_vm` and `delete_golden`, so the path grammar stays in the protocol crate (`stop_delete.rs`: builder and parser together, with a round-trip test).
- `--dry-run` of both commands stays in the Host CLI and asks the app nothing, as `vm start --dry-run` does. It reads local files, and the app has nothing to add. A test shows a dry run works with no app.
- A VM id keeps the grammar of the guest stream routes and is never decoded. An id that is no plain id gets `VM_NOT_FOUND` from the CLI without a request, which is also what the app answers. The path builder cannot make a valid request from such an id.
- A golden name is percent-encoded in the path and decoded in the app. The rejected option was the plain-id grammar for goldens. A golden is a file name (`{name}.qcow2`), so `vm delete "my golden"` works today, and a stricter grammar would change a command that the brief says keeps its meaning. The app refuses a decoded name with `GOLDEN_NOT_FOUND`, and it does not look at the disk. A name is refused when it is empty, is `.` or `..`, holds `/` or NUL, or is longer than 255 bytes. The first version also refused a leading dot. The reviewer showed that this broke a hand-placed `.foo.qcow2`, which the direct command deletes. The tart golden names are all `testanyware-golden-…`, so none is affected. The integration test uses `sub/../../outside` with `sub` present, which is the one name that escapes only if the rule on `/` is missing. The first version of that test did not fail without the rule, because every name in it was already not found. A mutation showed it.
- The CLI prints every error that the app answers for these two routes exactly as the direct command does, with no default remedy. `remediation_for` adds a hint to `IO_ERROR` that the direct command does not print. That is the same finding as in `app-vm-start-k27`. The app sends the code, and the CLI maps it back to an exit code. A unit test checks that `exit_code_for(err.code()) == err.exit_code()` for each error that a stop or a delete can make.
- The vm crate got the tool-search variants that the app needs: `VmLifecycle::delete_with`, `tart::golden_exists_in`, `tart::delete_golden_in` and `QemuRunner::running_clones_backed_by_in`. The last one matters: `qemu-img` was found by `which`, which reads the process `PATH`. The app has none, so the "golden in use" check of a QEMU golden would have found no `qemu-img` and passed silently. On macOS `from_process_path` happens to add `/opt/homebrew/bin`, so no test could show it. The old functions delegate, and the direct path is unchanged.
- A stop takes the per-id permit of the start gate. So a stop waits for a start of that id, cleanup included, and for an earlier stop of that id. This closes the gap that `app-vm-start-k27` left open. The stop takes the permit inside `spawn_blocking`, with a token that never fires, because a stop has no client to cancel it. A delete takes no permit. It names a golden and not a VM, and nothing in the app starts or stops a golden.
- Both routes run on a blocking task that the client's departure does not cancel. A stop that has begun ends. The cost is that a stop queued behind a long start of its id holds one thread of the blocking pool until the start ends.
- The tests inject process control as real processes. The VM's processes are `sleep` commands that a shell orphans, so only the kernel reaps them, as with a VM that the app did not start. The test reads `kill -0` to see that the recorded pid is gone and that a bystander `sleep` is not. The tart backend is a fake script that logs each call. So a test sees `tart stop`, `tart delete` and their order.
- `cli-contract.rs` ran `vm stop` and `vm delete` with no socket. On a macOS host those now go to the default socket, so the test starts a test-mode app on Unix hosts, as the `vm list` test does.
- Mutation checks, each seen to fail and then restored:
  - The app's stop does not call the lifecycle code. Eight tests fail.
  - The CLI prints a stop error with the default remedy. The `IO_ERROR` test fails.
  - The golden rule on `/` is missing. The escape test fails, once I strengthened it.
  - `stop_vm` takes no permit. Both gate tests fail.
## Review of this leaf (one in-session reviewer, the leaf's allowance)

The claim had three parts. The two routes match the direct commands. No name escapes the VM records or the golden directory. The stop gate has no deadlock.

A fresh context got the contract and the files, with a prompt to find what is wrong. It found no path escape, no deadlock and no permit leak. It reported five defects:

1. **A stop that waited for the gate ran after its client left.** Valid and actionable. Take a user who stops a VM during a long start and presses Ctrl-C. They would lose the VM that the start then made. `stop_vm` holds a drop guard on a cancel token that the wait reads. When hyper drops the handler, the wait ends. A stop that holds the permit runs to its end. A test with an in-process server shows both cases. The stop ran for a client that stayed. It did not run for a client that left. The test failed before the fix.
2. **A flood of queued stops can exhaust the blocking pool.** A visible trade-off. Only a process of the same user can send them. That process can already stop every VM. Fix 1 ends the waits of clients that leave. No per-id cap exists.
3. **The leading-dot rule was stricter than the direct command.** Valid and actionable. Only `.` and `..` need refusing among dot names. A test deletes `.hand-placed`, and a mutation that restores the rule fails it.
4. **A name over about 22,000 characters gave `INTERNAL` where the direct command gives `GOLDEN_NOT_FOUND`.** Valid and actionable. A name over 255 bytes can name no golden. The CLI answers `GOLDEN_NOT_FOUND` without a request, and the app answers the same. A test covers 256, 3,000 and 30,000 bytes, and a mutation without the CLI check fails it.
5. **`pgrep`, `lsof` and `ps` resolve by the process `PATH`.** Noise. They are system tools in `/usr/bin`, which the default `PATH` of the app holds.

Each of fixes 1, 3 and 4 has an executable test that failed first. So no review leaf is cut.


## Not verified

- A live stop and delete of a real macOS guest through the development bundle. The fake `tart` stands for it. `herdr-acceptance-run-k29` runs the real one.
- The Linux and Windows builds. The new app code is `cfg(unix)`, and the vm and protocol changes use no new platform call, but this toolchain has no such target.
- The QEMU "golden in use" arm with a real `qemu-img` and a running clone. The tart arm shows the refusal and `--force`.
