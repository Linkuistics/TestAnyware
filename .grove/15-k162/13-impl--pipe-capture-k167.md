# pipe-capture-k167

## Goal

Capture call output through pipes. The controller then opens no file for a
call.

## Context

The spec section is "Workload and teardown descriptors". Today `_call()` in
`controller.py` opens `{name}.stdout` and `{name}.stderr` and gives them to the
child process. The stdout match then reads the file back.

## Done when

- Each call gets pipes for stdout and stderr, the null device for stdin and `/`
  as its working directory.
- A turn drains at most 64 KiB from each stream. It keeps a running length and
  a SHA-256 digest, and sends the bytes to the writer as stream records.
- The stream records keep the names `{name}.stdout` and `{name}.stderr`.
- The stdout match reads from memory, not from a file.
- After the leader is reaped, the final drain makes at most 16 reads of
  64 KiB on each stream.
- Each stream receipt states `eof`, `writer_open` or `cutoff`. Only `eof`
  passes.
- The plan validator refuses a relative teardown executable. It also refuses
  one under the evidence root.
- The buffered output review control passes. It fails on a build without the
  final drain.
- The inherited writer review control passes. It reports `writer_open` and adds
  no wait for end of file.
- The marker comes off the inherited state control, and the control passes.
- The outbox overflow control keeps its marker. Its reason then names only
  `storage-publication-k168`, which bounds the close wait.
- The full suite passes.
