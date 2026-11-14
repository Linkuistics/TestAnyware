"""Admission and bounded post-creation capture for local inspection helpers.

Admission is a snapshot, not a lock on the host's thread or signal state.
The live controller uses worker_capture to isolate this synchronous helper
engine. Persistent worker scheduling remains a separate obligation.
"""

import ctypes
import errno
import gc
import os
import select
import signal
import struct
import subprocess
import threading
import time
from dataclasses import dataclass, field
from typing import TypedDict

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


class _HelperProcess(subprocess.Popen[bytes]):
    def __del__(self) -> None:
        # CPython's destructor polls and registers live children for later
        # implicit cleanup. Exact-PID ownership stays in _helper_owners instead.
        # https://github.com/python/cpython/blob/3.12/Lib/subprocess.py
        pass


_helper_owners: dict[int, _HelperProcess] = {}


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


def capture_helper(argv: tuple[str, ...], deadline: float) -> HelperCapture:
    """Cap both pipes and spend only the caller's fixed allowance on reap waits.

    Popen creation is synchronous. A late return still gets exact-child KILL
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
        # No context manager, communicate(), wait() or Popen.kill() (which polls).
        # https://docs.python.org/3.12/library/subprocess.html#subprocess.Popen
        process = _HelperProcess(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            close_fds=True,
            bufsize=0,
        )
        _helper_owners[process.pid] = process
        receipt.pid = process.pid
        receipt.created_monotonic = time.monotonic()
        assert process.stdout is not None and process.stderr is not None
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
        receipt.error = str(error)
        if isinstance(error, OSError):
            receipt.error_errno = error.errno
    finally:
        receipt.stdout, receipt.stderr = bytes(output[0]), bytes(output[1])
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
                if stream is not None:
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


def reset_after_fork(allowed: set[int]) -> None:
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
    os.setsid()
    signal.pthread_sigmask(signal.SIG_SETMASK, set())
