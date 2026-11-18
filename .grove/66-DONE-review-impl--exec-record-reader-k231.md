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

## Decisions (running log)

The review remains inspection-only under `grove/references/review.md`. The
task's request to rerun `task cli:test` conflicts with that family procedure;
this session will inspect the producer's recorded verification and hand fresh
test execution to the integration session. No test, build, lint or format
command is run here.

The graph CLI refused `index_status` because a pre-coordination or unverified
generation is active. Direct source reading is the fallback; this review cannot
claim graph generation or index-coverage verification.

The four findings below block the reader's agreed validity, compatibility or
declared toolchain contract. They warrant one flat `integrate-review-impl`
sibling before `macos-agent-lifecycle-k219`. That session owns bounded repairs
and fresh verification; the existing lifecycle acceptance leaves stay live.

## Findings

Reviewed producer change `58b52586537fa0493e54e7279929b31adc6d3fdc`
(`exec-record-reader-k218`). These failures are derived from source inspection;
the examples below were not executed in this review.

### R1 — Required termination evidence can be bypassed by a null PID (P1)

Location: `cli-rs/crates/testanyware-protocol/src/exec_record.rs:335`, using
`Lifecycle::leader_started` at line 122.

Start with the body in
`agents/linux/tests/fixtures/exec-records/deadline-term-ignored.json`, send its
existing id, set `lifecycle.leaderPid` to null and remove `termination`.
The reader accepts this record: `leader_started()` is now false, so the
termination check takes `(None, false)`, and no other check relates the
outcome to the lifecycle identity. The timeout outcome and legacy projection
are unchanged.

A `timed_out` command necessarily started. The lifecycle contract in
`docs/specs/guest-command-lifecycle.md:405` and termination requirement at
line 430 require its identity, duration and termination evidence. The Linux
oracle derives started status from the outcome at
`agents/linux/tests/exec_validity.py:168`, checks the identity and duration at
line 170, and requires termination at line 179. The Host CLI instead emits a
successful typed-record envelope for a timeout with no group-cleanup evidence.

The same missing consistency check admits null `groupId` or `durationMs` for
started outcomes, negative PID/group values, and non-null identity on a
`spawn_failed` or `cancelled`/`beforeStart: true` record. These belong to the
same repair: validate lifecycle presence and values against the outcome before
using them to decide which evidence is mandatory.

### R2 — Incomplete capture reasons are never validated (P2)

Location: `cli-rs/crates/testanyware-protocol/src/exec_record.rs:378`;
`StreamEvidence.reason` is an unrestricted optional string at line 87.

Using the same timeout fixture, change `evidence.stdout.capture` to
`incomplete`, increase `fileBytes` from 14 to 15, and omit `reason` (or set it
to `"invented"`). Leave the captured bytes, digest and legacy fields intact.
The count inequality passes and `check_stream` never examines `reason`, so
the reader accepts it. The CLI publishes the missing or unknown reason as
part of a valid agent record.

`docs/specs/guest-command-lifecycle.md:413` requires an incomplete stream to
hold `reason: deadline` or `read_error`.
`agents/linux/tests/exec_validity.py:78` rejects both mutations. That oracle
also rejects a complete stream carrying a reason at line 73, which the Rust
reader currently accepts. Validate the capture/reason combination and add
independent invalid-record cases for it.

### R3 — macOS projection retains whitespace that Foundation removes (P2)

Location: `cli-rs/crates/testanyware-protocol/src/exec_record.rs:563`;
the assertions at lines 628–629 encode the same incorrect expectation.

The macOS agent removes trailing `\s+$` using Foundation's regular-expression
engine in `agents/macos/Sources/TestAnywareAgent/ProcessRunner.swift:129`.
The Rust projection explicitly retains vertical tab U+000B and next-line
U+0085. ICU's implementation defines `\s` using `WhiteSpace`, and both
characters belong to that property. See the
[ICU 76.1 implementation](https://github.com/unicode-org/icu/blob/release-76-1/icu4c/source/i18n/regexst.cpp#L51-L53),
the [Unicode 16 property data](https://www.unicode.org/Public/16.0.0/ucd/PropList.txt),
and Foundation's
[ICU syntax declaration](https://github.com/swiftlang/swift-corelibs-foundation/blob/main/Sources/Foundation/NSRegularExpression.swift#L72).

For an otherwise valid macOS exited record with stdout evidence bytes
`61 0b` (`base64: YQs=`, byte/file counts 2, matching digest), the agent's
legacy stdout is `"a"`. The reader projects `"a\u000b"` and returns
`EXEC_RECORD_INVALID`. The same problem occurs with a trailing U+0085 on
either stream. Conversely, a record with the untrimmed legacy text is
incorrectly accepted. This blocks compatibility with the forthcoming macOS
writer for ordinary command output containing these bytes.

The synthetic macOS protocol-test helper derives its expected legacy text
using the production `legacy_projection` function, so those cases cannot
independently detect this discrepancy. Preserve agent-compatible expected
values in regression coverage rather than deriving both sides from the
reader being tested.

### R4 — New protocol dependency exceeds the declared Rust minimum (P2)

Location: `cli-rs/Cargo.toml:44`, consumed by
`cli-rs/crates/testanyware-protocol/Cargo.toml:14`.

The workspace declares `rust-version = "1.81"` at
`cli-rs/Cargo.toml:20`, inherited by the protocol crate. The new direct
dependency `sha2 = "0.11"` resolves to 0.11.0, whose package metadata declares
edition 2024 and Rust 1.85. This was verified by reading the downloaded
`sha2-0.11.0/Cargo.toml` package metadata, lines 13–16. A Rust/Cargo 1.81
consumer of the protocol crate cannot build the new dependency.

The lockfile already contained sha2 0.11 through other workspace crates;
this is not evidence that the whole workspace previously built on 1.81.
The regression here is the newly introduced direct dependency in the
previously serde-only protocol crate. Reconcile this addition with the
declared toolchain contract and verify the chosen resolution. Reusing an
existing lockfile version does not establish compatibility with that minimum.

## Other review judgments and verification limits

`agent_clock_values_are_read_without_host_time` at
`cli-rs/crates/testanyware-protocol/tests/exec_record.rs:544` checks very large
agent-relative values and zero values. That is useful coverage of the reader's
actual contract: non-negative values, ordering within the record and derived
bounds, without comparison to host time. A fixed wall-clock offset does not
make these elapsed durations large, so the comment's explanation of clock
skew is imprecise. There is no acceptance-blocking reader defect here. These
tests do not establish controller-observed timing bounds; the later acceptance
work still owns those measurements, as the spec at line 662 requires.

The inspected command handler preserves legacy text printing and exit handling,
adds the specified JSON fields for attached commands and retains the detached
envelope. The fixture tests exercise old Linux/macOS/Windows replies, typed
guest outcomes and invalid-record errors. The Windows fixture is derived from
agent source, not a captured Windows reply. The macOS typed records are
synthetic; a live macOS writer has not yet been delivered by k219.

No test, build, lint or format command ran. The producer task lists
`cli:test`, `cli:clippy` and `cli:fmt` as done criteria, and its commit describes
the fixture tests, but neither the task nor the commit description records a
command result. This review therefore makes no claim that those checks passed.
The integration session must run appropriate fresh verification, including
`task cli:test`, after triage and repairs. Guest acceptance remains with its
existing leaves.
