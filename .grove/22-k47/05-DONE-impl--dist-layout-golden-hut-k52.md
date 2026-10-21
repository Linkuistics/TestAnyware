# dist-layout-golden-hut-k52

## Goal

`linux_dist_install_layout` uses the same HUT, channel and forward as `linux_host_harness`. After this leaf, the Linux host harness file dials no guest from the test process, and it holds no SSH code.

## Context

- The brief of `harness-guest-streams-k47`. The decision log of `app-all-guest-traffic-k42` says that this test "uses the same HUT, and it changes in the same way".
- The test reproduces the Homebrew keg layout in the HUT. It stages with `stage_dist_binary`, provisions a venv with `provision_dist_venv`, and starts its own forward for the VNC endpoint.
- `harness-golden-hut-k49`, `harness-ocr-library-check-k50` and `harness-golden-forward-k51` built the pieces. Reuse them.
- `task release:linux-harness` runs only `linux_host_harness`. This test is outside the unblock of `non-macos-runtime-check-k40`.

## Done when

- `linux_dist_install_layout` takes its HUT from `vm start --platform linux --json`, provisions it through the agent channel, and checks the backend as `linux_host_harness` does.
- Its forward opens guest streams by the golden's id, binds the HUT's gateway address and accepts only the HUT's own address.
- The binary library check runs before staging. The OCR venv library check runs on the venv that `provision_dist_venv` makes.
- `Hut::launch`, `provision_ssh`, `install_pubkey`, `SshChannel` and the tart clone of the stock image are removed from the file, with the imports and constants that only they used.
- The module docs describe the HUT and the channel as they now are. No text says that the harness uses SSH or a stock image.
- Demonstration: `linux_dist_install_layout` passes from an agent session that has no Local Network access of its own. The decision log records the command and the result line. If the session cannot run it, the log says what is missing.

## Notes

- Keep what the test proves: the RUNPATH self-location and the install-layout OCR. Only the HUT, the channel and the forward change.
- `linux_host_harness` must still pass. Run it again after the removal.

## Decisions (running log)

- **`linux_dist_install_layout` starts the HUT as `linux_host_harness` does.** The test runs `check_staged_libraries` before the HUT starts, then `start_vm(HUT_PLATFORM, "hut")` under a `VmGuard`, then `hut_backend_is_tart`, then `AgentChannel::to_vm`. It uses `forward_plan_here` and `PortForward` for its VNC forward, which k51 already moved. The test body keeps its two deltas and their text: RUNPATH self-location and install-layout OCR.
- **`provision_dist_venv` installs with `ocr_pip_install_command`.** The old line was `pip install --quiet easyocr==1.7.2`. That line had the two faults that `harness-ocr-recognizer-k55` and `harness-golden-hut-k49` found on the Linux golden: the newest torch dies with `SIGILL`, and pip's cache fills the 8.8 GB of free disk. So the dist venv takes the same pinned torch pair and `--no-cache-dir` as the OCR venv. The formula's pin of easyocr stays `EASYOCR_PIN`. The torch pin is a property of this guest and not of the formula, so the test does not claim that the formula pins torch.
- **`provision_dist_venv` runs the OCR library check after the pip install.** It calls `check_venv_libraries(ch, &venv)` on `<prefix>/libexec/venv`, the venv it just made, and an error returns like a pip failure. The caller reports it as the `install-layout-ocr` result, as `linux_host_harness` does for `screen-find-text`. The function already took the venv as a parameter (k50), so no new pure function was needed and no unit test is new.
- **Removed from the file:** `Hut` with its `launch` and `Drop`, `HUT_BASE`, `provision_ssh`, `install_pubkey`, `SshChannel`, and the imports `tart` and `SshSession`. The `ProvisionChannel` trait has one implementation now, `AgentChannel`, and stays because the helpers are generic over it. A grep for the removed names finds none, and the control `AgentChannel|open_guest_stream` finds 11 lines in the same file. The harness has no `TcpStream::connect` and no `connect(` call.
- **The texts are current.** The module docs say that both tests have the same HUT, channel and forward. The reuse-seam paragraph, the `ProvisionChannel` doc, the section heading above `stage_binary`, the comment on the `cfg(target_os = "macos")` line and the comment above the dist test no longer mention the clone, SSH or "until it moves". The one line with "SSH" says that the harness uses none.
- **The `cfg(target_os = "macos")` stays.** Its reason changed: the TestAnyware app and the macOS golden need a macOS host, and the stock-image clone is gone.

### Demonstration

The runs were on 2026-10-21 from an agent session. The development bundle of the app was running (`testanyware doctor`: running, `local network: a dial to 192.168.64.3:8648 connected`). The binary was the workspace build for aarch64 Linux that `harness-golden-forward-k51` used (`cli-rs/target/aarch64-unknown-linux-gnu/release/testanyware`, sha256 `4b9d2966…8321c0`, unchanged before and after). The ffmpeg libraries came from `~/.cache/taw-ffmpeg-sr/aarch64-linux`. The sha256 of `linux-host-harness.rs` for the two final runs was `a7d69e61…8d5e2b`, and `shasum -c` reported OK after them. One run wrote one log.

Command, with `<t>` as `linux_dist_install_layout`, then as `linux_host_harness`:

```
TESTANYWARE_LINUX_HARNESS=1 TESTANYWARE_LINUX_BIN=$PWD/target/aarch64-unknown-linux-gnu/release/testanyware \
  TESTANYWARE_FFMPEG_LIB_DIR=$HOME/.cache/taw-ffmpeg-sr/aarch64-linux/lib \
  cargo test -p testanyware-cli --test linux-host-harness -- --ignored --nocapture <t>
```

Result line of `linux_dist_install_layout`: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 255.39s`

```
[libs] the binary and the bundled libraries need only bundled libraries and the C runtime
[hut] testanyware-beabca41 runs on tart
[dist] ✓ delta 1 — RUNPATH self-location: --version execs with no LD_LIBRARY_PATH: testanyware 0.0.1
[dist] library check passed: read 350 native libraries of the venv; …
[forward] vnc: 192.168.64.1:52902 accepts 192.168.64.20 only -> Vm { id: "testanyware-a6315252" } vnc stream of the app
  ✓ install-layout-ocr: engine=easyocr_daemon found 'File' at 26x16 px on aarch64-linux
```

Result line of `linux_host_harness`, run again after the removal: `test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 41 filtered out; finished in 266.13s`. All 6 endpoint-free checks and all 11 endpoint-driven checks passed, `screen-find-text` included (cold daemon 8.5 s).

### Not proved

- **Two failed runs before the passing runs.** The first two runs of `linux_dist_install_layout` on the first version of the file failed at `wait_for_golden_ready`: `golden never became ready: snapshot had no Finder menu bar File item`, after 120 s, on two different macOS golden clones. The HUT part had passed in both. I did not find the cause. A macOS golden that I started by hand showed `File` in its menu bar within seconds. A third run with a temporary diagnostic in that error message passed, and so did the two final runs on the restored file. The wait is code that I did not change, and `linux_host_harness` runs it too. One VM of another session (`k26-live`, macOS) ran during all runs. I cannot say whether it, the boot state of the clone or chance caused the failures. Three passes follow two failures, so a person who sees this failure again should read it as a slow or stuck macOS boot and not as a fault of the HUT, the channel or the forward. The temporary diagnostic is not in the final file (`grep DIAG` finds nothing, and the digest is the one of the file before it).
- **A foreign peer in a live run.** As in k51: the peer check has unit tests and no other peer connected.
- **A pass from Terminal.** This session cannot run the test from Terminal, and the forward no longer needs it.
- **That this session lacks Local Network access.** As in k51, I rely on the earlier runs of the same kind of session and on the absence of a direct dial in the harness.
- **`task release:linux-harness`.** It runs only `linux_host_harness`, so `linux_dist_install_layout` is outside it, as the leaf says. I changed no Taskfile text.
