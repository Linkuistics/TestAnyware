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

## Decisions (running log)

The capture lives in `_Capture` and `_Stream` in `controller.py`. Each stream
sends an empty create frame at spawn, then one append frame for each read. The
pin in the storage section comes from the bytes in memory when the capture
closes. Readback of the stored file never decides a receipt.

The wait, termination and reap logic of `_call()` moved into `_supervise()`.
The capture then wraps one function with a clear start and end. The isolated
reap overrun mutation in `test_controller.py` now targets `_supervise`.

The working directory `/` breaks `-c` fixtures that import `controller` from
the old working directory. The self-signal fixture now inserts the experiment
directory into `sys.path`. Calls themselves keep `/`, as the spec requires.

The retention tests encoded the file-backed design, so they now test the
memory-based capture. A late writer cannot change the retained stream, and
both streams report `writer_open`. A count of zero final reads is valid when
the normal turns already saw end of file. Without the stream comparison, the
storage digest entry still catches a late write. Only the removal of both
checks makes `verify` pass.

The adapter `writer_descriptors` reads the `report:PATH` fault of
`writer_faults.py`. It uses only the acknowledgement snapshots, because only
they follow the open of the run directory. The inherited state control checks
both directions. The call holds only its two pipes and the null device. The
writer holds only its three standard descriptors and the run directory.

With `--runxfail`, the outbox overflow control fails by its 12 s watchdog
inside `_RunStorage.close()`. The drop already fails storage and stops forward
progress. The close wait is the only remaining gap, so the reason names only
`storage-publication-k168`.

The buffered output control makes every normal drain turn a no-op, so only the
final drain reads the 8 KiB. A companion test replaces the final drain and
sees the stdout match fail. The inherited writer control starts a descendant
in a new session that holds stdout open. A companion test makes the read ends
blocking and sees the watchdog expire. This shows that the capture adds no
wait for end of file.

Evidence: full suite 1071 passed, 5 xfailed. Ruff check, ruff format and
pyright pass on the experiment's source files.
