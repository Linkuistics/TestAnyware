"""Real-process controls for the interim per-query creation boundary."""

import json
import os
import signal
import sys
import time
from pathlib import Path

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
