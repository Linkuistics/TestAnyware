"""Controller side of the evidence storage channel.

The contract is docs/specs/diagnostic-evidence-storage.md, sections
"Ownership", "Phases and bounds" (P0 and P1) and "Channel contract".

The client spawns the evidence writer before ownership starts, sends the setup
frame and waits for readiness as scheduler turns. After readiness, the outbox
is a turn. Each turn writes at most 256 KiB of frames and reads at most 64 KiB
of acknowledgements. Both pipe ends are nonblocking in the controller, so a
stopped or slow writer never blocks a turn. The client makes no filesystem
call after the spawn. It sends KILL only to the PID from its own spawn, and
only while that PID is unreaped. It reaps only by a nonblocking wait on that
exact PID.
"""

from __future__ import annotations

import errno
import fcntl
import json
import os
import signal
import subprocess
import sys
import time
from collections import deque
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import cast

from turns import Turn, run_turns
from writer import MAX_PAYLOAD, encode_frame

HERE = Path(__file__).resolve().parent
WRITER: tuple[str, ...] = (sys.executable, "-I", str(HERE / "writer.py"))

READINESS_CEILING = 5.0
REAP_GRACE = 2.0
WRITE_BUDGET = 256 * 1024
ACK_BUDGET = 64 * 1024
OUTBOX_CAP = 64 * 1024 * 1024
# A turn yields after its bounded work. This timer is the fallback wake when no
# readiness event arrives. Kill and reap polls use the shorter interval.
IDLE_WAKE = 0.05
REAP_POLL = 0.005


class _WriterProcess(subprocess.Popen[bytes]):
    def __del__(self) -> None:
        # Popen.__del__ polls and reaps. Only the client's exact-PID wait may
        # reap the writer, so the destructor does nothing.
        # https://github.com/python/cpython/blob/v3.12.0/Lib/subprocess.py
        pass


def _promoted_pipe() -> tuple[int, int]:
    """Return a non-inheritable pipe whose ends are both above the standard descriptors."""
    # https://docs.python.org/3.12/library/fcntl.html#fcntl.fcntl
    descriptors = list(os.pipe())
    try:
        for index, fd in enumerate(descriptors):
            if fd < 3:
                descriptors[index] = fcntl.fcntl(fd, fcntl.F_DUPFD_CLOEXEC, 3)
                os.close(fd)
        return descriptors[0], descriptors[1]
    except BaseException:
        for fd in descriptors:
            os.close(fd)
        raise


def _close(fd: int | None) -> None:
    if fd is None:
        return
    try:
        os.close(fd)
    except OSError:
        # Both platforms release the descriptor even when close fails.
        pass


@dataclass
class _Frame:
    seq: int
    name: str
    data: bytes
    offset: int = 0


class StorageClient:
    """Spawn one writer, wait for its readiness and run the outbox as a turn.

    `storage` is `starting`, then `ready`. A readiness failure makes it
    `unavailable`. An `ok` acknowledgement of the close frame makes it
    `closed`. Otherwise, after readiness, a broken frame pipe, a closed
    acknowledgement pipe, a writer exit or a malformed acknowledgement makes
    it `failed`. `first_failure` names the first such cause.
    """

    # Seams for the per-turn budget tests. Production uses the os calls.
    _write = staticmethod(os.write)
    _read = staticmethod(os.read)

    def __init__(
        self,
        command: tuple[str, ...] = WRITER,
        *,
        readiness_bound: float = READINESS_CEILING,
        outbox_cap: int = OUTBOX_CAP,
    ) -> None:
        if not 0 < readiness_bound <= READINESS_CEILING:
            raise ValueError(f"the readiness bound must be in (0, {READINESS_CEILING:g}] s")
        if outbox_cap <= 0:
            raise ValueError("the outbox cap must be positive")
        self.command = command
        self.readiness_bound = readiness_bound
        self.outbox_cap = outbox_cap

        self.storage = "starting"
        self.readiness = "pending"
        self.readiness_reason: str | None = None
        self.readiness_ack: dict[str, object] | None = None
        self.first_failure: str | None = None
        self.spawn_error: str | None = None

        self.pid: int | None = None
        self.spawned_at: float | None = None
        self.readiness_deadline: float | None = None
        self.ready_at: float | None = None
        self.failed_at: float | None = None
        self.killed_at: float | None = None
        self.kill_error: str | None = None
        self.reap_deadline: float | None = None
        self.reaped_at: float | None = None
        self.exit_status: int | None = None
        self.reap_error: str | None = None
        self.residual = False

        self._frame_fd: int | None = None
        self._ack_fd: int | None = None
        self._outbox: deque[_Frame] = deque()
        self.outbox_bytes = 0
        self.drops: dict[str, int] = {}
        self.abandoned: dict[str, int] = {}
        self._seq = 0
        self._sent: dict[int, str] = {}
        self._setup_seq: int | None = None
        self._close_seq: int | None = None
        self._ack_buffer = bytearray()
        self.acks: dict[int, dict[str, object]] = {}
        self.ack_eof = False
        self.turn_written = 0
        self.turn_read = 0

    # P0: spawn ---------------------------------------------------------------

    def spawn(self) -> bool:
        """Start the writer in its own session. Return False on a spawn error.

        Popen waits for the exec, as any command start does. The writer gets
        the frame pipe as standard input, the acknowledgement pipe as standard
        output and the null device as standard error. close_fds closes every
        other descriptor in the child.
        """
        if self.spawned_at is not None:
            raise RuntimeError("the writer is already spawned")
        frame_read, frame_write = _promoted_pipe()
        try:
            ack_read, ack_write = _promoted_pipe()
        except OSError:
            _close(frame_read)
            _close(frame_write)
            raise
        self.spawned_at = time.monotonic()
        self.readiness_deadline = self.spawned_at + self.readiness_bound
        try:
            # https://docs.python.org/3.12/library/subprocess.html#subprocess.Popen
            process = _WriterProcess(
                self.command,
                stdin=frame_read,
                stdout=ack_write,
                stderr=subprocess.DEVNULL,
                cwd="/",
                start_new_session=True,
                close_fds=True,
            )
        except (OSError, subprocess.SubprocessError) as error:
            for fd in (frame_read, frame_write, ack_read, ack_write):
                _close(fd)
            self.spawn_error = f"{type(error).__name__}: {error}"
            self._fail_readiness("spawn_error", time.monotonic())
            return False
        _close(frame_read)
        _close(ack_write)
        self.pid = process.pid
        self._frame_fd = frame_write
        self._ack_fd = ack_read
        os.set_blocking(frame_write, False)
        os.set_blocking(ack_read, False)
        return True

    # P1: readiness -----------------------------------------------------------

    def request_readiness(self, root: str, run: str, plan: bytes, subjects: list[str]) -> None:
        """Enqueue the setup frame. The writer resolves the evidence root."""
        if self._setup_seq is not None:
            raise RuntimeError("the setup frame is already queued")
        seq = self.enqueue("setup", "setup", plan, root=root, run=run, subjects=subjects)
        if seq is None:
            self._fail_readiness("dropped", time.monotonic())
            return
        self._setup_seq = seq

    def start(self, root: str, run: str, plan: bytes, subjects: list[str]) -> bool:
        """Spawn the writer and drive turns until readiness or a reaped failure."""
        if self.spawn():
            self.request_readiness(root, run, plan, subjects)
        run_turns(self.turn(lambda: self.readiness != "pending"))
        return self.storage == "ready"

    def request_close(self) -> int | None:
        """Enqueue the close frame. Its `ok` acknowledgement makes storage `closed`."""
        if self._close_seq is not None:
            raise RuntimeError("the close frame is already queued")
        self._close_seq = self.enqueue("close", "close")
        return self._close_seq

    def kill(self) -> None:
        """Send KILL to the exact unreaped writer PID. Later turns reap it within the grace."""
        self._kill(time.monotonic())

    # Outbox ------------------------------------------------------------------

    def enqueue(self, op: str, name: str, payload: bytes = b"", **fields: object) -> int | None:
        """Serialise one frame into the outbox and return its sequence number.

        `name` is the record name, which counts drops. It becomes the frame's
        `name` field only for record operations. A frame that would exceed the
        cap is dropped and counted. After a storage failure, the frame is
        abandoned and counted. The call never blocks and returns None for an
        unqueued frame.
        """
        if len(payload) > MAX_PAYLOAD:
            raise ValueError(f"a frame payload holds at most {MAX_PAYLOAD} bytes")
        if self.storage in ("failed", "unavailable", "closed"):
            self.abandoned[name] = self.abandoned.get(name, 0) + 1
            return None
        if op in ("create", "append", "result"):
            fields = {"name": name, **fields}
        data = encode_frame(self._seq + 1, op, payload, **fields)
        if self.outbox_bytes + len(data) > self.outbox_cap:
            self.drops[name] = self.drops.get(name, 0) + 1
            return None
        self._seq += 1
        self._outbox.append(_Frame(self._seq, name, data))
        self.outbox_bytes += len(data)
        self._sent[self._seq] = op
        return self._seq

    def ack(self, seq: int) -> dict[str, object] | None:
        return self.acks.get(seq)

    @property
    def queued(self) -> int:
        return len(self._outbox)

    # Turn --------------------------------------------------------------------

    def turn(self, until: Callable[[], bool], *, safety: bool = False) -> Turn:
        """Return the outbox turn. It ends when `until` holds and no kill or
        reap work is due, or when the writer is reaped or left as a residual."""

        def advance() -> float | None:
            wake = self.advance()
            if self._settled() or (until() and not self._reap_due()):
                return None
            return wake

        return Turn(advance, safety=safety, readiness=self.interests)

    def interests(self) -> tuple[tuple[int, ...], tuple[int, ...]]:
        readers = (self._ack_fd,) if self._ack_fd is not None else ()
        writers = (self._frame_fd,) if self._frame_fd is not None and self._outbox else ()
        return readers, writers

    def advance(self) -> float:
        """Run one bounded turn and return the next absolute wake."""
        self.turn_written = 0
        self.turn_read = 0
        self._reap()
        self._read_acks()
        self._write_frames()
        now = time.monotonic()
        if self.readiness == "pending" and self.readiness_deadline is not None:
            if now >= self.readiness_deadline:
                self._fail_readiness("timeout", now)
        if self.pid is not None and self.reaped_at is None and self.storage == "unavailable":
            self._kill(now)
        self._check_reap_deadline(now)
        return self._next_wake(now)

    def _reap_due(self) -> bool:
        return self.killed_at is not None and self.reaped_at is None and not self.residual

    def _settled(self) -> bool:
        # A reaped writer settles only after its acknowledgements drain to EOF.
        reaped = self.reaped_at is not None and self._ack_fd is None
        return self.pid is None or self.residual or reaped

    def _next_wake(self, now: float) -> float:
        if self._reap_due():
            assert self.reap_deadline is not None
            return min(self.reap_deadline, now + REAP_POLL)
        wake = now + IDLE_WAKE
        if self.readiness == "pending" and self.readiness_deadline is not None:
            wake = min(wake, self.readiness_deadline)
        return wake

    # Writes ------------------------------------------------------------------

    def _write_frames(self) -> None:
        if self._frame_fd is None:
            return
        while self._outbox and self.turn_written < WRITE_BUDGET:
            frame = self._outbox[0]
            room = WRITE_BUDGET - self.turn_written
            view = memoryview(frame.data)[frame.offset : frame.offset + room]
            try:
                written = self._write(self._frame_fd, view)
            except BlockingIOError:
                # EAGAIN keeps the frame, including any partial offset.
                return
            except BrokenPipeError:
                self._fail("EPIPE")
                return
            except OSError as error:
                code = errno.errorcode.get(error.errno or 0, "EIO")
                self._fail(code)
                return
            self.turn_written += written
            frame.offset += written
            if frame.offset == len(frame.data):
                self._outbox.popleft()
                self.outbox_bytes -= len(frame.data)

    # Acknowledgements --------------------------------------------------------

    def _read_acks(self) -> None:
        if self._ack_fd is None:
            return
        while self.turn_read < ACK_BUDGET:
            try:
                data = self._read(self._ack_fd, ACK_BUDGET - self.turn_read)
            except BlockingIOError:
                if self.reaped_at is not None:
                    # The reaped writer finished every write before it exited,
                    # so an empty pipe holds nothing more from it. Another
                    # holder of the write end cannot delay the settle.
                    data = b""
                else:
                    break
            except OSError as error:
                self._fail(errno.errorcode.get(error.errno or 0, "EIO"))
                return
            if not data:
                self.ack_eof = True
                _close(self._ack_fd)
                self._ack_fd = None
                self._parse_acks()
                self._fail("writer_exit" if self.reaped_at is not None else "ack_eof")
                return
            self.turn_read += len(data)
            self._ack_buffer += data
        self._parse_acks()

    def _parse_acks(self) -> None:
        while True:
            end = self._ack_buffer.find(b"\n")
            if end < 0:
                return
            line = bytes(self._ack_buffer[:end])
            del self._ack_buffer[: end + 1]
            if not self._accept_ack(line):
                self._fail("protocol")
                return

    def _accept_ack(self, line: bytes) -> bool:
        try:
            decoded: object = json.loads(line)
        except ValueError:
            return False
        if not isinstance(decoded, dict):
            return False
        ack = cast(dict[str, object], decoded)
        seq = ack.get("seq")
        if not isinstance(seq, int) or isinstance(seq, bool) or seq not in self._sent:
            return False
        if seq in self.acks or ack.get("op") != self._sent[seq]:
            return False
        self.acks[seq] = ack
        if seq == self._setup_seq and self.readiness == "pending":
            now = time.monotonic()
            assert self.readiness_deadline is not None
            if now >= self.readiness_deadline:
                self._fail_readiness("timeout", now)
            elif ack.get("status") != "ok":
                self._fail_readiness(str(ack.get("status")), now)
            else:
                self.readiness = "ready"
                self.readiness_reason = "ok"
                self.readiness_ack = ack
                self.ready_at = now
                self.storage = "ready"
        elif seq == self._close_seq and ack.get("status") == "ok" and self.storage == "ready":
            self.storage = "closed"
        return True

    # Failure, kill and reap --------------------------------------------------

    def _fail(self, cause: str) -> None:
        if self.readiness == "pending":
            self._fail_readiness(cause, time.monotonic())
            return
        if self.storage in ("failed", "unavailable", "closed"):
            return
        self.storage = "failed"
        self.first_failure = cause
        self.failed_at = time.monotonic()
        self._abandon()

    def _fail_readiness(self, reason: str, now: float) -> None:
        self.readiness = "failed"
        self.readiness_reason = reason
        self.storage = "unavailable"
        self.first_failure = self.first_failure or reason
        self.failed_at = now
        self._abandon()
        _close(self._ack_fd)
        self._ack_fd = None

    def _abandon(self) -> None:
        for frame in self._outbox:
            self.abandoned[frame.name] = self.abandoned.get(frame.name, 0) + 1
        self._outbox.clear()
        self.outbox_bytes = 0
        _close(self._frame_fd)
        self._frame_fd = None

    def _kill(self, now: float) -> None:
        """Send KILL once to the exact unreaped writer PID and start the reap bound."""
        if self.killed_at is not None or self.pid is None or self.reaped_at is not None:
            return
        # A residual writer is lost or past its reap deadline. A lost PID can
        # belong to another process, so the client never signals it.
        if self.residual:
            return
        self.killed_at = now
        self.reap_deadline = now + REAP_GRACE
        try:
            os.kill(self.pid, signal.SIGKILL)
        except OSError as error:
            self.kill_error = f"{type(error).__name__}: {error}"

    def _reap(self) -> None:
        if self.pid is None or self.reaped_at is not None or self.residual:
            return
        try:
            # https://docs.python.org/3.12/library/os.html#os.waitpid
            pid, status = os.waitpid(self.pid, os.WNOHANG)
        except ChildProcessError as error:
            # Another reaper took the exact PID. The client cannot signal it again.
            self.reap_error = f"{type(error).__name__}: {error}"
            self.residual = True
            self._fail("writer_lost")
            self._release()
            return
        if pid == 0:
            return
        self.reaped_at = time.monotonic()
        self.exit_status = os.waitstatus_to_exitcode(status)
        # The dead writer held the only write end of the acknowledgement pipe,
        # so later turns read its last acknowledgements and then EOF. A close
        # acknowledgement written before the exit still makes storage closed.
        _close(self._frame_fd)
        self._frame_fd = None
        if self._ack_fd is None:
            self._fail("writer_exit")

    def _release(self) -> None:
        """Close both pipe ends after the writer is gone."""
        _close(self._frame_fd)
        self._frame_fd = None
        _close(self._ack_fd)
        self._ack_fd = None

    def _check_reap_deadline(self, now: float) -> None:
        if self._reap_due() and self.reap_deadline is not None and now >= self.reap_deadline:
            self.residual = True
            self.reap_error = self.reap_error or "writer remains unreaped at the reap deadline"
            self._release()
