"""Admission controls catch execution or acquisition before unsafe fork rejection."""

import ctypes
import json
import os
import signal
import sys
import threading
import uuid
from pathlib import Path

import pytest

from controller import Call, Plan, Resource, execute

HERE = Path(__file__).resolve().parent


def configured_plan(tmp_path):
    run_id = uuid.uuid4().hex
    identity = f"admission-{run_id}"

    def peer(name, mode):
        return Call(name, (sys.executable, str(HERE / "peer.py"), mode, identity, str(tmp_path)), 1)

    resource = Resource(
        "marker",
        identity,
        peer("acquire", "acquire"),
        peer("cleanup", "bytes"),
        peer("dispose", "dispose"),
        Call("absent", peer("absent", "inspect").argv, 1, expected_stdout=b"absent\n"),
    )
    return Plan(
        run_id,
        (peer("workload", "bytes"),),
        (resource,),
        (HERE / "peer.py", Path(__file__)),
        term_grace=0.05,
        kill_wait=0.3,
    )


def run_plan(tmp_path, plan):
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    path = execute(plan, root)
    return path, json.loads((path / "result.json").read_text())


def assert_rejected(tmp_path, reason):
    plan = configured_plan(tmp_path)
    path, result = run_plan(tmp_path, plan)
    assert result["ok"] is False
    assert result["fork_admission"]["ok"] is False
    assert result["fork_admission"]["reason"] == reason
    assert result["unreached"] == ["acquire", "workload"]
    assert result["calls"] == []
    assert result["resources"] == []
    assert not (tmp_path / plan.resources[0].identity).exists()
    assert not list(path.glob("intent-*.json"))
    assert result["subjects_before"] == result["subjects_after"]
    return result


def test_review_admission_allows_default_single_thread_controller(tmp_path):
    _, result = run_plan(tmp_path, configured_plan(tmp_path))
    assert result["ok"] is True
    assert result["fork_admission"]["ok"] is True
    assert result["fork_admission"]["python_threads"] == 1
    assert result["fork_admission"]["native_threads"] == 1
    assert result["fork_admission"]["sigchld"] == "SIG_DFL"
    assert [item["name"] for item in result["calls"]] == [
        "acquire",
        "workload",
        "cleanup",
        "dispose",
        "absent",
    ]
    assert result["resources"][0]["state"] == "disposed"
    assert str(HERE / "inspection.py") in result["subjects_before"]


def test_review_admission_rejects_live_python_thread_before_acquisition(tmp_path):
    release = threading.Event()
    thread = threading.Thread(target=release.wait)
    thread.start()
    try:
        result = assert_rejected(tmp_path, "python_threads")
        assert result["fork_admission"]["python_threads"] == 2
    finally:
        release.set()
        thread.join(timeout=1)
    assert not thread.is_alive()


def test_review_admission_rejects_native_pthread_absent_from_python_registry(tmp_path):
    # The ctypes callback never calls threading.current_thread(), so it remains
    # outside threading's registry. libproc must still count the real pthread.
    libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib")
    callback_type = ctypes.CFUNCTYPE(ctypes.c_void_p, ctypes.c_void_p)
    ready = threading.Event()
    release = threading.Event()

    @callback_type
    def hold(_):
        ready.set()
        release.wait(timeout=10)
        return None

    libc.pthread_create.argtypes = [
        ctypes.c_void_p,
        ctypes.c_void_p,
        callback_type,
        ctypes.c_void_p,
    ]
    libc.pthread_create.restype = ctypes.c_int
    libc.pthread_join.argtypes = [ctypes.c_void_p, ctypes.c_void_p]
    libc.pthread_join.restype = ctypes.c_int
    thread = ctypes.c_void_p()
    assert libc.pthread_create(ctypes.byref(thread), None, hold, None) == 0
    try:
        assert ready.wait(timeout=1)
        assert threading.active_count() == 1
        result = assert_rejected(tmp_path, "native_threads")
        assert result["fork_admission"]["native_threads"] == 2
    finally:
        release.set()
        assert libc.pthread_join(thread, None) == 0


@pytest.mark.parametrize("handler", [signal.SIG_IGN, lambda _signum, _frame: None])
def test_review_admission_rejects_nondefault_sigchld_before_acquisition(tmp_path, handler):
    previous = signal.signal(signal.SIGCHLD, handler)
    try:
        assert_rejected(tmp_path, "sigchld")
    finally:
        signal.signal(signal.SIGCHLD, previous)


@pytest.mark.parametrize("returned,count", [(0, 1), (-1, 1), (95, 1), (97, 1), (96, 0), (96, -1)])
def test_review_admission_rejects_unavailable_or_malformed_native_observation(
    tmp_path,
    monkeypatch,
    returned,
    count,
):
    import struct

    class NativeQuery:
        def __call__(self, pid, flavor, argument, buffer, size):
            assert (pid, flavor, argument, size) == (os.getpid(), 4, 0, 96)
            values = [0] * 18
            values[15] = count
            ctypes.memmove(buffer, struct.pack("=6Q12i", *values), 96)
            ctypes.set_errno(0)
            return returned

    class NativeLibrary:
        proc_pidinfo = NativeQuery()

    # Adapter fault: a real libproc short read cannot be induced reliably.
    # Keep execute, the ABI parser, receipt writer and acquisition boundary real.
    monkeypatch.setattr("inspection.ctypes.CDLL", lambda *_args, **_kwargs: NativeLibrary())
    result = assert_rejected(tmp_path, "native_observation_error")
    assert result["fork_admission"]["native_threads"] is None
    assert result["fork_admission"]["error"]
    assert result["fork_admission"]["errno"] is not None


def test_review_admission_retains_native_library_error_before_acquisition(tmp_path, monkeypatch):
    import errno

    def unavailable(*_args, **_kwargs):
        raise OSError(errno.ENOENT, "libproc unavailable")

    monkeypatch.setattr("inspection.ctypes.CDLL", unavailable)
    result = assert_rejected(tmp_path, "native_observation_error")
    assert result["fork_admission"]["errno"] == errno.ENOENT
    assert "libproc unavailable" in result["fork_admission"]["error"]


def test_review_admission_retains_missing_native_query_before_acquisition(tmp_path, monkeypatch):
    import errno

    monkeypatch.setattr("inspection.ctypes.CDLL", lambda *_args, **_kwargs: object())
    result = assert_rejected(tmp_path, "native_observation_error")
    assert result["fork_admission"]["errno"] == errno.ENOSYS
