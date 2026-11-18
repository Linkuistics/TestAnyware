"""The exec registry: running commands, retained records and tombstones.

The source of truth is docs/specs/guest-command-lifecycle.md, sections
"Cancellation and CLI interruption", "Retention" and "Agent shutdown", and
ADR-0025. The registry knows exec ids and retention. The owner in
`exec_owner` knows the processes. A cancellation passes from the registry to
the owner only through the command's `CancelSignal`.
"""

from __future__ import annotations

import threading
import time
from collections.abc import Callable
from dataclasses import dataclass, field

from testanyware_agent.exec_owner import CancelSignal
from testanyware_agent.models import ExecRecord

RETENTION_S = 300.0  # each terminal record and each tombstone
RETENTION_LIMIT = 64  # terminal records and tombstones together
CANCEL_WAIT_MAX_MS = 10000

RUNNING = "running"
TERMINAL = "terminal"
TOMBSTONE = "tombstone"


@dataclass
class _Entry:
    exec_id: str
    state: str
    since: float  # the retention clock: publication or tombstone time
    signal: CancelSignal | None = None
    record: ExecRecord | None = None


@dataclass
class Admission:
    """The answer to an `/exec` id.

    `kind` is `start` (run the command with `signal`), `tombstoned` (answer a
    cancelled record and spawn nothing), `in_use` or `closed`.
    """

    kind: str
    signal: CancelSignal | None = field(default=None)


class ExecRegistry:
    """One agent's exec ids, under one lock.

    `clock` measures retention ages. A wait for publication always uses the
    monotonic clock.
    """

    def __init__(
        self,
        clock: Callable[[], float] = time.monotonic,
        retention_s: float = RETENTION_S,
        limit: int = RETENTION_LIMIT,
    ) -> None:
        self._clock = clock
        self._retention_s = retention_s
        self._limit = limit
        self._lock = threading.Lock()
        self._changed = threading.Condition(self._lock)
        self._entries: dict[str, _Entry] = {}
        self._closed = False

    # ---- Retention, under the lock ----------------------------------------

    def _evict(self) -> None:
        now = self._clock()
        retained = [e for e in self._entries.values() if e.state != RUNNING]
        for entry in retained:
            if now - entry.since >= self._retention_s:
                del self._entries[entry.exec_id]
        retained = [e for e in self._entries.values() if e.state != RUNNING]
        # A running command is never evicted. The oldest retained entry goes first.
        retained.sort(key=lambda e: e.since)
        for entry in retained[: max(0, len(retained) - self._limit)]:
            del self._entries[entry.exec_id]

    # ---- The owner side ---------------------------------------------------

    def admit(self, exec_id: str) -> Admission:
        """Register an `/exec` id (spec, section "Start", steps 2 and 3)."""
        with self._lock:
            if self._closed:
                return Admission("closed")
            self._evict()
            entry = self._entries.get(exec_id)
            if entry is not None and entry.state != TOMBSTONE:
                return Admission("in_use")
            tombstoned = entry is not None
            signal = CancelSignal()
            self._entries[exec_id] = _Entry(exec_id, RUNNING, self._clock(), signal)
            return Admission("tombstoned" if tombstoned else "start", signal)

    def publish(self, exec_id: str, record: ExecRecord) -> None:
        """Make the record of a running command final and retain it."""
        with self._lock:
            entry = self._entries[exec_id]
            assert entry.state == RUNNING and entry.signal is not None
            entry.signal.close()
            entry.state, entry.record, entry.since = TERMINAL, record, self._clock()
            self._changed.notify_all()
            self._evict()

    def abandon(self, exec_id: str) -> None:
        """Forget a running command whose owner failed without a record."""
        with self._lock:
            entry = self._entries.pop(exec_id, None)
            if entry is not None and entry.signal is not None:
                entry.signal.close()
            self._changed.notify_all()

    def state(self, exec_id: str) -> str | None:
        """`running`, `terminal`, `tombstone`, or None for an id it does not hold."""
        with self._lock:
            self._evict()
            entry = self._entries.get(exec_id)
            return None if entry is None else entry.state

    # ---- The client side --------------------------------------------------

    def cancel(self, exec_id: str, wait_s: float) -> tuple[str, ExecRecord | None]:
        """Cancel by id and wait up to `wait_s` for the record.

        Returns `(state, record)`: `terminal` with the record, `terminating`
        without it, or `unknown_id` without it. A repeat never changes a
        terminal outcome. An unknown id becomes a tombstone.
        """
        with self._lock:
            self._evict()
            entry = self._entries.get(exec_id)
            if entry is None:
                self._entries[exec_id] = _Entry(exec_id, TOMBSTONE, self._clock())
                self._evict()
                return "unknown_id", None
            if entry.state == TOMBSTONE:
                return "unknown_id", None
            if entry.state == RUNNING:
                assert entry.signal is not None
                entry.signal.request(time.monotonic())
                end = time.monotonic() + wait_s
                while entry.state == RUNNING:
                    left = end - time.monotonic()
                    if left <= 0:
                        break
                    self._changed.wait(left)
            if entry.state == TERMINAL:
                return "terminal", entry.record
            return "terminating", None

    # ---- Agent shutdown ---------------------------------------------------

    def close(self) -> list[str]:
        """Stop admission and cancel every running command. Returns their ids."""
        with self._lock:
            self._closed = True
            now = time.monotonic()
            running = [e for e in self._entries.values() if e.state == RUNNING]
            for entry in running:
                assert entry.signal is not None
                entry.signal.request(now)
            return [e.exec_id for e in running]

    def wait_published(self, exec_ids: list[str], until: float) -> list[str]:
        """Wait until each command is published or `until` (monotonic) passes.

        Returns the ids that are still running. An abandoned id counts as done.
        """
        with self._lock:

            def pending() -> list[str]:
                return [
                    i
                    for i in exec_ids
                    if (e := self._entries.get(i)) is not None and e.state == RUNNING
                ]

            while (left := pending()) and (remaining := until - time.monotonic()) > 0:
                self._changed.wait(remaining)
            return left


REGISTRY = ExecRegistry()
