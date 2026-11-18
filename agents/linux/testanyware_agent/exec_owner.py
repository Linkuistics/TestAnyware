"""The lifecycle owner of one guest command (ADR-0024).

One owner starts the command, arbitrates its events, ends its process group and
publishes its record. The source of truth is
docs/specs/guest-command-lifecycle.md, section "Lifecycle".

Two seams exist for the controls that the spec names, and neither is a second
execution backend:

- `OwnerSeams.leader_waiter` builds the leader-wait source. The control for a
  leader status that stays unavailable past K + 1.4 s substitutes it.
- `OwnerSeams.open_capture` opens a capture file for the reader. Its path names
  a descriptor that the owner holds. The control for a stalled capture
  substitutes it.

A cancellation reaches the owner through its `CancelSignal` (ADR-0025). The
owner arbitrates it with the leader exit and the deadline, in section
"Arbitration" of the spec.
"""

from __future__ import annotations

import hashlib
import math
import os
import select
import shutil
import signal
import subprocess
import sys
import tempfile
import threading
import time
import uuid
from collections.abc import Callable
from dataclasses import dataclass, field
from pathlib import Path
from typing import BinaryIO, Protocol

from testanyware_agent.models import (
    EMPTY_STREAM,
    ExecRecord,
    LeaderStatus,
    Lifecycle,
    Outcome,
    StreamEvidence,
    Termination,
)

# The schedule keeps 0.1 s of margin under each root bound of 2 s.
GRACE_S = 1.9  # SIGTERM to SIGKILL
STATUS_WAIT_S = 1.4  # SIGKILL to the end of the leader wait and the group probe
FINALISE_S = 1.9  # SIGKILL, or a non-terminated outcome, to the capture deadline
PROBE_INTERVAL_S = 0.01
READ_CHUNK = 1024 * 1024
# The longest single kernel wait. `poll` takes a C int of milliseconds and
# raises OverflowError from about 2147484 s (k234 D5), so a longer wait loops.
MAX_KERNEL_WAIT_S = 60.0

CAPTURE_ROOT_ENV = "TESTANYWARE_AGENT_CAPTURE_ROOT"


def _ms(seconds: float) -> int:
    """Whole milliseconds, rounded down, never negative."""
    return max(0, int(seconds * 1000))


def _log(message: str) -> None:
    print(f"testanyware-agent: {message}", file=sys.stderr, flush=True)


def _seconds(timeout: float) -> float:
    """A timeout in seconds, at least 0. A value past the float range saturates."""
    try:
        return max(0.0, float(timeout))
    except OverflowError:
        return math.inf


# ---- Cancellation signal --------------------------------------------------


class CancelSignal:
    """The cancellation event of one guest command, as its owner receives it.

    A request writes one byte to a pipe that the owner polls beside the leader
    pidfd. The first request keeps its time. `close` runs when the record is
    published. After it, a request changes nothing and writes nothing, so a
    write never reaches a descriptor number that the kernel gave to another
    file. The pipe descriptors are not inheritable (PEP 446), so no command
    holds them.
    """

    def __init__(self) -> None:
        self.wake_fd, self._write_fd = os.pipe()
        self._lock = threading.Lock()
        self._requested_at: float | None = None
        self._closed = False

    @property
    def requested_at(self) -> float | None:
        with self._lock:
            return self._requested_at

    def request(self, now: float) -> None:
        with self._lock:
            if self._closed or self._requested_at is not None:
                return
            self._requested_at = now
            os.write(self._write_fd, b"c")

    def close(self) -> None:
        with self._lock:
            if self._closed:
                return
            self._closed = True
            os.close(self._write_fd)
            os.close(self.wake_fd)


# ---- Capture root ---------------------------------------------------------


def capture_root() -> Path:
    override = os.environ.get(CAPTURE_ROOT_ENV)
    if override:
        return Path(override)
    return Path(tempfile.gettempdir()) / f"testanyware-agent-exec-{os.getuid()}"


def _ensure_capture_root() -> Path:
    root = capture_root()
    root.mkdir(mode=0o700, parents=True, exist_ok=True)
    info = root.lstat()
    if not root.is_dir() or root.is_symlink() or info.st_uid != os.getuid():
        raise PermissionError(f"capture root {root} is not a directory that this agent owns")
    os.chmod(root, 0o700)
    return root


def prepare_capture_root() -> None:
    """Remove the capture directories that an earlier agent left behind."""
    try:
        root = _ensure_capture_root()
    except OSError as e:
        _log(f"cannot prepare the capture root: {e}")
        return
    for child in root.iterdir():
        shutil.rmtree(child, ignore_errors=True)


# ---- Leader-wait source ---------------------------------------------------


def _status_of(result: os.waitid_result) -> LeaderStatus:
    if result.si_code == os.CLD_EXITED:
        return LeaderStatus("exited", result.si_status)
    return LeaderStatus("signalled", result.si_status)


class LeaderWaiter(Protocol):
    def wait_exit(self, timeout: float | None, wake: int | None = None) -> bool:
        """Wait until the leader exit status is available. Never reaps.

        Returns False at the timeout, or earlier when `wake` becomes readable.
        """
        ...

    def peek(self) -> LeaderStatus | None:
        """One non-blocking kernel wait that leaves the leader unreaped."""
        ...

    def reap(self) -> LeaderStatus:
        """Reap the leader. Call only after `peek` returned a status."""
        ...

    def close(self) -> None: ...


class PidfdLeaderWaiter:
    """The kernel leader-wait source: a pidfd and `waitid` with `WNOWAIT`.

    Sources: https://docs.python.org/3.12/library/os.html#os.pidfd_open and
    https://docs.python.org/3.12/library/os.html#os.waitid (`P_PIDFD`, Linux
    5.4 and later). Without a pidfd the waiter polls `waitid` on the PID. The
    leader stays unreaped until `reap`, so its PID keeps naming it.
    """

    def __init__(self, pid: int) -> None:
        self.pid = pid
        self.pidfd: int | None
        try:
            self.pidfd = os.pidfd_open(pid)
        except (AttributeError, OSError):
            self.pidfd = None

    def _waitid(self, options: int) -> os.waitid_result | None:
        if self.pidfd is not None:
            return os.waitid(os.P_PIDFD, self.pidfd, os.WEXITED | options)
        return os.waitid(os.P_PID, self.pid, os.WEXITED | options)

    def wait_exit(self, timeout: float | None, wake: int | None = None) -> bool:
        end = math.inf if timeout is None else time.monotonic() + _seconds(timeout)
        if self.pidfd is not None:
            poller = select.poll()
            poller.register(self.pidfd, select.POLLIN)
            if wake is not None:
                poller.register(wake, select.POLLIN)
            while True:
                left = min(max(0.0, end - time.monotonic()), MAX_KERNEL_WAIT_S)
                # Round up, so that a short remainder never becomes a busy poll of 0 ms.
                ready = {fd for fd, _ in poller.poll(math.ceil(left * 1000))}
                if self.pidfd in ready:
                    return True
                if ready or time.monotonic() >= end:
                    return False
        while True:
            if self.peek() is not None:
                return True
            left = end - time.monotonic()
            if left <= 0:
                return False
            pause = min(PROBE_INTERVAL_S, left)
            if wake is not None:
                if select.select([wake], [], [], pause)[0]:
                    return False
            else:
                time.sleep(pause)

    def peek(self) -> LeaderStatus | None:
        result = self._waitid(os.WNOWAIT | os.WNOHANG)
        return None if result is None else _status_of(result)

    def reap(self) -> LeaderStatus:
        result = self._waitid(0)
        assert result is not None
        return _status_of(result)

    def close(self) -> None:
        if self.pidfd is not None:
            os.close(self.pidfd)
            self.pidfd = None


def _open_capture(path: Path) -> BinaryIO:
    return open(path, "rb")


@dataclass
class OwnerSeams:
    leader_waiter: Callable[[int], LeaderWaiter] = PidfdLeaderWaiter
    open_capture: Callable[[Path], BinaryIO] = _open_capture


DEFAULT_SEAMS = OwnerSeams()


# ---- Group probe ----------------------------------------------------------


def _probe_group(group_id: int) -> str:
    """One signal-0 probe of the group: `empty`, `members` or `unknown`."""
    try:
        os.killpg(group_id, 0)
    except ProcessLookupError:
        return "empty"
    except PermissionError:
        return "unknown"
    return "members"


def _signal_group(group_id: int, signum: int) -> None:
    # Called only while the owner holds the leader unreaped, so the group ID
    # still names the owned group.
    try:
        os.killpg(group_id, signum)
    except OSError as e:
        _log(f"signal {signum} to group {group_id} failed: {e}")


# ---- Capture finalisation -------------------------------------------------


@dataclass
class _StreamProgress:
    data: bytearray = field(default_factory=bytearray)
    digest: hashlib._Hash = field(default_factory=hashlib.sha256)
    done: bool = False
    error: bool = False


def _held_path(fd: int) -> Path:
    """The path that reopens a held capture descriptor (proc(5), /proc/PID/fd).

    The link names the open file even after the command unlinks or replaces
    the capture path, so the command cannot hide its own output.
    """
    return Path(f"/proc/self/fd/{fd}")


def _finalise_capture(
    held: list[int], deadline: float, open_capture: Callable[[Path], BinaryIO]
) -> list[StreamEvidence]:
    """Read each held capture file up to its size at finalisation.

    The sizes come from `fstat` of descriptors that the owner opened before
    the spawn. The worker counts and digests the bytes as it reads (spec,
    section "Capture", step 3). At the deadline the owner abandons the worker,
    and the worker reads nothing more after its current read returns.
    """
    sizes = [os.fstat(fd).st_size for fd in held]
    lock = threading.Lock()
    abandoned = False
    progress = [_StreamProgress() for _ in held]

    def work() -> None:
        for fd, size, state in zip(held, sizes, progress, strict=True):
            with lock:
                # After abandonment the owner can close the held descriptors,
                # and the kernel can give their numbers to other files.
                if abandoned:
                    return
            try:
                with open_capture(_held_path(fd)) as f:
                    while True:
                        with lock:
                            if abandoned:
                                return
                            left = size - len(state.data)
                        if left <= 0:
                            break
                        chunk = f.read(min(READ_CHUNK, left))
                        if not chunk:
                            break
                        with lock:
                            if abandoned:
                                return
                            state.data.extend(chunk)
                            state.digest.update(chunk)
                with lock:
                    state.done = len(state.data) == size
                    state.error = not state.done
            except OSError:
                with lock:
                    state.error = True

    # A stalled read blocks only this daemon worker. The record never waits
    # for it after the deadline.
    worker = threading.Thread(target=work, name="exec-capture", daemon=True)
    worker.start()
    worker.join(max(0.0, deadline - time.monotonic()))
    with lock:
        # The worker checks this flag under the same lock before each change,
        # so the data and the digest below stay consistent and stay fixed.
        abandoned = True
    evidence: list[StreamEvidence] = []
    for size, state in zip(sizes, progress, strict=True):
        data = bytes(state.data)
        digest = state.digest.hexdigest()
        # Every byte up to `fileBytes` was read, even when no read was needed.
        if state.done or len(data) == size:
            evidence.append(StreamEvidence(data, size, True, sha256=digest))
        elif state.error:
            evidence.append(StreamEvidence(data, size, False, "read_error", sha256=digest))
        else:
            evidence.append(StreamEvidence(data, size, False, "deadline", sha256=digest))
    return evidence


# ---- Late reaps -----------------------------------------------------------

# Leaders whose status was unavailable at K + 1.4 s. The owner keeps holding
# each one, so its PID and group ID stay reserved, and reaps it when the kernel
# delivers the status. The published record does not change.
_held_leaders: set[subprocess.Popen[bytes]] = set()
_held_lock = threading.Lock()


def _reap_later(proc: subprocess.Popen[bytes], waiter: LeaderWaiter) -> None:
    with _held_lock:
        _held_leaders.add(proc)

    def work() -> None:
        try:
            while not waiter.wait_exit(None):
                pass
            status = waiter.reap()
            proc.returncode = status.value if status.kind == "exited" else -(status.value or 0)
            _log(f"late reap of leader {proc.pid}: {status.to_dict()}")
        finally:
            waiter.close()
            with _held_lock:
                _held_leaders.discard(proc)

    threading.Thread(target=work, name=f"exec-late-reap-{proc.pid}", daemon=True).start()


def held_leader_count() -> int:
    with _held_lock:
        return len(_held_leaders)


# ---- Leader identity ------------------------------------------------------


def _leader_start_time(pid: int) -> str | None:
    """The start time of the unreaped leader, in clock ticks after boot.

    Field 22 of /proc/PID/stat (proc(5)). The command name in field 2 can hold
    spaces and parentheses, so the fields count from its last parenthesis.
    """
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
        return text[text.rindex(")") + 2 :].split()[19]
    except (OSError, ValueError, IndexError):
        return None


# ---- Owner ----------------------------------------------------------------


def new_exec_id() -> str:
    return str(uuid.uuid4())


def _status_outcome(status: LeaderStatus) -> Outcome:
    if status.kind == "exited":
        return Outcome("exited", exit_code=status.value)
    return Outcome("signalled", signal=status.value)


def _spawn_failed(
    exec_id: str, error: OSError, request_start: float, spawn_end: float
) -> ExecRecord:
    outcome = Outcome("spawn_failed", errno=error.errno or 0, message=str(error))
    return ExecRecord(
        exec_id=exec_id,
        outcome=outcome,
        stdout=EMPTY_STREAM,
        stderr=EMPTY_STREAM,
        lifecycle=Lifecycle(
            leader_pid=None,
            group_id=None,
            leader_start_time=None,
            spawn_ms=_ms(spawn_end - request_start),
            duration_ms=None,
            owner_lateness_ms=0,
            finalised_after_ms=_ms(time.monotonic() - spawn_end),
        ),
        remaining_group=None,
        termination=None,
    )


def cancelled_before_start(exec_id: str, request_start: float, lateness_ms: int) -> ExecRecord:
    """The record of a command that a cancellation ended before its spawn.

    The spec, section "Start", step 3: a tombstoned id gets this record, and no
    process starts. No leader exists, so the leader fields and the duration
    are null, and no termination block exists.
    """
    decided = time.monotonic()
    return ExecRecord(
        exec_id=exec_id,
        outcome=Outcome("cancelled", before_start=True),
        stdout=EMPTY_STREAM,
        stderr=EMPTY_STREAM,
        lifecycle=Lifecycle(
            leader_pid=None,
            group_id=None,
            leader_start_time=None,
            spawn_ms=_ms(decided - request_start),
            duration_ms=None,
            owner_lateness_ms=lateness_ms,
            finalised_after_ms=_ms(time.monotonic() - decided),
        ),
        remaining_group=None,
        termination=None,
    )


def run_command(
    command: str,
    timeout: float,
    exec_id: str,
    request_start: float,
    seams: OwnerSeams = DEFAULT_SEAMS,
    cancel: CancelSignal | None = None,
) -> ExecRecord:
    """Start one guest command, own its lifecycle and return its record.

    `cancel` carries the cancellation requests for this command. Without it,
    only the leader exit and the deadline end the command.
    """
    if cancel is not None and (requested := cancel.requested_at) is not None:
        return cancelled_before_start(exec_id, request_start, _ms(time.monotonic() - requested))
    try:
        capture_dir = Path(tempfile.mkdtemp(prefix="exec-", dir=_ensure_capture_root()))
    except OSError as e:
        return _spawn_failed(exec_id, e, request_start, time.monotonic())
    paths = [capture_dir / "stdout", capture_dir / "stderr"]
    held: list[int] = []
    try:
        return _own(command, timeout, exec_id, request_start, seams, cancel, paths, held)
    finally:
        for fd in held:
            os.close(fd)
        shutil.rmtree(capture_dir, ignore_errors=True)


def _own(
    command: str,
    timeout: float,
    exec_id: str,
    request_start: float,
    seams: OwnerSeams,
    cancel: CancelSignal | None,
    paths: list[Path],
    held: list[int],
) -> ExecRecord:
    try:
        with open(paths[0], "wb") as out, open(paths[1], "wb") as err:
            # The owner holds a read descriptor of each capture file, opened
            # before the spawn. The command can unlink its capture path, but
            # it cannot take the file, its size or its bytes from the owner.
            for path in paths:
                held.append(os.open(path, os.O_RDONLY | os.O_CLOEXEC))
            proc = subprocess.Popen(
                ["/bin/bash", "-c", command],
                stdin=subprocess.DEVNULL,
                stdout=out,
                stderr=err,
                start_new_session=True,
            )
    except OSError as e:
        return _spawn_failed(exec_id, e, request_start, time.monotonic())
    spawn_end = time.monotonic()
    deadline = spawn_end + _seconds(timeout)
    wake = None if cancel is None else cancel.wake_fd
    leader_pid = proc.pid
    group_id = leader_pid  # `start_new_session` makes the leader its group leader
    start_time = _leader_start_time(leader_pid)
    waiter = seams.leader_waiter(leader_pid)
    held_leader = False
    try:
        while True:
            exited_first = waiter.wait_exit(deadline - time.monotonic(), wake)
            # Arbitration: at the deadline or a cancellation, one non-blocking
            # kernel wait comes first, so a command that has finished is never
            # reported as timed out or cancelled.
            status = waiter.peek()
            outcome_at = time.monotonic()
            cancel_at = None if cancel is None else cancel.requested_at
            if status is not None or outcome_at >= deadline or cancel_at is not None:
                break
        # The event that woke the owner is the earlier of the deadline and the
        # cancellation. A leader exit event has no lateness.
        events = [t for t in (deadline, cancel_at) if t is not None and t <= outcome_at]
        event_at = min(events) if events else None
        lateness = 0 if exited_first or event_at is None else _ms(outcome_at - event_at)

        termination: Termination | None = None
        remaining: str | None = None
        if status is not None:
            outcome = _status_outcome(waiter.reap())
            proc.returncode = (
                outcome.exit_code if outcome.kind == "exited" else -(outcome.signal or 0)
            )
            probe = _probe_group(group_id)
            remaining = {"empty": "none", "members": "present"}.get(probe, "unknown")
            finalise_deadline = outcome_at + FINALISE_S
        else:
            if cancel_at is not None and cancel_at == event_at:
                outcome = Outcome("cancelled", before_start=False)
            else:
                outcome = Outcome("timed_out", timeout_seconds=timeout)
            termination, held_leader = _terminate(proc, waiter, group_id, outcome_at)
            finalise_deadline = outcome_at + termination.kill_sent_after_ms / 1000 + FINALISE_S

        stdout, stderr = _finalise_capture(held, finalise_deadline, seams.open_capture)
        lifecycle = Lifecycle(
            leader_pid=leader_pid,
            group_id=group_id,
            leader_start_time=start_time,
            spawn_ms=_ms(spawn_end - request_start),
            duration_ms=_ms(outcome_at - spawn_end),
            owner_lateness_ms=lateness,
            finalised_after_ms=0,
        )
        record = ExecRecord(
            exec_id=exec_id,
            outcome=outcome,
            stdout=stdout,
            stderr=stderr,
            lifecycle=lifecycle,
            remaining_group=remaining,
            termination=termination,
        )
        # The record is now published: its digests, its base64 text and its
        # legacy projection exist, so this time covers them. The capture
        # directory goes in `run_command` right after this return.
        lifecycle.finalised_after_ms = _ms(time.monotonic() - outcome_at)
        return record
    finally:
        if not held_leader:
            waiter.close()


def _terminate(
    proc: subprocess.Popen[bytes], waiter: LeaderWaiter, group_id: int, t0: float
) -> tuple[Termination, bool]:
    """End the owned group in the spec's order. Returns the record and whether
    the leader is still held for a late reap."""
    _signal_group(group_id, signal.SIGTERM)
    term_sent = time.monotonic()
    leader_exit_at: float | None = None
    if waiter.wait_exit(t0 + GRACE_S - time.monotonic()):
        leader_exit_at = time.monotonic()
    # SIGKILL at the leader exit or at T0 + 1.9 s, while the leader is unreaped.
    _signal_group(group_id, signal.SIGKILL)
    kill_at = time.monotonic()
    if leader_exit_at is None and waiter.wait_exit(kill_at + STATUS_WAIT_S - time.monotonic()):
        leader_exit_at = time.monotonic()
    status = waiter.peek() if leader_exit_at is not None else None

    if status is None:
        _log(f"leader {proc.pid} status unavailable at K + {STATUS_WAIT_S} s; holding it")
        _reap_later(proc, waiter)
        termination = Termination(
            term_sent_after_ms=_ms(term_sent - t0),
            kill_sent_after_ms=_ms(kill_at - t0),
            leader_status=LeaderStatus("unavailable"),
            leader_exit_after_ms=None,
            group_empty="no",
            group_empty_after_ms=None,
        )
        return termination, True

    assert leader_exit_at is not None
    reaped = waiter.reap()
    proc.returncode = reaped.value if reaped.kind == "exited" else -(reaped.value or 0)
    # After the reap only signal-0 probes go to the group ID.
    group_empty, empty_at = "no", None
    while True:
        probe = _probe_group(group_id)
        if probe == "empty":
            group_empty, empty_at = "yes", time.monotonic()
            break
        if probe == "unknown":
            group_empty = "unknown"
            break
        if time.monotonic() >= kill_at + STATUS_WAIT_S:
            break
        time.sleep(PROBE_INTERVAL_S)
    termination = Termination(
        term_sent_after_ms=_ms(term_sent - t0),
        kill_sent_after_ms=_ms(kill_at - t0),
        leader_status=reaped,
        leader_exit_after_ms=_ms(leader_exit_at - t0),
        group_empty=group_empty,
        group_empty_after_ms=None if empty_at is None else _ms(empty_at - t0),
    )
    return termination, False
