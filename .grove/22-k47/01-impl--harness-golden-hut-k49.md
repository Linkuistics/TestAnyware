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
