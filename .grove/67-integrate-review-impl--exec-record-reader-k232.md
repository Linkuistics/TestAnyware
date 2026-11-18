# exec-record-reader-k232

**Integrates:** exec-record-reader-k231

## Goal

Triage the committed review of `exec-record-reader-k218`, repair the actionable
reader defects, and verify the resulting contract before the macOS writer and
cancellation consumers proceed.

## Context

- Read the findings from the commit that retires `exec-record-reader-k231`.
  The review's body in that commit is the findings artifact.
- Producer: `58b52586537fa0493e54e7279929b31adc6d3fdc`.
- Contract: `docs/specs/guest-command-lifecycle.md`, the exec record, validity,
  legacy projections and Host CLI semantics; ADR-0023.
- Compare the reader with `agents/linux/tests/exec_validity.py` and the macOS
  projection with `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`.
- Scope: the protocol record reader and dependencies, its protocol tests and
  Host CLI fixture coverage. Keep legacy outputs and schema version 1.0.

## Done when

- Each committed finding is verified and classified against the agreed
  acceptance and material-risk threshold, with the disposition recorded.
- Actionable defects have bounded repairs and independent regression coverage
  at the protocol/Host CLI seam; the reader rejects invalid records and accepts
  valid agent projections according to the contract.
- The protocol dependency choice is reconciled with the declared Rust minimum,
  with verification scoped honestly around any pre-existing workspace limits.
- Fresh `task cli:test` results are recorded after repairs, together with
  appropriate lint and per-file format checks and any remaining limitations.
- The existing lifecycle acceptance leaves retain their guest/controller timing
  obligations; synthetic record tests do not stand in for those measurements.

## Notes

This flat sibling belongs immediately before `macos-agent-lifecycle-k219`:
the reviewed reader is its prerequisite, and intervening source edits would
move the evidence before triage. The review ran no test, build, lint or format
command. It also could not establish graph generation or coverage because the
graph CLI refused a pre-coordination or unverified generation. Use current
graph evidence if available and direct source fallback where required.

The producer task and commit describe tests but record no command result.
The repo has existing rustfmt drift; use the Taskfile's per-file interface
and distinguish inherited drift from the repair.
