# exec-record-reader-k231

**Reviews:** exec-record-reader-k218

## Goal

Challenge the Host CLI record reader of exec-record-reader-k218 before the
macOS agent of leaf k219 and the cancellation leaves rely on it. Find each
record that the reader accepts but the spec calls invalid. Find each valid
record that it rejects. Find each output that changed for a consumer who uses
only the Host CLI.

## Context

- The producer's commit and its task file.
- Spec: `docs/specs/guest-command-lifecycle.md`, sections "Exec record",
  "Validity", "Host CLI semantics", "Compatibility and version skew" and
  candidate A.
- ADR-0023 (typed record).
- The Linux oracle: `agents/linux/tests/exec_validity.py`. Compare each rule
  with `validity_problems` in
  `cli-rs/crates/testanyware-protocol/src/exec_record.rs`.
- Code: `testanyware-protocol/src/exec_record.rs` and `agent_requests.rs`,
  `testanyware-agent-client/src/lib.rs` (`exec`), and
  `testanyware-cli/src/commands/file.rs` (`exec_envelope`).
- Tests: `testanyware-protocol/tests/exec_record.rs` and
  `testanyware-cli/tests/exec-record.rs`.
- Fixtures: `cli-rs/tests/fixtures/exec-replies/`. No Windows reply was
  recorded, so `windows.json` follows the agent source.

## Done when

- Each finding names its file, its line and a concrete failure.
- The review checks the macOS projection against
  `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift`, including the
  trailing-whitespace rule.
- The review judges the reading of "clock skew" in the producer's tests.
- The review judges the `sha2` 0.11 dependency, which declares Rust 1.85
  against the workspace `rust-version` 1.81.
- The review reruns `task cli:test` and reports the result.
- When a finding is worth acting on, insert an `integrate-review-impl` leaf as
  `references/decompose.md` directs.

## Notes

The producer kept every file's rustfmt drift at its HEAD level, because a
repo-wide format is forbidden. A rustfmt check on a whole file still reports
the drift from before the change.
