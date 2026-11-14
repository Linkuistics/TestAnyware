"""Admission and bounded post-creation capture for local inspection helpers.

Admission is a snapshot, not a lock on the host's thread or signal state.
The live controller uses worker_capture to isolate this synchronous helper
engine. Persistent worker scheduling remains a separate obligation.
"""

import ctypes
import errno
import fcntl
import gc
import os
import select
import signal
import struct
import threading
import time
import uuid
from collections.abc import Callable
from dataclasses import dataclass, field
from typing import BinaryIO, TypedDict

_TASK_INFO = struct.Struct("=6Q12i")


class ForkAdmission(TypedDict):
    ok: bool
    reason: str
    python_threads: int
    native_threads: int | None
    sigchld: str
    native_sigchld: str | None
    error: str | None
    errno: int | None


def _native_thread_count() -> int:
    # ABI: proc_taskinfo / PROC_PIDTASKINFO, including pti_threadnum.
    # https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/proc_info.h
    library = ctypes.CDLL("/usr/lib/libproc.dylib", use_errno=True)
    try:
        query = library.proc_pidinfo
    except AttributeError as error:
        raise OSError(errno.ENOSYS, "libproc has no proc_pidinfo") from error
    query.argtypes = [ctypes.c_int, ctypes.c_int, ctypes.c_uint64, ctypes.c_void_p, ctypes.c_int]
    query.restype = ctypes.c_int
    buffer = ctypes.create_string_buffer(_TASK_INFO.size)
    ctypes.set_errno(0)
    received = query(os.getpid(), 4, 0, buffer, _TASK_INFO.size)
    if received != _TASK_INFO.size:
        raise OSError(
            ctypes.get_errno() or errno.EIO,
            f"PROC_PIDTASKINFO returned {received} bytes; expected {_TASK_INFO.size}",
        )
    count = int(_TASK_INFO.unpack(buffer.raw)[15])
    if count < 1:
        raise OSError(errno.EPROTO, f"PROC_PIDTASKINFO returned invalid thread count {count}")
    return count


class _Sigaction(ctypes.Structure):
    # Darwin's public sigaction (not the kernel's __sigaction with a trampoline).
    # https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/signal.h
    _fields_ = [("handler", ctypes.c_void_p), ("mask", ctypes.c_uint32), ("flags", ctypes.c_int)]
    handler: int | None
    flags: int


def _native_sigchld() -> str:
    library = ctypes.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
    try:
        query = library.sigaction
    except AttributeError as error:
        raise OSError(errno.ENOSYS, "libSystem has no sigaction") from error
    query.argtypes = [ctypes.c_int, ctypes.POINTER(_Sigaction), ctypes.POINTER(_Sigaction)]
    query.restype = ctypes.c_int
    action = _Sigaction()
    ctypes.set_errno(0)
    received = query(signal.SIGCHLD, None, ctypes.byref(action))
    if received != 0:
        raise OSError(ctypes.get_errno() or errno.EIO, f"sigaction returned {received}")
    # Darwin SA_NOCLDWAIT discards wait status even with a SIG_DFL handler.
    # https://github.com/apple-oss-distributions/xnu/blob/main/bsd/sys/signal.h
    if action.handler is None and action.flags & 0x0020:
        return "SA_NOCLDWAIT"
    return "SIG_DFL" if action.handler is None else "SIG_IGN" if action.handler == 1 else "handler"


def fork_admission() -> ForkAdmission:
    """Fail closed unless Python, native threads and SIGCHLD permit admission."""
    handler = signal.getsignal(signal.SIGCHLD)
    receipt: ForkAdmission = {
        "ok": False,
        "reason": "native_observation_error",
        "python_threads": threading.active_count(),
        "native_threads": None,
        "sigchld": (
            "SIG_DFL"
            if handler == signal.SIG_DFL
            else "SIG_IGN"
            if handler == signal.SIG_IGN
            else "handler"
        ),
        "native_sigchld": None,
        "error": None,
        "errno": None,
    }
    try:
        receipt["native_threads"] = _native_thread_count()
        receipt["native_sigchld"] = _native_sigchld()
    except OSError as error:
        receipt["error"] = str(error)
        receipt["errno"] = error.errno
        return receipt
    if receipt["python_threads"] != 1:
        receipt["reason"] = "python_threads"
    elif receipt["native_threads"] != 1:
        receipt["reason"] = "native_threads"
    elif handler != signal.SIG_DFL or receipt["native_sigchld"] != "SIG_DFL":
        receipt["reason"] = "sigchld"
    else:
        receipt["ok"] = True
        receipt["reason"] = "ready"
    return receipt


@dataclass
class HelperCapture:
    stdout: bytes = b""
    stderr: bytes = b""
    returncode: int | None = None
    pid: int | None = None
    argv: tuple[str, ...] = ()
    admission: ForkAdmission | None = None
    started_monotonic: float = 0
    created_monotonic: float | None = None
    capture_deadline_monotonic: float = 0
    deadline_monotonic: float = 0
    finished_monotonic: float = 0
    kill_monotonic: float | None = None
    kill_errno: int | None = None
    reaped: bool = False
    reaped_monotonic: float | None = None
    reap_errno: int | None = None
    ownership_lost: bool = False
    close_errors: list[str] = field(default_factory=list[str])
    error: str | None = None
    error_errno: int | None = None
    residual_remedy: str | None = None
    worker: dict[str, object] | None = None


@dataclass
class _HelperProcess:
    pid: int
    stdout: BinaryIO
    stderr: BinaryIO
    returncode: int | None = None


_helper_owners: dict[int, _HelperProcess] = {}


class HelperIdentity(TypedDict):
    pid: int
    ppid: int
    pgid: int
    created_monotonic: float


class ForkFailure(TypedDict):
    errno: int
    failed_monotonic: float


def _await_gate(fd: int, token: bytes, deadline: float) -> bool:
    received = bytearray()
    while time.monotonic() < deadline:
        ready, _, _ = select.select([fd], [], [], min(0.005, max(0, deadline - time.monotonic())))
        if not ready:
            continue
        try:
            chunk = os.read(fd, len(token) + 1 - len(received))
        except BlockingIOError:
            continue
        if not chunk:
            return False
        received.extend(chunk)
        if len(received) >= len(token):
            return bytes(received) == token and time.monotonic() < deadline
    return False


def _capture_pipe() -> tuple[int, int]:
    # Keep pipe endpoints above stdio even when the parent closed 0, 1 or 2.
    # https://docs.python.org/3.12/library/fcntl.html#fcntl.fcntl
    descriptors = list(os.pipe())
    try:
        for index, fd in enumerate(descriptors):
            if fd < 3:
                descriptors[index] = fcntl.fcntl(fd, fcntl.F_DUPFD_CLOEXEC, 3)
                os.close(fd)
        os.set_blocking(descriptors[0], False)
        return descriptors[0], descriptors[1]
    except BaseException:
        for fd in descriptors:
            os.close(fd)
        raise


def _helper_child(
    argv: tuple[str, ...], stdout: int, stderr: int, gate: int, token: bytes, deadline: float
) -> None:
    try:
        # Both sources are above stdio; the copies survive exec. Reset closes
        # every other inherited FD and reopens stdin on /dev/null.
        # https://docs.python.org/3.12/library/os.html#os.dup2
        os.dup2(stdout, 1)
        os.dup2(stderr, 2)
        reset_after_fork({1, 2, gate}, new_session=False)
        if not _await_gate(gate, token, deadline):
            os._exit(71)
        os.close(gate)
        # Reset reopens stdin with os.open, which defaults to close-on-exec.
        # https://docs.python.org/3.12/library/os.html#os.set_inheritable
        os.set_inheritable(0, True)
        os.execvp(argv[0], argv)
    finally:
        # Reset/exec failures must never unwind into inherited Python cleanup.
        # https://docs.python.org/3.12/library/os.html#os._exit
        os._exit(70)


def _spawn_helper(
    argv: tuple[str, ...],
    receipt: HelperCapture,
    publish_created: Callable[[HelperIdentity], None] | None = None,
    publish_fork_error: Callable[[ForkFailure], None] | None = None,
) -> _HelperProcess:
    streams: list[BinaryIO] = []
    writes: list[int] = []
    owner: _HelperProcess | None = None
    gate_read: int | None = None
    try:
        gate_read, gate_write = _capture_pipe()
        writes.append(gate_write)
        token = uuid.uuid4().hex.encode()
        for _ in range(2):
            read, write = _capture_pipe()
            writes.append(write)
            try:
                streams.append(os.fdopen(read, "rb", buffering=0))
            except BaseException:
                os.close(read)
                raise
        owner = _HelperProcess(0, streams[0], streams[1])
        # Block delivery until the child has reset handlers and wakeup state.
        # https://docs.python.org/3.12/library/signal.html#signal.pthread_sigmask
        previous = signal.pthread_sigmask(signal.SIG_BLOCK, signal.valid_signals())
        try:
            try:
                pid = os.fork()
            except OSError as error:
                if publish_fork_error is not None and error.errno is not None:
                    try:
                        publish_fork_error(
                            {"errno": error.errno, "failed_monotonic": time.monotonic()}
                        )
                    except OSError as publication_error:
                        receipt.error = f"fork failure publication failed: {publication_error}"
                raise
            if pid == 0:
                _helper_child(
                    argv, writes[1], writes[2], gate_read, token, receipt.capture_deadline_monotonic
                )
                os._exit(70)
            # Register before restoring signals or performing fallible cleanup.
            owner.pid = pid
            _helper_owners[pid] = owner
            receipt.pid = pid
            receipt.created_monotonic = time.monotonic()
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous)
        if publish_created is not None:
            publish_created(
                {
                    "pid": pid,
                    "ppid": os.getpid(),
                    "pgid": os.getpgrp(),
                    "created_monotonic": receipt.created_monotonic,
                }
            )
        if time.monotonic() >= receipt.capture_deadline_monotonic:
            raise TimeoutError("helper gate deadline exhausted")
        # One private pipe, one token below PIPE_BUF; there is no controller ACK.
        # https://docs.python.org/3.12/library/os.html#os.write
        if os.write(gate_write, token) != len(token):
            raise OSError(errno.EIO, "incomplete helper gate release")
        return owner
    finally:
        if gate_read is not None:
            writes.append(gate_read)
        for fd in writes:
            try:
                os.close(fd)
            except OSError as error:
                receipt.close_errors.append(str(error))
                receipt.error = receipt.error or f"helper pipe close failed: {error}"
        if owner is None or owner.pid == 0:
            for stream in streams:
                try:
                    stream.close()
                except OSError as error:
                    receipt.close_errors.append(str(error))


def _reap_helper(process: _HelperProcess, receipt: HelperCapture) -> None:
    if receipt.reaped or receipt.reap_errno is not None:
        return
    try:
        # https://docs.python.org/3.12/library/os.html#os.waitpid
        pid, status = os.waitpid(process.pid, os.WNOHANG)
    except OSError as error:
        receipt.reap_errno = error.errno
        receipt.error = f"helper reap failed: {error}"
        if error.errno == errno.ECHILD:
            receipt.ownership_lost = True
            _helper_owners.pop(process.pid, None)
        return
    if pid == process.pid:
        process.returncode = os.waitstatus_to_exitcode(status)
        receipt.returncode = process.returncode
        receipt.reaped = True
        receipt.reaped_monotonic = time.monotonic()
        del _helper_owners[pid]


def capture_helper(
    argv: tuple[str, ...],
    deadline: float,
    *,
    publish_created: Callable[[HelperIdentity], None] | None = None,
    publish_fork_error: Callable[[ForkFailure], None] | None = None,
) -> HelperCapture:
    """Cap both pipes and spend only the caller's fixed allowance on reap waits.

    Fork creation is synchronous. A late return still gets exact-child KILL
    and a final nonblocking observation, without a new wait allowance. Scheduler
    lateness is recorded as failure. The live controller invokes this engine
    inside worker_capture's per-query child, whose parent bounds its own waits.
    """
    started = time.monotonic()
    receipt = HelperCapture(
        argv=argv,
        started_monotonic=started,
        deadline_monotonic=deadline,
        capture_deadline_monotonic=deadline - min(0.05, max(0, deadline - started) / 4),
    )
    receipt.admission = fork_admission()
    if not receipt.admission["ok"]:
        receipt.error = f"helper fork admission rejected: {receipt.admission['reason']}"
        receipt.finished_monotonic = time.monotonic()
        return receipt
    if time.monotonic() >= receipt.capture_deadline_monotonic:
        receipt.error = "helper capture deadline exhausted before creation"
        receipt.finished_monotonic = time.monotonic()
        return receipt
    process: _HelperProcess | None = None
    output = [bytearray(), bytearray()]
    try:
        if publish_fork_error is not None:
            process = _spawn_helper(argv, receipt, publish_created, publish_fork_error)
        elif publish_created is not None:
            process = _spawn_helper(argv, receipt, publish_created)
        else:
            process = _spawn_helper(argv, receipt)
        streams = {process.stdout.fileno(): 0, process.stderr.fileno(): 1}
        for fd in streams:
            os.set_blocking(fd, False)
        while time.monotonic() < receipt.capture_deadline_monotonic:
            _reap_helper(process, receipt)
            if receipt.reap_errno is not None or (receipt.reaped and not streams):
                break
            remaining = receipt.capture_deadline_monotonic - time.monotonic()
            if remaining <= 0:
                break
            # One read per ready stream (at most 32 KiB per turn), with a
            # deadline check between reads. No ready pipe can monopolise a turn.
            # https://docs.python.org/3.12/library/select.html#select.select
            ready, _, _ = select.select(list(streams), [], [], min(0.005, remaining))
            for fd in ready:
                if time.monotonic() >= receipt.capture_deadline_monotonic:
                    break
                available = 256 * 1024 - sum(map(len, output))
                try:
                    chunk = os.read(fd, min(16384, available + 1))
                except BlockingIOError:
                    continue
                if not chunk:
                    del streams[fd]
                else:
                    output[streams[fd]].extend(chunk[:available])
                    if len(chunk) > available:
                        raise ValueError("ps output limit exceeded")
        if receipt.error is None and not (receipt.reaped and not streams):
            receipt.error = "helper capture deadline exhausted"
    except (OSError, ValueError) as error:
        receipt.error = f"{error}; {receipt.error}" if receipt.error else str(error)
        if isinstance(error, OSError):
            receipt.error_errno = error.errno
    finally:
        receipt.stdout, receipt.stderr = bytes(output[0]), bytes(output[1])
        if process is None and receipt.pid is not None:
            # A parent-side failure after fork still owns this exact child.
            process = _helper_owners.get(receipt.pid)
        if process is not None:
            # Only an unreaped child with retained wait ownership pins the PID.
            # A wait error closes signal authority even though the owner remains.
            if not receipt.reaped and receipt.reap_errno is None:
                receipt.kill_monotonic = time.monotonic()
                try:
                    os.kill(process.pid, signal.SIGKILL)
                except OSError as error:
                    receipt.kill_errno = error.errno
                    receipt.error = receipt.error or f"helper KILL failed: {error}"
            for stream in (process.stdout, process.stderr):
                try:
                    stream.close()
                except (OSError, ValueError) as error:
                    receipt.close_errors.append(str(error))
                    receipt.error = receipt.error or f"helper pipe close failed: {error}"
            while not receipt.reaped and receipt.reap_errno is None and time.monotonic() < deadline:
                _reap_helper(process, receipt)
                if not receipt.reaped:
                    time.sleep(min(0.005, max(0, deadline - time.monotonic())))
            # Observe a child that exited during the last sleep, or late creation.
            # This adds no wait allowance and never reopens lost wait ownership.
            _reap_helper(process, receipt)
            if not receipt.reaped and not receipt.ownership_lost:
                receipt.error = receipt.error or "helper unreaped before fixed deadline"
                receipt.residual_remedy = (
                    "Strong helper ownership retained without further signals or waits. "
                    "Independently validate this PID, parent and argv "
                    "before external reconciliation."
                )
        receipt.finished_monotonic = time.monotonic()
        if receipt.finished_monotonic > deadline:
            receipt.error = receipt.error or "helper fixed deadline exceeded"
    return receipt


def reset_after_fork(allowed: set[int], *, new_session: bool = True) -> None:
    """Reset the isolated fork child only; never call this in the controller."""
    # Signals remain blocked until every inherited handler/wakeup FD is reset.
    # https://docs.python.org/3.12/library/signal.html#signal.pthread_sigmask
    gc.disable()
    signal.set_wakeup_fd(-1)
    for sig in signal.valid_signals() - {signal.SIGKILL, signal.SIGSTOP}:
        signal.signal(sig, signal.SIG_DFL)
    global _helper_owners
    _helper_owners = {}
    # /dev/fd enumeration itself closes its directory FD before this loop.
    for name in os.listdir("/dev/fd"):
        fd = int(name)
        if fd not in allowed:
            try:
                os.close(fd)
            except OSError as error:
                if error.errno != errno.EBADF:
                    raise
    for fd in (0, 1, 2):
        if fd in allowed:
            continue
        null = os.open(os.devnull, os.O_RDWR)
        if null != fd:
            os.dup2(null, fd)
            os.close(null)
    # Workers isolate themselves; helpers retain the worker's signal group.
    if new_session:
        os.setsid()
    signal.pthread_sigmask(signal.SIG_SETMASK, set())
