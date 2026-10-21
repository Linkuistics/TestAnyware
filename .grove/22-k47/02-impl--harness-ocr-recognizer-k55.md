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
