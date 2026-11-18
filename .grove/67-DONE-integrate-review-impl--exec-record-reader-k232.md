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

## Decisions (running log)

Findings come from review commit f8d67958e9. Source lines refer to producer
commit 58b5258653. Before any change, this session checked each finding
against the source, the Linux oracle and the spec.

- R1, null identity hides the termination evidence: a real issue, accepted.
  The reader took "started" from `leaderPid`, so a null PID turned a timeout
  into a command that never started.
  - Change: `Outcome::started` takes the started state from the outcome kind,
    as `exec_validity.py` does. `Lifecycle::leader_started` is gone.
  - Change: `check_identity` requires `leaderPid`, `groupId` and `durationMs`
    for a started command. It rejects each of them, and `leaderStartTime`, for
    a command that never started.
  - Change: `leaderPid` and `groupId` must not be negative.
- R2, unchecked capture reason: a real issue, accepted. An incomplete stream
  now needs `deadline` or `read_error`. A complete stream must have no reason,
  as the oracle requires.
- R3, macOS whitespace projection: a real issue, accepted. A Swift 6.4.2
  oracle ran the Foundation call from `ProcessRunner.swift`. It removes VT,
  U+0085, U+00A0, U+2028 and U+202F. It keeps U+001C to U+001F, U+180E,
  U+200B and U+FEFF. That set is the Unicode White_Space property.
  - Change: `macos_decode` trims with `char::is_whitespace`, which tests the
    same property. The unit test now asserts the oracle output.
  - Change: the new protocol test states each legacy value from the oracle.
    It does not derive the value from `legacy_projection`.
- R4, `sha2` 0.11 needs Rust 1.85: a real issue, accepted. The protocol crate
  inherits `rust-version` 1.81.
  - Change: the workspace takes `sha2 = "0.10"`. The lock adds `sha2` 0.10.9,
    `digest` 0.10.7 and `block-buffer` 0.10.4, and nothing else changes.
    Other crates keep `sha2` 0.11.0.
  - Verification: `cargo +1.81 test --locked -p testanyware-protocol` passed,
    with 61, 23 and 14 tests. Each dependency of the crate declares edition
    2021 or earlier.
  - Limit: this proves the minimum for the protocol crate only. Other crates,
    such as `testanyware-vm`, already pulled `sha2` 0.11 before this leaf.
- Clock-skew comment: noise for acceptance, accepted as polish. The comment in
  `agent_clock_values_are_read_without_host_time` now states the checked
  contract. It says that these cases do not measure controller timing bounds.

### Regression coverage

Each new test failed against the reader at HEAD and passes after the repair.

- Protocol: `a_started_outcome_requires_its_identity`,
  `a_command_that_never_started_has_no_identity_and_no_termination`,
  `capture_reason_contradictions_are_invalid` and
  `macos_projection_trims_as_foundation_trims`.
- Host CLI: `a_record_that_hides_its_evidence_is_an_error` sends an R1 record
  and an R2 record. Each gives `EXEC_RECORD_INVALID`.
- The old test `a_command_that_never_started_has_no_termination` nulled the
  identity of a timeout. R1 makes that record invalid, so the new spawn-failure
  and cancellation tests replace it.

### Verification

- `task cli:test`: every suite passed, and no test failed. Live-VM tests stay
  ignored.
- `task cli:clippy`: no warning in a changed file. The warnings in
  `testanyware-rfb` and `testanyware-vm` existed before this leaf.
- `task cli:fmt` on `crates/testanyware-protocol/src/exec_record.rs` and
  `crates/testanyware-protocol/tests/exec_record.rs`: clean.
- `task cli:fmt` on `crates/testanyware-cli/tests/exec-record.rs`: the only
  differences are in `tests/common/mod.rs`, which rustfmt follows through
  `mod common`. That drift existed before this leaf.

### Not changed

- This leaf keeps the legacy outputs and schema version 1.0.
- The macOS typed records are still synthetic until leaf k219 delivers the
  writer.
- The lifecycle acceptance leaves k223 to k225 keep their guest and controller
  timing obligations. These synthetic tests do not replace them.
