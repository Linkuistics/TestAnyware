# acceptance-preparation-k243

**Reviews:** acceptance-preparation-k224

## Goal

Try to break the freeze of acceptance-preparation-k224 before k225 runs ten
trial clones under it. Find each input of the trial that the manifest does
not freeze. Find each claim of the evidence that the records do not support.
Find each check that would pass on a wrong record. A wrong pass and a false
failure both count.

## Context

- Root brief: sections "Done when", "Constraints" and "Acceptance inventory
  and bounds".
- The contract of k224 is its `Done when` list and its notes.
- The code, in `experiments/exec-reliability`:
  - `acceptance_k224.py`: the export, the manifest writer and the
    preparation clone with its check.
  - `image_audit_k224.py`: the image record and the image comparison.
  - `test_acceptance_k224.py`: the controls.
  - The reused parts: `acceptance_k223.py` and `acceptance_k223_check.py`.
- `Taskfile.yml`: the tasks `diagnostic:acceptance-images`,
  `diagnostic:acceptance-images-compare`, `diagnostic:acceptance-export`,
  `diagnostic:acceptance-manifest`, `diagnostic:acceptance-prepare` and
  `diagnostic:acceptance-prepare-check`.
- The report `docs/experiments/exec-reliability-acceptance-preparation.md`.
- The evidence in
  `experiments/exec-reliability/evidence/acceptance-preparation-k224/`. The
  manifest is `manifest.json` in that directory.

## Specific doubts

- Compare the manifest fields with `acceptance_k223.read_manifest` and with
  each digest check of the trial. Find a trial input that no pin covers.
- Check that each pin names the file that the trial runs. Check the Host CLI,
  the app executable, Tart, the interpreter and the trial modules.
- Check the agent identities. Compare the macOS identity with the installed
  agent and the Linux listing with the files that the golden runs.
- Check that the image comparison can detect a boot and a change of content.
  Check what the comparison reads, and what it leaves out.
- Compare each declared bound and ceiling with the root brief and the k223
  plan. Check that the boot bound limits the whole `vm start`.
- Check that each preparation clone is not a trial clone. Check that its
  judgement equals the k223 judgement of the same records.
- Check that each corruption control damages an independent copy, and that
  the originals stay unchanged.
- Check the failed sample `prepare-macos-01`. Check that the report and the
  running log state its cause from its records.
- Check the residual resources that the report names for k227.

## Done when

- Each finding names the code or the evidence, the input and the observed or
  derived wrong result.
- The review reports findings. It changes no code and no pinned file.

## Notes

Review the producer commit of acceptance-preparation-k224 against the current
source. A change to a pinned file needs a new freeze, so leave each change to
an integration step. Boot no guest. Do not patch the global CLI or app. Do not
stop the installed app.
