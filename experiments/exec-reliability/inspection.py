"""Observe initial fork preconditions without creating an inspection subprocess.

Admission is a snapshot, not a lock on the host's thread or signal state.
A future worker implementation must repeat these checks before every fork.
"""

import ctypes
import errno
import os
import signal
import struct
import threading
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
