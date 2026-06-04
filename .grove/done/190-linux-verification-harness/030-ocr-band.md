# 030-ocr-band

**Kind:** work

## Goal

Close the three-band suite: run `screen find-text` (OCR) green from inside the
Ubuntu ARM64 HUT, driving the forwarded golden's framebuffer. On non-macOS,
`testanyware` routes OCR through the **EasyOCR daemon** (`OcrChildBridge`,
ADR-0002), so this leaf is really about **provisioning a working EasyOCR daemon
in the HUT** — the one heavy, separable provisioning step, isolated here so its
risk can't block `020`'s green.

## The hard part — provision `python -m ocr_analyzer --daemon`

`OcrEngine::detect()` on Linux builds `Daemon(OcrChildBridge)` with
`resolve_interpreter()` → `[$TESTANYWARE_OCR_PYTHON | <prefix>/libexec/venv/bin/
python | <ancestor>/pipeline/.venv/bin/python | /usr/bin/python3]`, and launches
`python -m ocr_analyzer --daemon` (`OcrChildBridgeConfig::new`, args
`["-m","ocr_analyzer","--daemon"]`). So the venv must:

1. **Contain the `ocr_analyzer` module** — which is **NOT in this repo's working
   tree** (grep finds only `tests/fake-ocr-daemon.sh`; the real module lived in
   the deleted Swift `cli/` tree or an unshipped `pipeline/`). **First sub-task:
   locate it** — check `git show 23e0c9d^:` paths around the Swift delete, the
   `provisioner/` history, and `release-build.sh`'s "testanyware_agent (Linux
   in-VM agent, Python source)" reference. It may need to be sourced/ported and
   given a home in-repo (a `pipeline/` or similar) so the harness can upload it.
2. **Have `easyocr` installed** — which pulls **torch** (CPU, aarch64-linux
   wheels exist on PyPI but the download is large and slow). Build the venv
   in-guest: `python3 -m venv venv && venv/bin/pip install easyocr` (+ whatever
   `ocr_analyzer` imports). Budget real time for the torch download; consider
   caching the built venv as a tarball between runs (host-side cache, not baked
   into the image — [[minimal-images]]).

Point the binary at it with `TESTANYWARE_OCR_PYTHON=<venv>/bin/python` (simplest
for the harness; avoids the install-layout `libexec/venv` convention) and set
`PYTHONPATH` so `ocr_analyzer` imports.

## The band

`screen find-text "File" --vnc <gw>:VFWD --json` (+ `TESTANYWARE_VNC_PASSWORD`)
against the forwarded macOS golden's Finder menu bar (the same deterministic
`File` fixture `live-vm-gate` uses). Assert `engine == "easyocr_daemon"` and a
hit on `File` with a plausible bounding box.

## Done when

- The full three-band harness runs **green** in one `TESTANYWARE_LINUX_HARNESS=1`
  invocation, including `screen find-text` resolving via the EasyOCR daemon on
  aarch64-linux.
- The `ocr_analyzer` module's location/home is resolved and documented (and, if
  it had to be ported into the repo, committed).
- The OCR provisioning recipe (venv build, torch, module path, env) is captured
  in the harness or a short doc so it's reproducible and reusable by the Windows
  harness.

## On node retire (this is likely the last leaf)

After this is green, perform the node BRIEF's "On retire" promotions: record the
Linux aarch64 runtime green into the **root brief's Tier-2 checklist**, confirm
the x86_64 build-only gap is logged, and promote the Windows-harness reuse notes
into the root's deferred Windows-harness line. Then the node retires into
`done/`.

## Notes

- If torch-on-ARM64 proves a multi-session rabbit hole, this leaf can itself
  decompose (e.g. a `pipeline`/`ocr_analyzer` sourcing leaf vs the venv+green
  leaf) — but try the straightforward `pip install easyocr` path first.
- The macOS `live-vm-gate` exercises the daemon path best-effort
  (`TESTANYWARE_OCR_FALLBACK=1`) and skips when the venv is absent — a reference
  for how the daemon is expected to behave once provisioned.
