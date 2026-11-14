"""Real-process controls for the interim per-query creation boundary."""

import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

import inspection


def retain(tmp_path, name, receipt):
    from dataclasses import asdict

    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    data = asdict(receipt)
    data.update(stdout=receipt.stdout.hex(), stderr=receipt.stderr.hex())
    (root / f"worker-{name}-{time.time_ns()}.json").write_text(json.dumps(data, indent=2))


def test_review_worker_bounds_real_delayed_helper_creation(tmp_path, monkeypatch):
    from worker_capture import capture_helper

    marker = tmp_path / "creation.json"

    def delayed(*_args, **_kwargs):
        marker.write_text(json.dumps({"pid": os.getpid(), "ppid": os.getppid()}))
        time.sleep(10)
        raise AssertionError("delayed creation escaped worker termination")

    monkeypatch.setattr(inspection, "_HelperProcess", delayed)
    started = time.monotonic()
    receipt = capture_helper(("/bin/ps", "-p", str(os.getpid())), started + 0.5)
    retain(tmp_path, "delayed-creation", receipt)
    assert marker.exists()
    worker = receipt.worker
    assert worker is not None
    assert json.loads(marker.read_text())["pid"] == worker["pid"]
    assert json.loads(marker.read_text())["ppid"] == os.getpid()
    assert receipt.error is not None
    assert receipt.finished_monotonic <= receipt.deadline_monotonic
    assert receipt.finished_monotonic - started < 0.5
    assert worker["reaped"] is True
    assert worker["unknown_helper_creation"] is True
    assert worker["signal_scope"] == "group"
    try:
        os.waitpid(worker["pid"], os.WNOHANG)
    except ChildProcessError:
        pass
    else:
        raise AssertionError("worker was not reaped")


def test_review_worker_resets_handlers_masks_wakeup_and_evidence_fd(tmp_path, monkeypatch):
    from worker_capture import capture_helper

    original = inspection._HelperProcess
    marker = tmp_path / "child.json"
    evidence = (tmp_path / "parent-evidence").open("wb")
    read_fd, write_fd = os.pipe()
    os.set_blocking(write_fd, False)
    old_handler = signal.signal(signal.SIGTERM, lambda *_args: evidence.write(b"inherited"))
    old_wakeup = signal.set_wakeup_fd(write_fd)
    old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, {signal.SIGUSR1})

    def checked(*args, **kwargs):
        state = {
            "pid": os.getpid(),
            "pgid": os.getpgrp(),
            "ppid": os.getppid(),
            "term": signal.getsignal(signal.SIGTERM) == signal.SIG_DFL,
            "int": signal.getsignal(signal.SIGINT) == signal.SIG_DFL,
            "pipe": signal.getsignal(signal.SIGPIPE) == signal.SIG_DFL,
            "mask": list(signal.pthread_sigmask(signal.SIG_BLOCK, set())),
            "wakeup": signal.set_wakeup_fd(-1),
        }
        # Inspect before opening a file can reuse one of the closed descriptors.
        for name, fd in (("evidence", evidence.fileno()), ("wakeup_fd", write_fd)):
            try:
                os.fstat(fd)
                state[name] = "open"
            except OSError:
                state[name] = "closed"
        marker.write_text(json.dumps(state))
        return original(*args, **kwargs)

    monkeypatch.setattr(inspection, "_HelperProcess", checked)
    try:
        receipt = capture_helper((sys.executable, "-c", "print('worker-ok')"), time.monotonic() + 1)
        retain(tmp_path, "reset", receipt)
        state = json.loads(marker.read_text())
        assert receipt.error is None
        assert receipt.stdout == b"worker-ok\n"
        assert receipt.reaped is True
        assert receipt.worker["reaped"] is True
        timing = receipt.worker["published_times"]
        assert (
            receipt.worker["dispatch_monotonic"]
            <= timing["started_monotonic"]
            <= timing["created_monotonic"]
            <= timing["reaped_monotonic"]
            <= timing["finished_monotonic"]
            <= receipt.worker["receipt_monotonic"]
            <= timing["deadline_monotonic"]
            == receipt.worker["query_deadline_monotonic"]
        )
        assert state["pid"] == state["pgid"] == receipt.worker["pid"]
        assert state["ppid"] == os.getpid()
        assert all(state[key] for key in ("term", "int", "pipe"))
        assert state["mask"] == []
        assert state["wakeup"] == -1
        assert state["evidence"] == state["wakeup_fd"] == "closed"
        assert signal.getsignal(signal.SIGTERM) != signal.SIG_DFL
        assert signal.SIGUSR1 in signal.pthread_sigmask(signal.SIG_BLOCK, set())
        evidence.flush()
        assert (tmp_path / "parent-evidence").read_bytes() == b""
    finally:
        signal.pthread_sigmask(signal.SIG_SETMASK, old_mask)
        signal.set_wakeup_fd(old_wakeup)
        signal.signal(signal.SIGTERM, old_handler)
        os.close(read_fd)
        os.close(write_fd)
        evidence.close()


def test_review_worker_startup_delay_uses_pid_only_without_group_authority(tmp_path, monkeypatch):
    from worker_capture import capture_helper

    marker = tmp_path / "before-ready"

    def delayed(_allowed):
        marker.write_text(str(os.getpid()))
        time.sleep(10)

    monkeypatch.setattr(inspection, "reset_after_fork", delayed)
    receipt = capture_helper(("/bin/ps",), time.monotonic() + 0.4)
    retain(tmp_path, "startup-delay", receipt)
    assert marker.exists()
    assert receipt.worker["pid"] == int(marker.read_text())
    assert receipt.worker["ready"] is False
    assert receipt.worker["signal_scope"] == "pid"
    assert receipt.worker["unknown_helper_creation"] is True
    assert receipt.worker["reaped"] is True
    assert receipt.error is not None
    assert receipt.finished_monotonic <= receipt.deadline_monotonic


def test_review_worker_rejects_fork_before_creation(tmp_path, monkeypatch):
    import worker_capture

    admission = inspection.fork_admission()
    admission.update(ok=False, reason="native_threads", native_threads=2)
    monkeypatch.setattr(inspection, "fork_admission", lambda: admission)

    def forbidden():
        raise AssertionError("fork despite failed admission")

    monkeypatch.setattr(os, "fork", forbidden)
    receipt = worker_capture.capture_helper(("/bin/ps",), time.monotonic() + 1)
    retain(tmp_path, "admission-rejected", receipt)
    assert receipt.worker is None
    assert receipt.error is not None
    assert receipt.admission == admission
    assert not worker_capture._owners


def test_review_worker_retains_real_fork_errno_without_child(tmp_path, monkeypatch):
    import errno

    from worker_capture import capture_helper

    def fail():
        raise OSError(errno.EAGAIN, "injected fork error; no OS child created")

    monkeypatch.setattr(os, "fork", fail)
    old_mask = signal.pthread_sigmask(signal.SIG_BLOCK, set())
    receipt = capture_helper(("/bin/ps",), time.monotonic() + 1)
    retain(tmp_path, "fork-errno-adapter", receipt)
    assert receipt.worker is None
    assert receipt.error_errno == errno.EAGAIN
    assert signal.pthread_sigmask(signal.SIG_BLOCK, set()) == old_mask


def test_review_worker_does_not_run_inherited_exit_hook(tmp_path):
    import atexit

    from worker_capture import capture_helper

    marker = tmp_path / "inherited-hook"

    def hook():
        marker.write_text(str(os.getpid()))

    atexit.register(hook)
    try:
        receipt = capture_helper((sys.executable, "-c", "print('ok')"), time.monotonic() + 1)
        retain(tmp_path, "exit-hook", receipt)
        assert receipt.error is None
        assert receipt.worker["reaped"] is True
        assert not marker.exists()
    finally:
        atexit.unregister(hook)


def test_review_worker_channel_close_error_keeps_receipt_and_other_closes(tmp_path, monkeypatch):
    import errno

    from worker_capture import capture_helper

    parent = os.getpid()
    pipe = os.pipe
    close = os.close
    reads = []
    closed = []

    def tracked_pipe():
        pair = pipe()
        reads.append(pair[0])
        return pair

    def fail_after_close(fd):
        close(fd)
        if os.getpid() == parent:
            closed.append(fd)
            if reads and fd == reads[0]:
                raise OSError(errno.EIO, "injected channel close error after real close")

    monkeypatch.setattr(os, "pipe", tracked_pipe)
    monkeypatch.setattr(os, "close", fail_after_close)
    receipt = capture_helper((sys.executable, "-c", "print('ok')"), time.monotonic() + 1)
    retain(tmp_path, "channel-close-error-adapter", receipt)
    assert receipt.close_errors
    assert receipt.error is not None
    assert receipt.worker["reaped"] is True
    assert all(fd in closed for fd in reads)


@pytest.mark.parametrize("closed", [(0,), (1,), (2,), (0, 1), (0, 2), (1, 2), (0, 1, 2)])
def test_review_worker_captures_with_closed_standard_streams(tmp_path, closed):
    # Real subprocess: closing pytest's own streams would invalidate the instrument.
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    target = root / f"worker-closed-stdio-{''.join(map(str, closed))}-{time.time_ns()}.json"
    code = """
import json, os, sys, time
from dataclasses import asdict
from pathlib import Path
from worker_capture import capture_helper
for fd in map(int, sys.argv[2:]):
    os.close(fd)
receipt = capture_helper((sys.executable, '-c', "print('stdio-ok')"), time.monotonic() + 1)
data = asdict(receipt)
data.update(stdout=receipt.stdout.hex(), stderr=receipt.stderr.hex(), controller_pid=os.getpid())
Path(sys.argv[1]).write_text(json.dumps(data, indent=2))
"""
    check = subprocess.run(
        [sys.executable, "-c", code, str(target), *map(str, closed)],
        capture_output=True,
        timeout=3,
    )
    assert check.returncode == 0, check.stderr
    receipt = json.loads(target.read_text())
    assert receipt["error"] is None
    assert bytes.fromhex(receipt["stdout"]) == b"stdio-ok\n"
    assert receipt["reaped"] and receipt["worker"]["reaped"]
    assert receipt["finished_monotonic"] <= receipt["deadline_monotonic"]


@pytest.mark.parametrize(
    "fault",
    [
        "nested-result",
        "nested-readiness",
        "malformed",
        "duplicate-key",
        "bad-token",
        "bad-timestamp",
        "bad-field-type",
        "oversized-frame",
        "oversized-channel",
        "partial-frame",
        "exit-before-intent",
        "duplicate-readiness",
        "predispatch-start",
        "reversed-query",
        "future-finish",
        "null-start",
        "null-finish",
        "null-deadline",
        "null-cutoff",
        "early-cutoff",
        "late-cutoff",
        "early-created",
        "future-created",
        "kill-before-create",
        "reap-before-kill",
        "kill-without-create",
        "nan-finish",
        "infinite-finish",
    ],
)
def test_review_worker_rejects_hostile_channel(tmp_path, monkeypatch, fault):
    # Protocol adapter replaces only child publication. Fork, reset, pipes, parent
    # validation, deadline, shutdown and exact waitpid are real; no helper is created.
    import struct
    from dataclasses import asdict

    import worker_capture

    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    publication = root / f"worker-publication-{fault}-{time.time_ns()}.json"
    owners = []
    reap = worker_capture._reap

    def track_reap(owner):
        if not owners:
            owners.append(owner)
        reap(owner)

    def publish(control, result, token, argv, deadline):
        try:
            inspection.reset_after_fork({control, result})
            identity = {
                "token": token,
                "pid": os.getpid(),
                "pgid": os.getpgrp(),
                "sid": os.getsid(0),
            }
            ready = json.dumps(identity).encode()
            if fault == "nested-readiness":
                ready = b"[" * 1500 + b"]" * 1500
            elif fault == "duplicate-readiness":
                ready = b'{"token":"wrong",' + ready[1:]
            worker_capture._write(control, ready + b"\n", deadline)
            if fault == "exit-before-intent":
                os._exit(73)
            worker_capture._write(control, b"intent\n", deadline)
            now = time.monotonic()
            body = asdict(
                inspection.HelperCapture(
                    argv=argv,
                    started_monotonic=now,
                    deadline_monotonic=deadline,
                    capture_deadline_monotonic=deadline,
                    finished_monotonic=now,
                    admission={"ok": True},
                )
            )
            body.update(stdout="", stderr="", token=token)
            if fault == "bad-token":
                body["token"] = "wrong"
            elif fault == "bad-timestamp":
                body["started_monotonic"] = deadline + 1
            elif fault == "bad-field-type":
                body["stdout"] = []
            changes = {
                "predispatch-start": {"started_monotonic": 0},
                "reversed-query": {"finished_monotonic": now - 0.001},
                "future-finish": {"finished_monotonic": deadline - 0.001},
                "null-start": {"started_monotonic": None},
                "null-finish": {"finished_monotonic": None},
                "null-deadline": {"deadline_monotonic": None},
                "null-cutoff": {"capture_deadline_monotonic": None},
                "early-cutoff": {"capture_deadline_monotonic": now - 0.001},
                "late-cutoff": {"capture_deadline_monotonic": deadline + 0.001},
                "early-created": {"created_monotonic": now - 0.001},
                "future-created": {"created_monotonic": deadline - 0.001},
                "kill-before-create": {
                    "started_monotonic": now - 0.002,
                    "created_monotonic": now,
                    "kill_monotonic": now - 0.001,
                },
                "reap-before-kill": {
                    "started_monotonic": now - 0.003,
                    "created_monotonic": now - 0.002,
                    "kill_monotonic": now,
                    "reaped_monotonic": now - 0.001,
                },
                "kill-without-create": {"kill_monotonic": now},
                "nan-finish": {"finished_monotonic": float("nan")},
                "infinite-finish": {"finished_monotonic": float("inf")},
            }
            body.update(changes.get(fault, {}))
            payload = json.dumps(body).encode()
            if fault == "nested-result":
                payload = b"[" * 200000 + b"]" * 200000
            elif fault == "malformed":
                payload = b"{"
            elif fault == "duplicate-key":
                payload = b'{"stdout":"",' + payload[1:]
            if fault == "oversized-frame":
                frame = struct.pack("!I", 1024 * 1024 + 1)
            elif fault == "oversized-channel":
                frame = struct.pack("!I", 1024 * 1024) + b"x" * (1024 * 1024 + 1)
            else:
                frame = struct.pack("!I", len(payload)) + payload
            if fault == "partial-frame":
                frame = frame[:-1]
            publication.write_text(
                json.dumps({"fault": fault, "payload_hex": payload.hex(), "frame_hex": frame.hex()})
            )
            worker_capture._write(result, frame, deadline)
            os._exit(0)
        except BaseException:
            os._exit(74)

    monkeypatch.setattr(worker_capture, "_child", publish)
    monkeypatch.setattr(worker_capture, "_reap", track_reap)
    receipt = None
    try:
        receipt = worker_capture.capture_helper(("/bin/ps",), time.monotonic() + 1)
    finally:
        # Keep ownership evidence even if the pre-fix decoder escapes the API.
        if receipt is not None:
            retain(tmp_path, f"hostile-{fault}-protocol-adapter", receipt)
        else:
            root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
            root.mkdir(parents=True, exist_ok=True)
            (root / f"escaped-{fault}-{time.time_ns()}.json").write_text(
                json.dumps({"worker": asdict(owners[0]), "fault": fault}, indent=2)
            )
    assert receipt.error is not None
    assert receipt.worker["unknown_helper_creation"] is True
    assert receipt.worker["reaped"] is True
    assert receipt.worker["dispatch_monotonic"] <= receipt.finished_monotonic
    assert receipt.worker["published_times"] is None
    if publication.exists():
        assert receipt.worker["receipt_monotonic"] is not None or fault in {
            "oversized-channel",
            "duplicate-readiness",
            "nested-readiness",
        }
    assert receipt.residual_remedy
    assert receipt.finished_monotonic <= receipt.deadline_monotonic
    assert receipt.stdout == receipt.stderr == b""


def test_review_group_snapshot_uses_real_worker(tmp_path):
    import controller

    # This unreaped session leader pins the queried group throughout observation.
    process = subprocess.Popen(["/bin/sleep", "10"], start_new_session=True)
    try:
        observation = controller._group_snapshot(process.pid, time.monotonic() + 1)
        helper = observation["helper"]
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
        root.mkdir(parents=True, exist_ok=True)
        data = {
            **observation,
            "stdout": observation["stdout"].hex(),
            "stderr": observation["stderr"].hex(),
            "fixture_pid": process.pid,
        }
        if helper is not None:
            data["helper"] = {
                **helper,
                "stdout": helper["stdout"].hex(),
                "stderr": helper["stderr"].hex(),
            }
        (root / f"worker-group-snapshot-{time.time_ns()}.json").write_text(
            json.dumps(data, indent=2)
        )
        assert observation["error"] is None
        assert observation["state"] == "live"
        assert any(item["pid"] == process.pid for item in observation["members"])
        assert helper["reaped"] is True
        assert helper["worker"]["reaped"] is True
    finally:
        os.killpg(process.pid, signal.SIGKILL)
        process.wait(timeout=1)


def test_review_worker_retains_timing_for_real_killed_helper(tmp_path):
    from worker_capture import capture_helper

    receipt = capture_helper(("/bin/sleep", "10"), time.monotonic() + 0.5)
    retain(tmp_path, "real-helper-timeout", receipt)
    worker = receipt.worker
    assert receipt.error is not None
    assert receipt.reaped and worker["reaped"]
    assert not worker["unknown_helper_creation"]
    timing = worker["published_times"]
    assert (
        worker["dispatch_monotonic"]
        <= timing["started_monotonic"]
        <= timing["created_monotonic"]
        <= timing["capture_deadline_monotonic"]
        <= timing["kill_monotonic"]
        <= timing["reaped_monotonic"]
        <= timing["finished_monotonic"]
        <= worker["receipt_monotonic"]
        <= worker["query_deadline_monotonic"]
        < worker["shutdown_deadline_monotonic"]
    )
