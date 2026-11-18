"""Identify a process by its PID and start time (proc(5), /proc/PID/stat)."""

from __future__ import annotations

import os
import signal
import threading
import time
from pathlib import Path


def _stat_fields(pid: int) -> list[str] | None:
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return None
    # The command name in field 2 can hold spaces and parentheses.
    return text[text.rindex(")") + 2 :].split()


def start_ticks(pid: int) -> str | None:
    fields = _stat_fields(pid)
    return None if fields is None else fields[19]


def process_state(pid: int) -> str | None:
    fields = _stat_fields(pid)
    return None if fields is None else fields[0]


def read_child(path: Path) -> tuple[int, str]:
    pid = int(path.read_text().split()[0])
    ticks = start_ticks(pid)
    assert ticks is not None, "the child had already gone when the test read it"
    return pid, ticks


def child_survives(pid: int, ticks: str) -> bool:
    """True when the same process (PID and start time) still runs."""
    return start_ticks(pid) == ticks and process_state(pid) != "Z"


def wait_for_child(path: Path, limit: float = 5.0) -> tuple[int, str]:
    end = time.monotonic() + limit
    while time.monotonic() < end:
        try:
            if path.read_text().strip():
                return read_child(path)
        except (OSError, ValueError, AssertionError):
            pass
        time.sleep(0.01)
    raise AssertionError("the command never wrote its child PID")


def kill_if_same(pid: int, ticks: str) -> None:
    """Clean up a test child only while its identity still matches."""
    if child_survives(pid, ticks):
        os.kill(pid, signal.SIGKILL)


class ChildRecorder:
    """Read the child PID file while the command runs, before the owner ends it."""

    def __init__(self, path: Path, limit: float = 1.5) -> None:
        self.path = path
        self.limit = limit
        self.child: tuple[int, str] | None = None
        self._thread = threading.Thread(target=self._watch, daemon=True)
        self._thread.start()

    def _watch(self) -> None:
        try:
            self.child = wait_for_child(self.path, self.limit)
        except AssertionError:
            pass

    def result(self) -> tuple[int, str]:
        self._thread.join(self.limit + 0.5)
        assert self.child is not None, "the command never wrote its child PID"
        return self.child
