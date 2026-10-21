# harness-golden-hut-k49

## Goal

The HUT of `linux_host_harness` is the Linux golden, started through the app. The harness provisions it through the In-VM agent, by the HUT's id, on guest streams. The harness stops cloning the stock image and stops using SSH.

## Context

- The brief of `harness-guest-streams-k47` holds the design in its Notes. The decision log of `app-all-guest-traffic-k42` holds the reasons. Read both.
- Copy the shape of the agent-based channel in `windows-host-harness.rs`. It implements `ProvisionChannel` on the agent client. The Linux harness has `SshChannel` today.
- `testanyware_cli::app_route::agent_client` builds an agent client on guest streams. The harness needs two additions to the public surface of `app_route`. The first is a function that opens a guest stream for a target and a kind. The second is an agent client whose timeout the caller sets. Long uploads and `exec` calls need the longer timeout. Change no flag, exit code or `--json` field.
- Start the HUT with `vm start --platform linux --json`, as `start_golden_vm` starts the macOS golden. The Linux golden is `testanyware-golden-linux-24.04`. Its clones mask `sshd`. Its agent runs `/exec` through `/bin/bash -c` as `admin`. That user has passwordless `sudo`.
- `wants_tart` in `testanyware-vm/src/lifecycle.rs` gives a same-named qcow2 priority over the tart golden. Do not change it.

## Done when

- `linux_host_harness` no longer calls `Hut::launch`, `provision_ssh`, `install_pubkey` or the SSH channel. `linux_dist_install_layout` still needs them. They stay for it until `dist-layout-golden-hut-k52`.
- The HUT comes from `vm start --platform linux --json`. The agent channel stages the binary and the bundled libraries, runs the `--version` canary and runs the endpoint-free band.
- After the start, the harness reads the HUT's `backend` from `vm list --json`. When it is not `tart`, the harness stops the HUT, opens no forward and fails. The message names the qcow2 as the cause.
- Before staging, the harness checks the libraries that the binary and the bundled libraries need. It fails on any library that is neither bundled nor in the C runtime. The C runtime is `libc.so.6`, `libm.so.6`, `libpthread.so.0`, `libdl.so.2`, `libgcc_s.so.1` and `ld-linux-aarch64.so.1`.
- The backend check and the binary library check are pure functions. Unit tests cover them, including a failing case for each.
- The test still passes when a person runs it from Terminal, with the app running. The forward is not changed in this leaf, so the macOS golden half still uses the old forward.
- Demonstration: run `linux_host_harness` from an agent session that has no Local Network access. The run stops at the forward. The log shows that these steps passed before that point: the HUT start, the staging, the canary, the endpoint-free band and the OCR provisioning. Record the log lines in the decision log.
- The texts are correct for what this leaf changes. The module docs of `linux-host-harness.rs` describe the HUT and its channel. `vision/stages/text-ocr/README.md` describes them too.
- The description of the Taskfile task `release:linux-harness` says that the task needs the Linux golden.

## Notes

- A person must allow the development bundle of the app first (`task app:bundle-dev`). If the app is not running or not allowed, stop and name the remedy.
- The OCR band runs `provision_ocr` through the channel. Keep its behaviour. The check of its libraries is `harness-ocr-library-check-k50`.
- Keep the forward and the gateway discovery as they are. `harness-golden-forward-k51` changes them.
- The owner asked for focus. Do not touch `linux_dist_install_layout` beyond what keeps it compiling.

## Decisions (running log)

- **The HUT is the Linux golden, started by `vm start --platform linux --json`.** `linux_host_harness` no longer calls `Hut::launch`, `provision_ssh`, `install_pubkey` or `SshChannel`. Those stay in the file for `linux_dist_install_layout` until `dist-layout-golden-hut-k52`. The `GoldenGuard` became `VmGuard` with a label, so one guard stops the HUT and the macOS golden.
- **The channel is `AgentChannel`, built by `agent_client_with_timeout`.** It copies the shape of the Windows harness. The client takes each connection from the app, by the HUT's id. `vm start` waits for the agent's health, so the harness adds no readiness poll.
- **`app_route` gained two public functions.** `open_guest_stream(socket, target, kind)` opens one stream. `agent_client_with_timeout(socket, target, timeout)` sets the timeout of each request. `agent_client` calls the same code with the 30 s default, so no command changes. `open_guest_stream` has no caller in the harness yet. `harness-golden-forward-k51` uses it. `tests/app-harness-library.rs` runs both against the real app in its test mode.
- **The library check runs before the HUT starts.** A bad bundle then costs no boot. `check_library_needs` is a pure function over the parsed `objdump -p` output. It also fails when a file lists no needs, because that means `objdump` did not read an ELF file and every file would pass. I saw that control fail: a Mach-O binary gave `objdump listed no needed libraries`.
- **The backend check reads `vm list --all --filter kind=running,name=<id> --json`.** `check_hut_backend` is pure. Its message names `testanyware-golden-linux-24.04.qcow2` and the golden directory. The guard stops the HUT when the check panics, and no forward is open by then.
- **`expect_ok_envelope` keeps the error object.** With `--json` the CLI prints its error on stdout and leaves stderr empty, so the old message of a failed case said nothing.
- **The OCR pip install runs with `--no-cache-dir`.** A clone of the golden has 17 GB for `/` and 8.8 GB free. The install of the newest torch fills it ("No space left on device") when pip also keeps its 3 GB cache. With the flag it ends with 3.3 GB free. Your request for more clone disk space became `clone-disk-size-k54`, a `design` leaf at the end of the tree. The owner chose a separate delivery on 2026-10-21.
- **The OCR band fails on the golden, and `harness-ocr-recognizer-k55` owns it.** I cut it ahead of `harness-ocr-library-check-k50`, because k50 and the forward leaf need a passing OCR band. The cause is not the channel or the HUT start. With the newest packages the daemon dies with `SIGILL` in the recogniser, because the guest CPU passes SME through. With torch 2.6.0 it runs and reads garbage. The leaf records the evidence.
- **I raised the OCR first-call deadline to 60 s, then reverted it.** The owner chose that on 2026-10-21, because I read `OCR_TIMEOUT` as a slow cold start (9 to 15 s measured with the newest torch). Later I measured a cold daemon at 8.2 s with torch 2.6.0 and the page cache dropped. So the deadline was not the cause. I reverted `bridge.rs` and the README, and the owner's choice does not stand on the new evidence. `harness-ocr-recognizer-k55` keeps the 15 s unless its run shows otherwise.
- **The binary of the live runs is a workspace build.** `cargo zigbuild` for `aarch64-unknown-linux-gnu`, with the recipe of `scripts/release-build.sh`. The shell found Homebrew's Rust first, which has no Linux standard library. `PATH=$HOME/.cargo/bin:$PATH` selects the rustup toolchain.

### Demonstration

The run was on 2026-10-21, from an agent session. The development bundle of the app was running. The binary was a workspace build for aarch64 Linux, with the ffmpeg libraries of `~/.cache/taw-ffmpeg-sr/aarch64-linux`. These lines passed before the forward:

```
[libs] the binary and the bundled libraries need only bundled libraries and the C runtime
[hut] started testanyware-a8e2b05e
[hut] testanyware-a8e2b05e runs on tart
[provision] staged binary + 5 ffmpeg libs into /home/admin/taw
[canary] --version execs on aarch64-linux: testanyware 0.0.1
  ✓ help  ✓ capabilities  ✓ schema  ✓ llm-instructions  ✓ doctor  ✓ dry-run
linux_host_harness: all 6 endpoint-free checks passed.
[ocr] daemon provisioned at /home/admin/taw/venv/bin/python
[ocr] ready: /home/admin/taw/venv/bin/python
```

At the forward, the agent connections failed as the leaf predicts, because the forward still dials the golden from the test process:

```
[forward] agent conn from 192.168.64.3:…: connect 192.168.64.7:8648: No route to host (os error 65)
  ✗ agent-health / agent-snapshot / agent-windows / agent-wait: CONNECTION_DROPPED
  ✓ screen-size ✓ screen-capture ✓ screen-record ✓ input-key ✓ input-type ✓ input-click
```

The VNC cases pass, because the keeper listens on loopback. `screen-find-text` fails: `EasyOCR did not find 'File'; detections: []`. That is `harness-ocr-recognizer-k55`.

### Not proved

- **A pass from Terminal.** I cannot run the harness from Terminal in this session. The agent cases fail here by design. The OCR band fails on the packages, as the leaf above records. So "the test still passes from Terminal" is not shown. A person runs `task release:linux-harness` after `harness-ocr-recognizer-k55`.
- **The cold-start time.** The 8.2 s was measured on a hand-provisioned clone, with the daemon fed a request on stdin. It was not measured through the bridge.
