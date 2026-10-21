# harness-ocr-recognizer-k55

## Goal

The OCR band of `linux_host_harness` finds `File` on the Linux golden HUT. The harness installs a set of Python packages. With them, the EasyOCR daemon runs on this Mac's Linux guest and reads the macOS menu bar correctly.

## Context

- `harness-golden-hut-k49` moved the HUT to the Linux golden. Its run of 2026-10-21 passed the library check, the start, the backend check, the staging, the canary, the endpoint-free band and the OCR provisioning. Then `screen find-text` failed. This leaf owns that failure. No other band fails for a reason that is not the forward.
- The guest CPU passes SME and SME2 through in `/proc/cpuinfo`. `pip install easyocr` takes the newest packages: torch 2.14.1, numpy 2.5.4, opencv-python-headless 5.0.0.93 and scipy 1.18.1. With them, the daemon dies with `SIGILL` (`OCR_CHILD_CRASHED`) in `easyocr/recognition.py`, inside the text recogniser, on any frame that holds text. A blank 1920x1080 frame passes, because the recogniser does not run on it.
- Tried, with no effect on the crash: `ONEDNN_MAX_CPU_ISA=ADVANCED_SIMD`, `DNNL_MAX_CPU_ISA=ADVANCED_SIMD`, `ATEN_CPU_CAPABILITY=default` and `TORCH_DISABLE_SME=1`.
- Tried: `torch==2.6.0 torchvision==0.21.0`. The daemon runs, and a cold daemon answers in 8.2 s with the page cache dropped. But on the real frame it reads `Finder` at confidence 1.0 and then returns garbage (`LE=I`) at confidence 0.0, so it finds no `File`. Setting `torch.set_num_threads(1)` does not change that. A downgrade of opencv to 4.14 does not change it.
- Tried: `numpy<2`, `torch==2.2.2`, `torchvision==0.17.2`, `opencv-python-headless<4.11` and `scipy<1.14`. The daemon then failed with `std::bad_alloc`. Another package of the venv (`scikit-image` or `tifffile`) needs `numpy>=2.1`, so that mix was not consistent. The attempt was not finished.
- The frame is the macOS golden's desktop at 1920x1080. The host's Vision engine finds ` Finder File Edit View Go Window Help` in it, so the frame is sound. A copy is in no repository file. Capture it with `testanyware screen capture --vm <macos id>`.
- The stock server HUT passed this band when the harness last ran. It did not run again after the default changed to the app. So the cause may be the drift of the packages on PyPI and not the golden. That is not proved. A run on the stock image with today's packages would settle it.
- The release formula pins `easyocr` and takes the other packages as they are (`scripts/templates/testanyware.rb.tmpl`). The Linux hosts that it installs on run SME natively, so the crash is a property of the guest, not of the formula. Do not change the formula in this leaf.
- Disk: a clone of the Linux golden has 8.8 GB free. `harness-golden-hut-k49` added `--no-cache-dir` to the pip install so that it fits. `clone-disk-size-k54` decides whether clones get more.

## Done when

- A set of pinned versions, written once in the harness, makes `screen find-text File` pass on the Linux golden HUT through the in-VM CLI. The OCR band reports `engine=easyocr_daemon found 'File'`.
- A comment at the pin states the evidence: the versions that crashed, the versions that read garbage, and the version set that works.
- The pin does not change the product. No flag, exit code or `--json` field changes, and neither does the formula.
- A unit test or a check in the harness fails when the pin is missing from the install command.
- The run records how long a cold daemon takes. If it is within 15 s on the passing set, the 15 s first-call deadline of the bridge stays.
- The test still passes from Terminal. The decision log records the lines of the run. If the forward cannot dial the macOS golden from an agent session, the log names the lines that passed before it.

## Notes

- Time-box the search. Try the versions of the last known good run first. Find the date of the last green `linux_host_harness` in `git log`. Then install the packages that PyPI offered on that date, with the `--exclude-newer` option of `uv`.
- `linux_dist_install_layout` pins `easyocr==1.7.2` in the same way and has the same exposure. `dist-layout-golden-hut-k52` moves it to the golden, and it reuses the pin from this leaf.
- Do not touch the forward or the HUT start.

## Decisions (running log)

- **The pin is `torch==2.12.0 torchvision==0.27.0`, with `easyocr==1.7.2`.** I resolved the packages that PyPI offered on 2026-06-04, the date of the last green OCR band (`edd94b2`), with `uv pip compile --exclude-newer`. That set is torch 2.12.0, torchvision 0.27.0, numpy 2.4.6, opencv-python-headless 4.13.0.92 and scipy 1.17.1. On a clone of the Linux golden it found `File` with confidence 1.0.
- **Only torch is pinned.** In the same venv I moved numpy, opencv-python-headless and scipy to the newest (numpy 2.5.4, opencv 5.0.0.93, scipy 1.18.1) and kept torch 2.12.0. The result was the same: `File` at 1.0. So the crash and the garbage come from torch. The pin stays at two packages, and the other packages float as the formula lets them.
- **I did not search between 2.6.0 and 2.12.0, or between 2.12.0 and 2.14.1.** The brief asks for a time-boxed search with no rabbit holes. A passing version is enough. I did not re-run torch 2.14.1 or 2.6.0. Their evidence in the comment at the pin comes from `harness-golden-hut-k49`.
- **The pin lives in `OCR_TORCH_PINS`, and `ocr_pip_install_command` builds the command.** `provision_ocr` calls the function. The unit test `the_ocr_install_command_carries_both_pins` compares the whole command and checks each pin. I saw it fail: with `{OCR_TORCH_PINS}` removed from the format string, the test panicked. `provision_ocr` now installs `EASYOCR_PIN` too, the pin that `linux_dist_install_layout` already uses. `dist-layout-golden-hut-k52` can call the same function.
- **The product did not change.** The diff holds the harness and `vision/stages/text-ocr/README.md`. No flag, exit code or `--json` field changed, and the formula did not change.
- **The 15 s first-call deadline of the bridge stays.** `check_find_text` now times the first call. The cold `find-text` took 12.6 s through the bridge in the run below. That time includes the capture of the frame, so the daemon's own cold start is less. By hand, the model load took 10.1 s and the first read 5.4 s with the page cache warm from the install. The margin is 2.4 s. If a slower host fails on it, the deadline needs its own leaf.
- **The stock-image question stays open.** I did not run the stock server image with today's packages. The fix does not need the answer, because the pin passes on the golden.
- **The frame came from the running macOS guest `k26-live`.** A read-only `screen capture` gave 1920x1080 with the full menu bar and one notification banner.

### Demonstration

The run was on 2026-10-21, from an agent session, with the development bundle of the app running. The binary was the workspace build for aarch64 Linux from `harness-golden-hut-k49`. These lines passed:

```
[hut] started testanyware-1cd5987c
[hut] testanyware-1cd5987c runs on tart
[canary] --version execs on aarch64-linux: testanyware 0.0.1
linux_host_harness: all 6 endpoint-free checks passed.
[ocr] creating venv + pip install easyocr==1.7.2 torch==2.12.0 torchvision==0.27.0 (minutes) …
[ocr] daemon provisioned at /home/admin/taw/venv/bin/python
  ✓ screen-find-text: engine=easyocr_daemon found 'File' at 26x16 px on aarch64-linux; the cold daemon answered in 12.6 s
```

The four agent cases (`agent-health`, `agent-snapshot`, `agent-windows`, `agent-wait`) failed with `CONNECTION_DROPPED`. The forward dials the macOS golden from the test process, and this session has no Local Network access (`No route to host (os error 65)`). `harness-golden-forward-k51` owns that. The VNC cases passed.

### Not proved

- **A pass from Terminal.** This session cannot run the harness from Terminal. A person runs `task release:linux-harness` after `harness-golden-forward-k51`.
- **The SIGILL of torch 2.14.1 and the garbage of torch 2.6.0.** I took both from the log of `harness-golden-hut-k49`.
