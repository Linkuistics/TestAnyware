"""Run the evidence writer with one or more injected storage faults.

The writer and storage controls start this module as
`python -I writer_faults.py FAULT...`. Each FAULT has the form `CALL:NAME`.

CALL `close`, `read` or `write` makes that call fail with EIO for each
descriptor that the writer opened by the run-file name NAME. The real close
still runs first, so an injected close fault leaks no descriptor. Calls on
other descriptors, such as the acknowledgement pipe, are unchanged.

CALL `deny` makes the open of NAME fail with EACCES, so the writer rejects
that frame with an error acknowledgement and carries on.

CALL `stall` stops the writer with SIGSTOP when it opens NAME, before the
write. CALL `exit` ends the writer at once with status 70 at the same point.
The name `@start` applies either fault before the writer reads its first
frame. A stopped writer stays stopped until its owner sends KILL.

CALL `report` takes a path instead of a name. Before the first read and before
each acknowledgement, the wrapper appends one JSON line to that path. The line
lists each open descriptor with its kind: `directory`, `fifo`, `null`, `file`
or `other`. The wrapper opens the report only after it takes the snapshot, so
the report descriptor never appears in it.
"""

from __future__ import annotations

import errno
import json
import os
import signal
import stat
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import writer  # noqa: E402 - the path entry above makes the module importable

EIO_CALLS = ("close", "read", "write")
STOP_CALLS = ("stall", "exit")
CALLS = (*EIO_CALLS, *STOP_CALLS, "deny", "report")
START = "@start"
EXIT_STATUS = 70
# The snapshot checks descriptors below this bound. The writer opens few.
DESCRIPTOR_SCAN = 256


def descriptor_kinds() -> dict[int, str]:
    """Return each open descriptor below the scan bound with its kind."""
    null = os.stat(os.devnull).st_rdev
    kinds: dict[int, str] = {}
    for descriptor in range(DESCRIPTOR_SCAN):
        try:
            mode = os.fstat(descriptor)
        except OSError:
            continue
        if stat.S_ISDIR(mode.st_mode):
            kinds[descriptor] = "directory"
        elif stat.S_ISFIFO(mode.st_mode):
            kinds[descriptor] = "fifo"
        elif stat.S_ISCHR(mode.st_mode) and mode.st_rdev == null:
            kinds[descriptor] = "null"
        elif stat.S_ISREG(mode.st_mode):
            kinds[descriptor] = "file"
        else:
            kinds[descriptor] = "other"
    return kinds


def stop(call: str) -> None:
    if call == "stall":
        os.kill(os.getpid(), signal.SIGSTOP)
    else:
        os._exit(EXIT_STATUS)


class Report:
    """Append descriptor snapshots to one path."""

    def __init__(self, path: str | None) -> None:
        self.path = path

    def snapshot(self, point: str) -> None:
        if self.path is None:
            return
        kinds = descriptor_kinds()
        line = json.dumps({"point": point, "descriptors": kinds}).encode() + b"\n"
        flags = os.O_WRONLY | os.O_APPEND | os.O_CREAT | os.O_CLOEXEC
        descriptor = os.open(self.path, flags, 0o600)
        try:
            os.write(descriptor, line)
        finally:
            os.close(descriptor)


class FaultyOs:
    """The `os` module as the writer sees it, with faults on chosen descriptors."""

    def __init__(
        self, faults: set[tuple[str, str]], report: Report | None = None, ack_fd: int = 1
    ) -> None:
        self.faults = faults
        self.names: dict[int, str] = {}
        self.report = report or Report(None)
        self.ack_fd = ack_fd

    def __getattr__(self, attribute: str) -> object:
        return getattr(os, attribute)

    def _fault(self, call: str, descriptor: int) -> None:
        if (call, self.names.get(descriptor)) in self.faults:
            raise OSError(errno.EIO, os.strerror(errno.EIO))

    def open(self, path: str, flags: int, mode: int = 0o777, *, dir_fd: int | None = None) -> int:
        name = os.path.basename(path)
        if ("deny", name) in self.faults:
            raise PermissionError(errno.EACCES, os.strerror(errno.EACCES), name)
        descriptor = os.open(path, flags, mode, dir_fd=dir_fd)
        self.names[descriptor] = name
        for call in STOP_CALLS:
            if (call, name) in self.faults:
                stop(call)
        return descriptor

    def read(self, descriptor: int, count: int) -> bytes:
        self._fault("read", descriptor)
        return os.read(descriptor, count)

    def write(self, descriptor: int, data: bytes | memoryview) -> int:
        if descriptor == self.ack_fd:
            self.report.snapshot("ack")
        self._fault("write", descriptor)
        return os.write(descriptor, data)

    def close(self, descriptor: int) -> None:
        os.close(descriptor)
        name = self.names.pop(descriptor, None)
        if ("close", name) in self.faults:
            raise OSError(errno.EIO, os.strerror(errno.EIO))


def main(argv: list[str]) -> int:
    faults: set[tuple[str, str]] = set()
    report: str | None = None
    for argument in argv:
        call, _, name = argument.partition(":")
        if call not in CALLS or not name:
            print("usage: python -I writer_faults.py CALL:NAME...", file=sys.stderr)
            return 2
        if call == "report":
            report = name
        else:
            faults.add((call, name))
    snapshots = Report(report)
    snapshots.snapshot("start")
    for call in STOP_CALLS:
        if (call, START) in faults:
            stop(call)
    writer.os = FaultyOs(faults, snapshots)  # type: ignore[assignment]
    return writer.run(writer.Writer())


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
