# clone-disk-size-k54

## Goal

Decide how a clone of a golden gets more disk space than its golden has. The owner asked for it on 2026-10-21, while `harness-golden-hut-k49` ran the OCR band.

## Context

- The Linux golden `testanyware-golden-linux-24.04` has a 17 GB root file system. A clone of it starts with 8.8 GB free.
- `pip install easyocr` in a clone filled that space. The wheels of torch and the CUDA libraries took 3 GB in pip's cache and about 6 GB in the venv. The install failed with `No space left on device`. `harness-golden-hut-k49` works around it with `--no-cache-dir`. The install then ends with 3.3 GB free.
- The owner chose a separate delivery on 2026-10-21. Two other options were on the table. One grows every tart clone in `vm start`. The other rebuilds the golden larger.
- The golden is a tart guest. Cirrus images grow the root partition on boot, so a larger disk may need no guest step. Check that on the Linux golden.
- `vm start` is a command with a fixed meaning (the brief). A change to it must keep every flag, exit code and `--json` field.

## Done when

- A design records where the size is set. The candidates are an option of `vm start`, the `config.rhai` file, a fixed rule for the Linux platform, and a rebuild of the golden. It records the rejected options and the reason.
- The design says whether a macOS clone changes, and what a QEMU clone does.
- The design says what the guest does with the extra space, and how a test proves it.
- The spec or an ADR carries the decision, if the decision changes a behaviour that they state.

## Notes

- The tree places this leaf after `app-release-k21`, because `leaf-add` appends at the end. The owner may want it before the release. That is a decision for the next planning session. The OCR band passes without it.
- This leaf is outside the issue #8 delivery. It exists so the request is not lost.
