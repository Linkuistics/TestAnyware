"""Admission controls catch execution or acquisition before unsafe fork rejection."""

import ctypes
import errno
import json
import os
import signal
import sys
import threading
import time
import uuid
from contextlib import contextmanager
from dataclasses import asdict
from pathlib import Path

import pytest

import inspection
import worker_capture
from controller import Call, Plan, Resource, execute

HERE = Path(__file__).resolve().parent


@contextmanager
def native_auto_reap(flags):
    # Independent public Darwin ABI and literal SDK flags, not inspection's type.
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
    assert previous.handler is None and not previous.flags & 0x0020
    assert signal.getsignal(signal.SIGCHLD) == signal.SIG_DFL
    installed = Action(None, 0, flags)
    try:
        assert libc.sigaction(signal.SIGCHLD, ctypes.byref(installed), None) == 0
        observed = Action()
        assert libc.sigaction(signal.SIGCHLD, None, ctypes.byref(observed)) == 0
        assert observed.handler is None and observed.flags & 0x0020
        assert signal.getsignal(signal.SIGCHLD) == signal.SIG_DFL
        # Fail RED before a broken admission can launch an auto-reaped child.
        assert inspection.fork_admission()["ok"] is False
        yield
    finally:
        assert libc.sigaction(signal.SIGCHLD, ctypes.byref(previous), None) == 0
        restored = Action()
        assert libc.sigaction(signal.SIGCHLD, None, ctypes.byref(restored)) == 0
        assert (restored.handler, restored.mask, restored.flags) == (
            previous.handler,
            previous.mask,
            previous.flags,
        )


@pytest.mark.parametrize("flags", [0x0020, 0x0028])
@pytest.mark.parametrize("boundary", ["acquisition", "worker", "helper"])
def test_review_native_auto_reap_rejected_before_creation(tmp_path, flags, boundary):
    with native_auto_reap(flags):
        if boundary == "acquisition":
            admission = assert_rejected(tmp_path, "sigchld")["fork_admission"]
        else:
            capture = (
                worker_capture.capture_helper if boundary == "worker" else inspection.capture_helper
            )
            receipt = capture(("/usr/bin/true",), time.monotonic() + 1)
            admission = receipt.admission
            assert receipt.error and "sigchld" in receipt.error
            assert receipt.pid is None and receipt.created_monotonic is None
            assert receipt.worker is None
            assert receipt.stdout == receipt.stderr == b""
            root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
            root.mkdir(parents=True, exist_ok=True)
            path = root / f"native-auto-reap-{boundary}-{flags}-{time.time_ns()}.json"
            path.write_text(json.dumps(asdict(receipt), default=lambda value: value.hex()) + "\n")
        assert admission["ok"] is False
        assert admission["native_sigchld"] == "SA_NOCLDWAIT"
        assert admission["sigchld"] == "SIG_DFL"


@pytest.mark.parametrize("flags", [0x0020, 0x0028])
def test_review_worker_transports_real_native_auto_reap_refusal(tmp_path, monkeypatch, flags):
    original = inspection.capture_helper

    def auto_reaping_capture(argv, deadline):
        with native_auto_reap(flags):
            return original(argv, deadline)

    monkeypatch.setattr(inspection, "capture_helper", auto_reaping_capture)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 1)
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    path = root / f"native-auto-reap-worker-refusal-{flags}-{time.time_ns()}.json"
    path.write_text(json.dumps(asdict(receipt), default=lambda value: value.hex()) + "\n")
    assert receipt.worker["reaped"] and not receipt.worker["unknown_helper_creation"]
    assert receipt.error == "helper fork admission rejected: sigchld"
    assert receipt.admission["native_sigchld"] == "SA_NOCLDWAIT"
    assert receipt.admission["ok"] is False
    assert receipt.pid is None and receipt.created_monotonic is None
    assert receipt.stdout == receipt.stderr == b""


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


def reconcile_helper(pid):
    # Independent test-fixture cleanup: establish exact-child ownership before
    # any signal. Never signal a PID after ECHILD or after observing its exit.
    for attempt in range(200):
        try:
            waited, _status = os.waitpid(pid, os.WNOHANG)
        except ChildProcessError:
            return
        if waited == pid:
            return
        if attempt == 0:
            os.kill(pid, signal.SIGKILL)
        time.sleep(0.005)
    raise AssertionError(f"test fixture could not reap helper {pid}")


def retain_helper_control(tmp_path, condition, capture, observations=None):
    from dataclasses import asdict

    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    receipt = asdict(capture)
    receipt.update(stdout=capture.stdout.hex(), stderr=capture.stderr.hex())
    (root / f"helper-{condition}-{uuid.uuid4().hex}.json").write_text(
        json.dumps(
            {"condition": condition, "capture": receipt, "observations": observations}, indent=2
        )
    )


@pytest.mark.parametrize("failed_pipe", ["stdout", "stderr"])
def test_review_helper_close_error_preserves_receipt_and_reaps(tmp_path, monkeypatch, failed_pipe):
    import inspection

    original_process = inspection._spawn_helper
    created = []

    class FaultedClose:
        def __init__(self, stream):
            self.stream = stream

        def fileno(self):
            return self.stream.fileno()

        def close(self):
            self.stream.close()
            raise OSError(errno.EIO, "injected pipe close failure")

    def create(*args, **kwargs):
        process = original_process(*args, **kwargs)
        created.append(process)
        setattr(process, failed_pipe, FaultedClose(getattr(process, failed_pipe)))
        return process

    monkeypatch.setattr(inspection, "_spawn_helper", create)
    try:
        capture = inspection.capture_helper(
            (sys.executable, "-c", "import time; time.sleep(10)"), time.monotonic() + 0.3
        )
        assert capture.reaped is True
        assert capture.returncode == -signal.SIGKILL
        assert capture.finished_monotonic >= capture.reaped_monotonic
        assert any("injected pipe close failure" in error for error in capture.close_errors)
        other = "stderr" if failed_pipe == "stdout" else "stdout"
        assert getattr(created[0], other).closed
        assert capture.pid not in inspection._helper_owners
        retain_helper_control(tmp_path, f"close-error-{failed_pipe}-adapter", capture)
    finally:
        # Independent cleanup also runs when the old implementation raises.
        for process in created:
            reconcile_helper(process.pid)
            for stream in (process.stdout, process.stderr):
                (stream.stream if isinstance(stream, FaultedClose) else stream).close()
            inspection._helper_owners.pop(process.pid, None)


def test_review_helper_last_sleep_gets_final_nonblocking_observation(tmp_path, monkeypatch):
    from types import SimpleNamespace

    import inspection

    # Clock/wait adapter isolates the final-sleep race; no real child or signal.
    now = 100.0
    waited = []
    process = SimpleNamespace(
        pid=987654321,
        stdout=(tmp_path / "stdout").open("w+b"),
        stderr=(tmp_path / "stderr").open("w+b"),
        returncode=None,
    )

    def create(_argv, receipt):
        nonlocal now
        now = 100.99
        inspection._helper_owners[process.pid] = process
        receipt.pid = process.pid
        receipt.created_monotonic = now
        return process

    def waitpid(pid, flags):
        assert (pid, flags) == (process.pid, os.WNOHANG)
        waited.append(now)
        return (pid, 9) if now >= 101 else (0, 0)

    def sleep(seconds):
        nonlocal now
        now += seconds

    monkeypatch.setattr(inspection, "_spawn_helper", create)
    monkeypatch.setattr(inspection, "time", SimpleNamespace(monotonic=lambda: now, sleep=sleep))
    monkeypatch.setattr(os, "waitpid", waitpid)
    monkeypatch.setattr(os, "kill", lambda pid, sig: None)
    try:
        capture = inspection.capture_helper(("timing-adapter",), 101)
        assert capture.reaped is True
        assert capture.returncode == -signal.SIGKILL
        assert waited[-1] == 101
        assert capture.residual_remedy is None
        assert capture.finished_monotonic == 101
        retain_helper_control(tmp_path, "final-observation-clock-wait-adapter", capture)
    finally:
        inspection._helper_owners.pop(process.pid, None)


def test_review_helper_late_creation_signals_pinned_child_without_new_allowance(
    tmp_path, monkeypatch
):
    import inspection

    original_process = inspection._spawn_helper
    created = []

    def late_create(*args, **kwargs):
        time.sleep(0.1)
        process = original_process(*args, **kwargs)
        created.append(process)
        return process

    monkeypatch.setattr(inspection, "_spawn_helper", late_create)
    deadline = time.monotonic() + 0.05
    try:
        capture = inspection.capture_helper(
            (sys.executable, "-c", "import time; time.sleep(10)"), deadline
        )
        assert capture.created_monotonic > deadline
        assert capture.kill_monotonic is not None
        assert capture.kill_monotonic >= capture.created_monotonic
        assert capture.finished_monotonic > deadline
        assert capture.error
        assert capture.deadline_monotonic == deadline
        assert capture.stdout == capture.stderr == b""
        retain_helper_control(tmp_path, "late-creation-real-child-delay-adapter", capture)
    finally:
        for process in created:
            if process.pid in inspection._helper_owners:
                os.kill(process.pid, signal.SIGKILL)
            reconcile_helper(process.pid)
            inspection._helper_owners.pop(process.pid, None)


def test_review_helper_echild_releases_stale_owner_without_claiming_exit(tmp_path, monkeypatch):
    import inspection

    original_waitpid = os.waitpid

    def externally_reaped(pid, flags):
        assert flags == os.WNOHANG
        # Models a competing reaper, not permission denial.
        assert original_waitpid(pid, 0)[0] == pid
        raise ChildProcessError(errno.ECHILD, "injected competing reaper")

    def forbidden(*_args):
        raise AssertionError("signal after lost wait ownership")

    monkeypatch.setattr(os, "waitpid", externally_reaped)
    monkeypatch.setattr(os, "kill", forbidden)
    capture = inspection.capture_helper((sys.executable, "-c", "pass"), time.monotonic() + 0.5)
    try:
        assert capture.reap_errno == errno.ECHILD
        assert capture.pid not in inspection._helper_owners
        assert capture.ownership_lost is True
        assert capture.reaped is False
        assert capture.returncode is None
        assert capture.kill_monotonic is None
        assert capture.residual_remedy is None
        assert capture.error
        retain_helper_control(tmp_path, "echild-competing-reaper-adapter", capture)
    finally:
        inspection._helper_owners.pop(capture.pid, None)


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
    import subprocess

    monkeypatch.setattr(subprocess.Popen, "__init__", forbidden)
    capture = inspection.capture_helper((sys.executable, "-c", "pass"), time.monotonic() + 0.2)
    assert capture.reaped is False
    assert capture.pid in inspection._helper_owners
    assert waited and set(waited) == {capture.pid}
    owner = inspection._helper_owners[capture.pid]
    assert not isinstance(owner, subprocess.Popen)
    assert not hasattr(owner, "__del__")
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
        monkeypatch.setattr(inspection, "_spawn_helper", forbidden)
        capture = inspection.capture_helper(("/bin/ps",), time.monotonic() + 0.5)
        assert capture.pid is None
        assert capture.admission["reason"] == "python_threads"
        assert capture.error
    finally:
        release.set()
        thread.join(timeout=1)


def test_review_worker_group_kill_reaches_live_raw_helper(tmp_path):
    import select

    import inspection
    import worker_capture

    # A helper setsid regression must leave this actual exec child alive after
    # the real worker-group KILL. Kernel exit notification is independent of
    # worker status and killpg's return; it is not helper reap acknowledgement.
    marker, release = tmp_path / "helper.json", tmp_path / "release"
    source = """
import json, os, sys, time
from pathlib import Path
marker, release = map(Path, sys.argv[1:])
state = dict(pid=os.getpid(), ppid=os.getppid(), pgid=os.getpgrp(), sid=os.getsid(0))
temporary = marker.with_suffix('.tmp')
temporary.write_text(json.dumps(state))
temporary.replace(marker)
end = time.monotonic() + 6
while not release.exists() and time.monotonic() < end:
    time.sleep(.005)
"""
    receipt = {
        "real": "production worker _child, helper admission/fork/reset/exec, group SIGKILL, kqueue",
        "fixture": "Python helper instead of ps; file readiness and cooperative cleanup only",
        "bounds": {"ready_seconds": 1, "exit_seconds": 0.5, "cleanup_seconds": 2},
        "helper_reap_acknowledgement": "unavailable after worker death",
        "helper": None,
        "helper_exit_event": None,
        "worker_returncode": None,
    }
    assert inspection.fork_admission()["ok"]
    control_read, control_write = os.pipe()
    result_read, result_write = os.pipe()
    queue = select.kqueue()
    worker = None
    reaped = False
    try:
        previous = signal.pthread_sigmask(signal.SIG_BLOCK, signal.valid_signals())
        try:
            worker = os.fork()
            if worker == 0:
                worker_capture._child(
                    control_write,
                    result_write,
                    "group-kill-control",
                    (sys.executable, "-c", source, str(marker), str(release)),
                    time.monotonic() + 5,
                )
                os._exit(70)
        finally:
            signal.pthread_sigmask(signal.SIG_SETMASK, previous)
        receipt["worker_pid"] = worker
        end = time.monotonic() + 1
        while not marker.exists() and time.monotonic() < end:
            time.sleep(0.005)
        assert marker.exists(), "real helper did not become ready within fixed bound"
        helper = json.loads(marker.read_text())
        receipt["helper"] = helper
        ready_fds, _, _ = select.select([control_read], [], [], max(0, end - time.monotonic()))
        assert ready_fds, "worker readiness missing"
        ready = json.loads(os.read(control_read, 4096).splitlines()[0])
        receipt["worker_ready"] = ready
        assert ready["pid"] == ready["pgid"] == ready["sid"] == worker
        assert os.getpgid(worker) == os.getsid(worker) == worker != os.getpgrp()
        assert helper["ppid"] == worker
        assert os.getpgid(helper["pid"]) == helper["pgid"]
        assert os.getsid(helper["pid"]) == helper["sid"]
        change = select.kevent(
            helper["pid"],
            filter=select.KQ_FILTER_PROC,
            flags=select.KQ_EV_ADD | select.KQ_EV_ONESHOT,
            fflags=select.KQ_NOTE_EXIT,
        )
        assert queue.control([change], 1, 0) == [], "helper already exited"
        receipt["kill_started_monotonic"] = time.monotonic()
        os.killpg(worker, signal.SIGKILL)  # Direct child remains unreaped, pinning the group.
        receipt["kill_returned_monotonic"] = time.monotonic()
        events = queue.control(None, 1, 0.5)
        receipt["observation_finished_monotonic"] = time.monotonic()
        if events:
            event = events[0]
            receipt["helper_exit_event"] = dict(
                pid=event.ident, filter=event.filter, fflags=event.fflags, data=event.data
            )
    finally:
        # Fixture reconciliation only. Release also handles the deliberately red
        # detached-helper case without signalling an orphan's potentially reused PID.
        release.touch()
        end = time.monotonic() + 2
        if worker is not None:
            waited, status = os.waitpid(worker, os.WNOHANG)
            if waited == 0:
                os.kill(worker, signal.SIGKILL)
            while waited == 0 and time.monotonic() < end:
                time.sleep(0.005)
                waited, status = os.waitpid(worker, os.WNOHANG)
            reaped = waited == worker
            if reaped:
                receipt["worker_returncode"] = os.waitstatus_to_exitcode(status)
        if receipt["helper"] is not None and receipt["helper_exit_event"] is None:
            cleanup_events = queue.control(None, 1, max(0, end - time.monotonic()))
            receipt["fixture_cleanup_exit_pids"] = [event.ident for event in cleanup_events]
        receipt["worker_reaped"] = reaped
        for fd in (control_read, control_write, result_read, result_write):
            os.close(fd)
        queue.close()
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"worker-group-kill-real-helper-{uuid.uuid4().hex}.json").write_text(
            json.dumps(receipt, indent=2)
        )
    assert reaped and receipt["worker_returncode"] == -signal.SIGKILL
    assert receipt["helper_exit_event"] is not None, "helper survived worker-group KILL"
    assert receipt["helper_exit_event"]["pid"] == helper["pid"]
    assert receipt["helper_exit_event"]["fflags"] & select.KQ_NOTE_EXIT
    assert helper["pgid"] == helper["sid"] == worker


def test_review_raw_helper_resets_before_exec(tmp_path, monkeypatch):
    import atexit
    import gc

    import inspection

    marker = tmp_path / "preexec.json"
    evidence = (tmp_path / "evidence").open("wb")
    read_fd, write_fd = os.pipe()
    os.set_blocking(write_fd, False)
    old_handler = signal.signal(signal.SIGUSR1, lambda *_args: evidence.write(b"inherited"))
    old_wakeup = signal.set_wakeup_fd(write_fd)
    old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGUSR1})
    old_exec = os.execv
    parent = os.getpid()

    def inherited_exit():
        (tmp_path / "exit-hook").write_text("inherited")

    def checked_exec(file, argv):
        # These observations occur inside the actual raw helper, before exec.
        state = {
            "pid": os.getpid(),
            "ppid": os.getppid(),
            "pgid": os.getpgrp(),
            "sid": os.getsid(0),
            "usr1_default": signal.getsignal(signal.SIGUSR1) == signal.SIG_DFL,
            "int_default": signal.getsignal(signal.SIGINT) == signal.SIG_DFL,
            "pipe_default": signal.getsignal(signal.SIGPIPE) == signal.SIG_DFL,
            "mask": list(signal.pthread_sigmask(signal.SIG_BLOCK, set())),
            "wakeup": signal.set_wakeup_fd(-1),
            "gc_disabled": not gc.isenabled(),
            "owners": list(inspection._helper_owners),
            "stdio_inheritable": [os.get_inheritable(fd) for fd in (0, 1, 2)],
        }
        for name, fd in (("evidence_closed", evidence.fileno()), ("wakeup_closed", write_fd)):
            try:
                os.fstat(fd)
                state[name] = False
            except OSError as error:
                state[name] = error.errno == errno.EBADF
        marker.write_text(json.dumps(state))
        old_exec(file, argv)

    atexit.register(inherited_exit)
    monkeypatch.setattr(os, "execv", checked_exec)
    try:
        capture = inspection.capture_helper(
            (sys.executable, "-c", "print('raw-helper-ok')"), time.monotonic() + 1
        )
        retain_helper_control(
            tmp_path,
            "raw-helper-reset-real-preexec-observation",
            capture,
            json.loads(marker.read_text()) if marker.exists() else None,
        )
        assert marker.exists(), "helper did not pass through the explicit raw exec boundary"
        state = json.loads(marker.read_text())
        assert state["pid"] == capture.pid
        assert state["pgid"] == os.getpgrp()
        assert state["sid"] == os.getsid(0)
        assert state["ppid"] == parent
        assert all(
            state[key]
            for key in (
                "usr1_default",
                "int_default",
                "pipe_default",
                "gc_disabled",
                "evidence_closed",
                "wakeup_closed",
            )
        )
        assert state["mask"] == state["owners"] == []
        assert state["stdio_inheritable"] == [True, True, True]
        assert state["wakeup"] == -1
        assert capture.reaped and capture.returncode == 0 and capture.error is None
        assert capture.stdout == b"raw-helper-ok\n" and capture.stderr == b""
        assert not (tmp_path / "exit-hook").exists()
        assert evidence.tell() == 0
        assert signal.getsignal(signal.SIGUSR1) == old_handler or callable(
            signal.getsignal(signal.SIGUSR1)
        )
        assert signal.pthread_sigmask(signal.SIG_BLOCK, set()) == old_mask | {signal.SIGUSR1}
    finally:
        atexit.unregister(inherited_exit)
        signal.pthread_sigmask(signal.SIG_SETMASK, old_mask)
        signal.signal(signal.SIGUSR1, old_handler)
        signal.set_wakeup_fd(old_wakeup)
        evidence.close()
        os.close(read_fd)
        os.close(write_fd)


@pytest.mark.parametrize("failure", ["reset", "exec", "pending-signal"])
def test_review_raw_helper_child_failure_uses_exit_without_hooks(tmp_path, monkeypatch, failure):
    import atexit

    import inspection

    hook_marker = tmp_path / "hook"
    reset_marker = tmp_path / "reset.json"
    original = inspection.reset_after_fork

    def inherited_exit():
        hook_marker.write_text("exit")

    old_handler = signal.signal(signal.SIGUSR1, lambda *_args: hook_marker.write_text("handler"))

    def checked_reset(allowed, **kwargs):
        mask = signal.pthread_sigmask(signal.SIG_BLOCK, set())
        reset_marker.write_text(json.dumps({"pid": os.getpid(), "mask": sorted(mask)}))
        if failure == "reset":
            raise OSError(errno.EIO, "injected reset failure")
        if failure == "pending-signal":
            os.kill(os.getpid(), signal.SIGUSR1)
        original(allowed, **kwargs)

    monkeypatch.setattr(inspection, "reset_after_fork", checked_reset)
    atexit.register(inherited_exit)
    try:
        argv = (
            (str(tmp_path / "missing-executable"),)
            if failure == "exec"
            else (
                sys.executable,
                "-c",
                "raise SystemExit('unexpected exec')",
            )
        )
        capture = inspection.capture_helper(argv, time.monotonic() + 1)
        retain_helper_control(
            tmp_path,
            f"raw-helper-{failure}-real-child",
            capture,
            json.loads(reset_marker.read_text()) if reset_marker.exists() else None,
        )
        assert reset_marker.exists(), "helper bypassed reset"
        state = json.loads(reset_marker.read_text())
        assert state["pid"] == capture.pid
        assert {signal.SIGUSR1, signal.SIGTERM} <= set(state["mask"])
        assert capture.reaped
        assert capture.returncode == (-signal.SIGUSR1 if failure == "pending-signal" else 70)
        assert not hook_marker.exists()
        assert capture.stderr == b""
    finally:
        atexit.unregister(inherited_exit)
        signal.signal(signal.SIGUSR1, old_handler)


@pytest.mark.parametrize("failure", ["fork", "parent-mask-restore"])
def test_review_raw_helper_parent_failure_keeps_ownership(tmp_path, monkeypatch, failure):
    import inspection

    previous = signal.pthread_sigmask(signal.SIG_BLOCK, set())
    original_mask = signal.pthread_sigmask
    raised = False
    before = set(os.listdir("/dev/fd"))

    def refused_fork():
        raise OSError(errno.EAGAIN, "injected fork refusal; not kernel resource-pressure proof")

    def failed_restore(how, mask):
        nonlocal raised
        if how == signal.SIG_SETMASK and os.getpid() == parent and not raised:
            raised = True
            raise OSError(errno.EIO, "injected parent mask restore failure")
        return original_mask(how, mask)

    parent = os.getpid()
    if failure == "fork":
        monkeypatch.setattr(os, "fork", refused_fork)
    else:
        monkeypatch.setattr(signal, "pthread_sigmask", failed_restore)
    try:
        capture = inspection.capture_helper(("/bin/sleep", "10"), time.monotonic() + 0.5)
        retain_helper_control(tmp_path, f"raw-helper-{failure}-error-adapter", capture)
        assert capture.error
        assert capture.error_errno == (errno.EAGAIN if failure == "fork" else errno.EIO)
        if failure == "fork":
            assert capture.pid is None
            assert capture.created_monotonic is None
        else:
            assert raised
            assert capture.pid is not None
            assert capture.reaped and capture.returncode == -signal.SIGKILL
            assert capture.pid not in inspection._helper_owners
        assert set(os.listdir("/dev/fd")) == before
    finally:
        original_mask(signal.SIG_SETMASK, previous)


def test_review_raw_helper_accepts_closed_standard_descriptors(tmp_path):
    import subprocess

    marker = tmp_path / "closed-stdio.json"
    source = """
import json, os, sys, time
from dataclasses import asdict
from pathlib import Path
import inspection
for fd in (0, 1, 2):
    os.close(fd)
capture = inspection.capture_helper((sys.executable, '-c',
    'import os; assert os.read(0,1)==b""; os.write(1,b"out"); os.write(2,b"err")'),
    time.monotonic() + 1)
result = asdict(capture)
result.update(stdout=capture.stdout.hex(), stderr=capture.stderr.hex())
Path(sys.argv[1]).write_text(json.dumps(result))
os._exit(0)
"""
    completed = subprocess.run(
        (sys.executable, "-c", source, str(marker)), cwd=HERE, timeout=3, check=False
    )
    assert completed.returncode == 0
    result = json.loads(marker.read_text())
    assert result["reaped"] and result["returncode"] == 0 and result["error"] is None
    assert result["stdout"] == b"out".hex() and result["stderr"] == b"err".hex()
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    (root / f"helper-raw-closed-stdio-{uuid.uuid4().hex}.json").write_text(
        json.dumps(result, indent=2)
    )
