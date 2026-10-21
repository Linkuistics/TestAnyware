# harness-ocr-library-check-k50

## Goal

After the harness provisions the OCR venv in the HUT, it checks the native libraries of that venv. The HUT is now a desktop golden. A library that only a desktop has could hide a missing dependency of the OCR band. This check keeps the boundary that the stock server image proved.

## Context

- The brief of `harness-guest-streams-k47`, Notes. The decision log of `app-all-guest-traffic-k46` records finding F4 and its fix.
- The release ships the `ocr_analyzer` module. The Linux formula installs a pinned EasyOCR into a venv (`scripts/templates/testanyware.rb.tmpl`). The harness mirrors that in `provision_ocr`.
- `harness-golden-hut-k49` moved the HUT to the Linux golden and the channel to the In-VM agent. Use that channel to read the libraries.

## Done when

- After `provision_ocr`, the harness lists each native library of the venv. For each one it reads the libraries that the loader needs at load time.
- The check fails on any needed library that resolves outside the venv and is neither in the C runtime nor `libstdc++.so.6`. The C runtime is `libc.so.6`, `libm.so.6`, `libpthread.so.0`, `libdl.so.2`, `libgcc_s.so.1` and `ld-linux-aarch64.so.1`. The failure names the native library and the library it needs.
- The decision is a pure function over the parsed output of the in-guest command. Unit tests cover a passing case and a failing case. The failing case uses a library that resolves outside the venv.
- The check reports through the same band result as the OCR provisioning. A failure reads as a failed OCR check and does not mask the driven band.
- Demonstration: a run on the desktop golden prints the check as passed. The decision log records the number of native libraries that it read. If a real run is not possible in the session, the log says so and names what is missing.
- The test still passes when a person runs it from Terminal.

## Notes

- Design the in-guest command so that it is easy to parse. The log of `app-all-guest-traffic-k42` does not fix the command. `ldd` and `readelf -d` are both candidates. Prefer the one that reports what the loader resolves.
- `linux_dist_install_layout` provisions its own venv. `dist-layout-golden-hut-k52` applies this check there, so keep the function reusable by both tests.
- Do not change the forward or the HUT start.
