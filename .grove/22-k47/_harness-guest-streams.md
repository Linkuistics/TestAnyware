# harness-guest-streams-k47 — brief

## Goal

The Linux host harness reaches every guest through the app. It dials no guest from the test process. This increment unblocks `non-macos-runtime-check-k40`: the harness then passes from an agent session that has no Local Network access of its own.

## Done when

- `linux_host_harness` takes its HUT from `vm start --platform linux --json` through the Host CLI. The tart clone of the stock image, `provision_ssh` and the SSH channel are gone from it.
- The harness provisions the HUT through its In-VM agent. The agent client takes its connections from the app, by the HUT's id.
- The harness forward opens a guest stream by the golden's id for each accepted connection. It binds the HUT's gateway address and accepts only the HUT's own address.
- `linux_dist_install_layout` uses the same HUT, channel and forward.
- The address choice, the peer check, the backend check and the two library checks are pure functions with unit tests.
- A person allowed the development bundle of the app. Then `linux_host_harness` passes from an agent session that has no Local Network access of its own. The binary is a workspace build for aarch64 Linux.
- The texts are correct: the module docs of `linux-host-harness.rs`, `vision/stages/text-ocr/README.md` on the HUT and its channel, and the description of the Taskfile task `release:linux-harness`. The description says that the task needs the Linux golden.
- No flag, exit code or `--json` field of the Host CLI changed.

## Decomposition

Each leaf leaves the harness green when a person runs it from Terminal, as it is today. Each demonstrates one new fact on its own.

1. `harness-golden-hut`: the HUT is the Linux golden, and the agent channel provisions it. This is the first step off the stock image and off SSH.
2. `harness-ocr-recognizer`: the OCR band finds `File` on the golden. The leaf pins the Python packages, because the newest ones crash or misread on this Mac's Linux guest. `harness-golden-hut` found the fault. It was cut ahead of the next leaves because they need a passing OCR band.
3. `harness-ocr-library-check`: the harness checks the native libraries of the OCR venv after it provisions them. It is small, and it stands apart from the forward.
4. `harness-golden-forward`: the forward opens guest streams. After this leaf, `linux_host_harness` passes from an agent session. This is the demonstration of the increment, and it is what `non-macos-runtime-check-k40` needs. `task release:linux-harness` runs only this test.
5. `dist-layout-golden-hut`: `linux_dist_install_layout` moves to the same HUT, channel and forward. The unblock does not wait for it.

The split follows the harness, not the code. The new `app_route` functions have no caller until leaf 1 uses them, so they stay in leaf 1.

## Pointers

- Spec: `docs/specs/testanyware-app.md`, sections "Guest traffic on a macOS host" and "How the host harnesses reach their guests".
- ADR-0004, ADR-0009 and ADR-0019, as `app-all-guest-traffic-k42` reworked them.
- The decision logs of `app-all-guest-traffic-k42` and `app-all-guest-traffic-k46`. They hold the rejected options, the evidence and the owner's two answers of 2026-10-21.
- The harness: `cli-rs/crates/testanyware-cli/tests/linux-host-harness.rs`. The Windows harness has an agent-based channel to copy from: `windows-host-harness.rs`.
- The shared layer: `testanyware_cli::app_route` (`cli-rs/crates/testanyware-cli/src/app_route.rs`). `agent_client` exists.

## Notes

- The HUT must be a tart guest. A same-named qcow2 makes `vm start` choose QEMU (`wants_tart` in `testanyware-vm/src/lifecycle.rs`). After the start, the harness reads the HUT's `backend` from `vm list --json`. When it is not `tart`, the harness stops the HUT, opens no forward and fails. Its message names the qcow2. Do not change how `vm start` chooses a backend.
- The forward binds the HUT's gateway address and accepts only the HUT's own address, the `src` of `ip route show default`. TN3179 says that accepting a connection needs no Local Network access. Only the outgoing dial does, and that dial now belongs to the app.
- The harness reads the golden's spec only for the VNC password.
- The shared layer needs a public function that opens a guest stream for a target and a kind. It also needs an agent client with a timeout that the caller sets, for long uploads and `exec` calls. Do not change a flag, an exit code or a `--json` field.
- The binary library check runs before staging. It reads the libraries that the binary and the bundled libraries need. It fails on any library that is neither bundled nor in the C runtime. The C runtime is `libc.so.6`, `libm.so.6`, `libpthread.so.0`, `libdl.so.2`, `libgcc_s.so.1` and `ld-linux-aarch64.so.1`. The candidate `v2.4.0` passed it by `objdump -p`.
- The OCR library check runs after the harness provisions the OCR venv in the HUT. It reads the libraries that each native library of the venv needs at load time. It fails on any library that resolves outside the venv and is neither in the C runtime nor `libstdc++.so.6`. The stock server image proved this boundary for the OCR band, and the desktop golden cannot prove it by a run.
- This increment changes code after the candidate commit, so it voids the candidate `v2.4.0`. `candidate-rebuild` in `app-release-k21` rebuilds it.
- The owner asked for focus: small increments and no rabbit holes.
