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

## Decisions (running log)

- Reviewed producer commit `a2c877cb` against the current source. This is an
  inspection-only review: no test, build, lint or format commands, guest boots,
  or changes to pinned files. The graph CLI refused to start because an
  unverified generation is active, so the review uses direct source and record
  reads; graph freshness and coverage remain unverified.
- The image disk digest is recorded but not enforced by the trial. This is a
  freeze defect against the root requirement to freeze every image input,
  rather than an optional strengthening of the trial.
- The answering app reads a configuration file that can select a different
  Tart executable. The manifest pins the audit/export Tart path without binding
  it to the app's selection. This is a separate tool-provenance defect.
- The Python pin does not select or validate the interpreter that runs the
  trial. The trial takes `sys.executable` from its launch and adds it as an
  unfrozen subject. A second environment can leave the pinned interpreter
  unchanged while supplying another interpreter to the measurement.
- The preparation checker does not validate its own current source against
  the run's source digest, and its shape check omits the controller termination
  bounds and the complete bound table. Both omissions can change a judgement
  without the rejection that the k223 checker applies.
- The image comparison accepts equal missing file records as equal content.
  That violates the root rule that missing evidence cannot count as success.
- The six findings and their source-derived counterexamples are in
  `docs/experiments/exec-reliability-acceptance-preparation-review.md`.
  Read-only checks found every manifest pin unchanged, and the saved rechecks
  agree with the passing samples. Their original corruption-control inputs
  still match their recorded hashes. No new measured acceptance is claimed.
  Both graph status and coverage calls failed with the same generation error.
- Inserted acceptance-preparation-k244 with kind integrate-review-impl as the
  next sibling before lifecycle-acceptance-k225. Its body points to this
  review's commit and leaves the findings open to independent triage. The root
  still has live integration, trial, release and handoff work, so no parent
  closes with this review.
