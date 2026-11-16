"""Run the evidence writer with injected file-call faults, for writer controls.

The controls start this module as `python -I writer_faults.py FAULT...`. Each
FAULT has the form `CALL:NAME`. CALL is `close`, `read` or `write`. NAME is a
run-file name. The wrapper makes that call fail with EIO for each descriptor
that the writer opened by NAME. The real close still runs first, so an
injected close fault leaks no descriptor. Calls on other descriptors, such as
the acknowledgement pipe, are unchanged.
"""

from __future__ import annotations

import errno
import os
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))

import writer  # noqa: E402 - the path entry above makes the module importable

CALLS = ("close", "read", "write")


class FaultyOs:
    """The `os` module as the writer sees it, with faults on chosen descriptors."""

    def __init__(self, faults: set[tuple[str, str]]) -> None:
        self.faults = faults
        self.names: dict[int, str] = {}

    def __getattr__(self, attribute: str) -> object:
        return getattr(os, attribute)

    def _fault(self, call: str, descriptor: int) -> None:
        if (call, self.names.get(descriptor)) in self.faults:
            raise OSError(errno.EIO, os.strerror(errno.EIO))

    def open(self, path: str, flags: int, mode: int = 0o777, *, dir_fd: int | None = None) -> int:
        descriptor = os.open(path, flags, mode, dir_fd=dir_fd)
        self.names[descriptor] = os.path.basename(path)
        return descriptor

    def read(self, descriptor: int, count: int) -> bytes:
        self._fault("read", descriptor)
        return os.read(descriptor, count)

    def write(self, descriptor: int, data: bytes | memoryview) -> int:
        self._fault("write", descriptor)
        return os.write(descriptor, data)

    def close(self, descriptor: int) -> None:
        os.close(descriptor)
        name = self.names.pop(descriptor, None)
        if ("close", name) in self.faults:
            raise OSError(errno.EIO, os.strerror(errno.EIO))


def main(argv: list[str]) -> int:
    faults: set[tuple[str, str]] = set()
    for argument in argv:
        call, _, name = argument.partition(":")
        if call not in CALLS or not name:
            print("usage: python -I writer_faults.py CALL:NAME...", file=sys.stderr)
            return 2
        faults.add((call, name))
    writer.os = FaultyOs(faults)  # type: ignore[assignment]
    return writer.run(writer.Writer())


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
