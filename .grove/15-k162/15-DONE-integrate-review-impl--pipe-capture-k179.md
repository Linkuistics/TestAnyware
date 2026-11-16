# pipe-capture-k179

**Integrates:** pipe-capture-k178

## Goal

Triage the implementation review from its committed findings and complete any
accepted repair before the publication slice builds on pipe capture.

## Context

Read the commit for `pipe-capture-k178` and the reviewed producer commit for
`pipe-capture-k167`. The contract is
`docs/specs/diagnostic-evidence-storage.md`; the parent brief owns the remaining
publication work in `storage-publication-k168`.

## Done when

Each review finding has a source-backed disposition against the agreed
acceptance and material-risk threshold. Accepted repairs have appropriate
regression verification through the repository's reusable tasks. Record any
accepted trade-off or rejected finding with its reason, and leave pipe capture
ready for the publication slice.

## Notes

Consume the finding details from the review's commit. This task's placement
keeps integration adjacent to that review, ahead of publication. Fixes and
post-fix verification belong here; the preceding review was inspection-only.

## Decisions (running log)

- F1 (P2, lexical root bypass) is a real issue. The k167 contract requires the
  validator to refuse a teardown executable under the evidence root. A direct
  reproducer showed that an evidence root spelt `//tmp/evidence` admitted
  `/tmp/evidence/tool` and `//tmp/evidence/tool`. POSIX lets `abspath` keep
  two leading slashes, and `commonpath` returns one. The equality test in
  `_refuse_evidence_executables` therefore compared two spellings of one path.
- Repair: the validator gives the root the `commonpath` spelling before the
  comparison. The comparison stays lexical and reads nothing under the root.
  A root of three or more slashes was already refused, because `abspath`
  collapses them.
- Regression: `test_review_double_slash_root_still_refuses_teardown_under_it`
  in `experiments/exec-reliability/test_controller.py` runs `execute()` with a
  double-slash root, for both executable spellings. It expects a refusal before
  acquisition. Both cases fail against the old controller and pass with the
  repair.
- Verification: `mise exec -- task diagnostic:test` passed 1073 tests, with 5
  expected failures. `ruff check`, `ruff format --check` and `pyright` report
  no findings. Pipe capture is ready for `storage-publication-k168`.
