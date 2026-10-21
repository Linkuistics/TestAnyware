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
