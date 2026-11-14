"""Admission controls catch execution or acquisition before unsafe fork rejection."""

import ctypes
import json
import os
import signal
import sys
import threading
import time
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
    assert result["fork_admission"]["native_sigchld"] == "SIG_DFL"
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


@pytest.mark.parametrize("failure", ["hang", "stdout", "stderr", "admission"])
def test_review_helper_failure_keeps_execute_safety_and_stops_queries(
    tmp_path, monkeypatch, failure
):
    import controller
    from inspection import capture_helper

    queries = []

    def helper(argv, deadline):
        queries.append(argv)
        if len(queries) != 2:
            return capture_helper(argv, deadline)
        if failure == "admission":
            release = threading.Event()
            thread = threading.Thread(target=release.wait)
            thread.start()
            try:
                return capture_helper(argv, deadline)
            finally:
                release.set()
                thread.join(timeout=1)
        source = (
            "import time; time.sleep(10)"
            if failure == "hang"
            else f"import os;\nwhile True: os.write({1 if failure == 'stdout' else 2},b'x'*16384)"
        )
        return capture_helper((sys.executable, "-c", source), deadline)

    monkeypatch.setattr(controller, "capture_helper", helper)
    plan = configured_plan(tmp_path)
    path, result = run_plan(tmp_path, plan)
    assert result["ok"] is False
    assert result["inspection_failure"]["call"] == "workload"
    assert len(queries) == 2
    assert [item["name"] for item in result["calls"]] == [
        "acquire",
        "workload",
        "cleanup",
        "dispose",
        "absent",
    ]
    assert all(item["reaped"] for item in result["calls"])
    assert result["resources"][0]["state"] == "uncertain"
    assert not (tmp_path / plan.resources[0].identity).exists()
    observation = result["inspection_failure"]["observation"]
    helper_receipt = observation["helper"]
    assert helper_receipt["error"]
    assert helper_receipt["finished_monotonic"] <= helper_receipt["deadline_monotonic"]
    if failure == "admission":
        assert helper_receipt["pid"] is None
        assert helper_receipt["admission"]["reason"] == "python_threads"
    else:
        assert helper_receipt["pid"] is not None
        assert helper_receipt["reaped"] is True
        assert helper_receipt["returncode"] == -signal.SIGKILL
    for receipt in result["calls"][2:]:
        assert receipt["group_observations"][0]["query_attempted"] is False
    (path / "helper-safety-control.json").write_text(
        json.dumps(
            {
                "condition": failure,
                "query_count": len(queries),
                "marker_absent": True,
            },
            indent=2,
        )
    )


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


@pytest.mark.parametrize("native_handler", ["ignore", "handler"])
def test_review_admission_rejects_native_sigchld_hidden_from_python(tmp_path, native_handler):
    # Use the SDK's public Darwin sigaction layout, independently of inspection.
    class Action(ctypes.Structure):
        _fields_ = [
            ("handler", ctypes.c_void_p),
            ("mask", ctypes.c_uint32),
            ("flags", ctypes.c_int),
        ]

    libc = ctypes.CDLL("/usr/lib/libSystem.B.dylib", use_errno=True)
    libc.sigaction.argtypes = [ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p]
    libc.sigaction.restype = ctypes.c_int
    previous = Action()
    assert libc.sigaction(signal.SIGCHLD, None, ctypes.byref(previous)) == 0
    assert previous.handler is None
    assert signal.getsignal(signal.SIGCHLD) == signal.SIG_DFL
    callback_type = ctypes.CFUNCTYPE(None, ctypes.c_int)

    @callback_type
    def handler(_signum):
        pass

    address = 1 if native_handler == "ignore" else ctypes.cast(handler, ctypes.c_void_p).value
    installed = Action(address, 0, 0)
    try:
        assert libc.sigaction(signal.SIGCHLD, ctypes.byref(installed), None) == 0
        observed = Action()
        assert libc.sigaction(signal.SIGCHLD, None, ctypes.byref(observed)) == 0
        assert observed.handler == address
        assert signal.getsignal(signal.SIGCHLD) == signal.SIG_DFL
        result = assert_rejected(tmp_path, "sigchld")
        assert result["fork_admission"]["sigchld"] == "SIG_DFL"
        assert result["fork_admission"]["native_sigchld"] == (
            "SIG_IGN" if native_handler == "ignore" else "handler"
        )
    finally:
        assert libc.sigaction(signal.SIGCHLD, ctypes.byref(previous), None) == 0


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


@pytest.mark.parametrize("failure", ["library", "symbol", "errno", "unexpected_return"])
def test_review_admission_rejects_unavailable_native_sigchld(tmp_path, monkeypatch, failure):
    import errno

    native_loader = ctypes.CDLL

    class Query:
        def __call__(self, signum, action, previous):
            assert signum == signal.SIGCHLD and action is None
            assert previous is not None
            ctypes.set_errno(errno.EINVAL if failure == "errno" else 0)
            return -1 if failure == "errno" else 1

    class Library:
        sigaction = Query()

    def load(path, **kwargs):
        if path != "/usr/lib/libSystem.B.dylib":
            return native_loader(path, **kwargs)
        if failure == "library":
            raise OSError(errno.ENOENT, "libSystem unavailable")
        return object() if failure == "symbol" else Library()

    # Only the native boundary is substituted; libproc and execute remain real.
    monkeypatch.setattr("inspection.ctypes.CDLL", load)
    result = assert_rejected(tmp_path, "native_observation_error")
    assert result["fork_admission"]["native_threads"] == 1
    assert result["fork_admission"]["native_sigchld"] is None
    assert (
        result["fork_admission"]["errno"]
        == {
            "library": errno.ENOENT,
            "symbol": errno.ENOSYS,
            "errno": errno.EINVAL,
            "unexpected_return": errno.EIO,
        }[failure]
    )
    assert result["fork_admission"]["error"]


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


@pytest.mark.parametrize("mode", ["healthy", "partial", "hang", "stdout", "stderr"])
def test_review_helper_capture_bounds_real_pipes_and_reaps(tmp_path, mode):
    from dataclasses import asdict

    from inspection import capture_helper

    source = {
        "healthy": "import os; os.write(1,b'table\\n'); os.write(2,b'note\\n')",
        "partial": "import os; os.write(1,b'partial')",
        "hang": "import time; time.sleep(10)",
        "stdout": "import os;\nwhile True: os.write(1,b'x'*16384)",
        "stderr": "import os;\nwhile True: os.write(2,b'x'*16384)",
    }[mode]
    deadline = time.monotonic() + 0.5
    capture = capture_helper((sys.executable, "-c", source), deadline)
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    receipt = asdict(capture)
    receipt["stdout"] = capture.stdout.hex()
    receipt["stderr"] = capture.stderr.hex()
    (root / f"helper-{mode}-{uuid.uuid4().hex}.json").write_text(json.dumps(receipt, indent=2))
    assert capture.pid is not None
    assert capture.reaped is True
    assert capture.finished_monotonic <= deadline
    assert len(capture.stdout) + len(capture.stderr) <= 256 * 1024
    with pytest.raises(ChildProcessError):
        os.waitpid(capture.pid, os.WNOHANG)
    if mode in {"healthy", "partial"}:
        assert capture.error is None
        assert capture.stdout == (b"table\n" if mode == "healthy" else b"partial")
        assert capture.stderr == (b"note\n" if mode == "healthy" else b"")
        assert capture.returncode == 0
        assert capture.kill_monotonic is None
    else:
        assert capture.error is not None
        assert capture.kill_monotonic is not None
        assert capture.returncode == -signal.SIGKILL


def test_review_helper_residual_ownership_never_uses_blocking_wait(tmp_path, monkeypatch):
    from dataclasses import asdict

    import inspection

    original_waitpid = os.waitpid
    waited = []

    def unreapable(pid, flags):
        assert flags == os.WNOHANG
        waited.append(pid)
        return 0, 0

    def forbidden(*_args, **_kwargs):
        raise AssertionError("helper used implicit blocking wait or destructor poll")

    monkeypatch.setattr(os, "waitpid", unreapable)
    monkeypatch.setattr(inspection._HelperProcess, "wait", forbidden)
    monkeypatch.setattr(inspection._HelperProcess, "_internal_poll", forbidden)
    capture = inspection.capture_helper((sys.executable, "-c", "pass"), time.monotonic() + 0.2)
    assert capture.reaped is False
    assert capture.pid in inspection._helper_owners
    assert waited and set(waited) == {capture.pid}
    owner = inspection._helper_owners[capture.pid]
    owner.__del__()
    # Independent fixture reconciliation after simulated non-observation; the
    # production owner made no blocking wait, retry or post-cutoff signal.
    pid, status = original_waitpid(capture.pid, 0)
    assert pid == capture.pid
    owner.returncode = os.waitstatus_to_exitcode(status)
    del inspection._helper_owners[pid]
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    receipt = asdict(capture)
    receipt.update(stdout=capture.stdout.hex(), stderr=capture.stderr.hex())
    (root / f"helper-simulated-reap-{uuid.uuid4().hex}.json").write_text(
        json.dumps(
            {
                "adapter": "waitpid always returns (0, 0); real child and KILL",
                "capture": receipt,
                "independent_fixture_reap": {"pid": pid, "returncode": owner.returncode},
            },
            indent=2,
        )
    )


def test_review_helper_admission_rejects_before_creation(monkeypatch):
    import inspection

    def forbidden(*_args, **_kwargs):
        raise AssertionError("fork attempted after admission rejection")

    release = threading.Event()
    thread = threading.Thread(target=release.wait)
    thread.start()
    try:
        monkeypatch.setattr(inspection, "_HelperProcess", forbidden)
        capture = inspection.capture_helper(("/bin/ps",), time.monotonic() + 0.5)
        assert capture.pid is None
        assert capture.admission["reason"] == "python_threads"
        assert capture.error
    finally:
        release.set()
        thread.join(timeout=1)
