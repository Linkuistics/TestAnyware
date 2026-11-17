"""The lifecycle owner of one guest command (ADR-0024).

One owner starts the command, arbitrates its events, ends its process group and
publishes its record. The source of truth is
docs/specs/guest-command-lifecycle.md, section "Lifecycle".

Two seams exist for the controls that the spec names, and neither is a second
execution backend:

- `OwnerSeams.leader_waiter` builds the leader-wait source. The control for a
  leader status that stays unavailable past K + 1.4 s substitutes it.
- `OwnerSeams.open_capture` opens a capture file for the reader. The control for
  a stalled capture substitutes it.
"""

from __future__ import annotations

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

CAPTURE_ROOT_ENV = "TESTANYWARE_AGENT_CAPTURE_ROOT"


def _ms(seconds: float) -> int:
    """Whole milliseconds, rounded down, never negative."""
    return max(0, int(seconds * 1000))


def _log(message: str) -> None:
    print(f"testanyware-agent: {message}", file=sys.stderr, flush=True)


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
    def wait_exit(self, timeout: float | None) -> bool:
        """Wait until the leader exit status is available. Never reaps."""
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

    def wait_exit(self, timeout: float | None) -> bool:
        end = None if timeout is None else time.monotonic() + max(0.0, timeout)
        if self.pidfd is not None:
            poller = select.poll()
            poller.register(self.pidfd, select.POLLIN)
            while True:
                left = None if end is None else max(0.0, end - time.monotonic())
                if poller.poll(None if left is None else int(left * 1000)):
                    return True
                if end is not None and time.monotonic() >= end:
                    return False
        while True:
            if self.peek() is not None:
                return True
            if end is not None and time.monotonic() >= end:
                return False
            time.sleep(PROBE_INTERVAL_S)

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
    done: bool = False
    error: bool = False


def _finalise_capture(
    paths: list[Path], deadline: float, open_capture: Callable[[Path], BinaryIO]
) -> list[StreamEvidence]:
    sizes: list[int] = []
    for path in paths:
        try:
            sizes.append(path.stat().st_size)
        except OSError:
            sizes.append(-1)
    lock = threading.Lock()
    progress = [_StreamProgress() for _ in paths]

    def work() -> None:
        for path, size, state in zip(paths, sizes, progress, strict=True):
            if size < 0:
                with lock:
                    state.error = True
                continue
            try:
                with open_capture(path) as f:
                    while True:
                        with lock:
                            left = size - len(state.data)
                        if left <= 0:
                            break
                        chunk = f.read(min(READ_CHUNK, left))
                        if not chunk:
                            break
                        with lock:
                            state.data.extend(chunk)
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
    evidence: list[StreamEvidence] = []
    with lock:
        for size, state in zip(sizes, progress, strict=True):
            data = bytes(state.data)
            # Every byte up to `fileBytes` was read, even when no read was needed.
            if state.done or (size >= 0 and len(data) == size):
                evidence.append(StreamEvidence(data, size, True))
            elif state.error:
                evidence.append(StreamEvidence(data, max(size, 0), False, "read_error"))
            else:
                evidence.append(StreamEvidence(data, size, False, "deadline"))
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
            waiter.wait_exit(None)
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


def run_command(
    command: str,
    timeout: float,
    exec_id: str,
    request_start: float,
    seams: OwnerSeams = DEFAULT_SEAMS,
) -> ExecRecord:
    """Start one guest command, own its lifecycle and return its record."""
    try:
        capture_dir = Path(tempfile.mkdtemp(prefix="exec-", dir=_ensure_capture_root()))
    except OSError as e:
        return _spawn_failed(exec_id, e, request_start, time.monotonic())
    paths = [capture_dir / "stdout", capture_dir / "stderr"]
    try:
        return _own(command, timeout, exec_id, request_start, seams, paths)
    finally:
        shutil.rmtree(capture_dir, ignore_errors=True)


def _own(
    command: str,
    timeout: float,
    exec_id: str,
    request_start: float,
    seams: OwnerSeams,
    paths: list[Path],
) -> ExecRecord:
    try:
        with open(paths[0], "wb") as out, open(paths[1], "wb") as err:
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
    deadline = spawn_end + max(0.0, timeout)
    leader_pid = proc.pid
    group_id = leader_pid  # `start_new_session` makes the leader its group leader
    start_time = _leader_start_time(leader_pid)
    waiter = seams.leader_waiter(leader_pid)
    held = False
    try:
        lateness = 0
        while True:
            exited_first = waiter.wait_exit(deadline - time.monotonic())
            # Arbitration: at the deadline, one non-blocking kernel wait comes
            # first, so a command that has finished is never reported as timed out.
            status = waiter.peek()
            outcome_at = time.monotonic()
            if status is not None or outcome_at >= deadline:
                break
        if not exited_first:
            lateness = _ms(outcome_at - deadline)

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
            outcome = Outcome("timed_out", timeout_seconds=timeout)
            termination, held = _terminate(proc, waiter, group_id, outcome_at)
            finalise_deadline = outcome_at + termination.kill_sent_after_ms / 1000 + FINALISE_S

        stdout, stderr = _finalise_capture(paths, finalise_deadline, seams.open_capture)
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
        # The capture directory goes in `run_command` right after this return.
        lifecycle.finalised_after_ms = _ms(time.monotonic() - outcome_at)
        return record
    finally:
        if not held:
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
