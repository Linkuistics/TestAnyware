# pipe-capture-k178

**Reviews:** pipe-capture-k167

## Goal

Review the pipe capture adversarially before the publication slice builds on
it.

## Context

Read the producer commit for `pipe-capture-k167`. The changed code is in
`experiments/exec-reliability/controller.py`: `_Stream`, `_Capture`, `_call()`,
`_supervise()`, `_RunStorage.stream()` and the teardown executable check in
`_validate()`. The changed tests are `test_storage_controls.py`,
`test_retention.py` and `test_controller.py`.

The spec is `docs/specs/diagnostic-evidence-storage.md`. Its section
"Workload and teardown descriptors" gives the contract. The section "Channel
and records" gives the frame and outbox rules. The producer leaf states the
done conditions and records each decision.

## Done when

- Check that the controller opens no file for a call after readiness. Check
  that it keeps no write end of a stream pipe after the spawn.
- Check that every pipe end is non-inheritable in the controller. Check that
  the workload, a teardown call, the worker and the helper receive no writer
  pipe end.
- Check that a turn reads at most 64 KiB from each stream. Check that the
  running length and SHA-256 digest cover exactly the bytes in memory.
- Check that the final drain runs after the termination decision and the
  leader reap. Check that it makes at most 16 reads on each stream and never
  waits.
- Check the three completeness values. Check that only `eof` passes, and that
  `writer_open` and `cutoff` fail the call.
- Check that the stdout match reads only the bytes in memory.
- Check the stream frames. Each frame must stay within `writer.MAX_PAYLOAD`,
  and each stream must have one create frame before its append frames.
- Check that an outbox drop on a stream record fails storage and sets the
  forward stop. Check that a stream frame after storage failure counts as
  abandoned.
- Check the digest entry of each stream in the storage section. It must come
  from the bytes in memory, including for a dropped stream.
- Check the teardown executable rule. It must refuse a relative path and a
  path under the evidence root by lexical comparison, with no file system
  access. Look for a path that escapes the comparison, such as `..` segments
  or a root with a trailing separator.
- Check that each new review control fails for its own gap on a mutated build.
  Check that the inherited writer control leaves no live descendant.
- Check that the rewritten retention tests still cover each property that the
  file-backed tests covered.

## Notes

The close wait has no bound until `storage-publication-k168`, so the outbox
overflow control keeps its marker. The producer did not measure the capture
under a stalled writer with large output beyond the overflow control.

## Findings

### F1 — P2: a double-slash evidence root bypasses executable refusal

`experiments/exec-reliability/controller.py:213` retains the root spelling
returned by `os.path.abspath(evidence_root)`. At
`experiments/exec-reliability/controller.py:218`, `os.path.commonpath()` is
compared directly with that spelling. POSIX path normalization preserves exactly
two leading slashes, whereas `commonpath()` removes empty components and emits
one leading slash.

For example, with `evidence_root=Path("//tmp/evidence")` and teardown executable
`//tmp/evidence/tool`, `root` is `//tmp/evidence`, but the common path is
`/tmp/evidence`. The equality is false and the executable is admitted even
though it is lexically beneath the supplied root. On this macOS target that
also permits executing a file from the evidence directory. Cleanup, disposal
and inspection all use this check.

This violates the explicit refusal requirement; it is an acceptance defect,
not optional hardening. Integration should decide the treatment of the
double-slash spelling and cover it with a regression that rejects a descendant
while retaining the no-filesystem-access validation contract. This finding is
derived from the implementation and the installed Python 3.14.10
`posixpath.py` implementations of `abspath`, `normpath`, `splitroot` and
`commonpath`; this review ran no reproducer or test.

## Decisions (running log)

The reviewed producer is `pipe-capture-k167`, commit
`09da783f26922a74af2f2893ab4d7338f93cc508`. Its controller and three test-file
diffs were inspected against the evidence-storage spec. F1 meets the
acceptance threshold, so a sibling integration will precede the live
publication slice. The shared parent already owns this scope; no new node or
ADR is needed. The integration will triage the finding rather than receive it
as an unquestionable work list.

Source inspection supports the other capture requirements: `_Capture` creates
non-inheritable pipes and makes read ends nonblocking; `_call()` closes the
controller write ends immediately after spawning with null stdin and cwd `/`.
Normal turns perform at most one 64 KiB read per stream. Final draining follows
supervision, performs at most 16 reads per stream without waiting, closes the
read ends, and only `eof` permits success. Expected stdout is compared with the
captured bytearray. Each read updates that bytearray, byte count, SHA-256 and
append frame from the same chunk. Stream creation precedes appends, and 64 KiB
chunks fit the writer's 1 MiB payload limit.

The storage path inspected in `controller.py` and `storage_client.py` makes an
outbox drop fail storage and stop forward work; later rejected frames count as
abandoned. Capture closure pins the bytes in memory even after a drop. Writer
spawn uses explicit descriptors and closes the unwanted ends. Worker and helper
fork children strip unrelated descriptors through `reset_after_fork()` before
their operation; close-on-exec alone is not the basis for that conclusion.

The recorded controls exercise final-drain-only buffered output and a lingering
inherited stdout writer. Their companions respectively remove final draining
and make the read blocking, so the assertions/watchdog distinguish those gaps.
The inherited-writer fixture's `finally` kills its escaped descendant's process
group. The retention rewrite retains memory-based comparison, stream integrity
checks and failure on an open writer. This is source inspection of the controls
and producer evidence, not an independent execution or a leak measurement.

The original Notes had the validation order reversed: `execute()` at
`experiments/exec-reliability/controller.py:1498` calls `_validate(plan,
evidence_root)` before resolving the root. The lexical check itself performs no
filesystem access. The later resolve is not a finding against this slice.
The unbounded close remains explicitly assigned to `storage-publication-k168`.

Verification is bounded to the producer diff and its relevant call, storage,
worker/helper and retention dependencies. The graph generation
`2026-11-15T02:19:52Z` was stale for controller/worker/test changes and did not
track several new storage/test files, so coverage checks were followed by direct
source reads. No exhaustive graph assurance is claimed. No test, build, lint or
format command was run, and no production or test code was changed. The
producer's recorded 1071 passing tests, 5 expected failures and clean Ruff and
Pyright results remain producer evidence only.
