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


@pytest.mark.parametrize(
    ("mode", "reason"),
    [
        ("admission-missing", "invalid worker admission fields"),
        ("admission-extra", "invalid worker admission fields"),
        ("admission-bool-count", "invalid worker admission values"),
        ("admission-zero-count", "invalid worker admission values"),
        ("admission-bool-ok", "invalid worker admission values"),
        ("admission-bad-signal", "invalid worker admission values"),
        ("admission-bad-native-signal", "invalid worker admission values"),
        ("admission-zero-errno", "invalid worker admission values"),
        ("admission-bool-errno", "invalid worker admission values"),
        ("admission-ready-thread", "inconsistent worker admission"),
        ("admission-ready-signal", "inconsistent worker admission"),
        ("admission-ready-error", "inconsistent worker admission"),
        ("admission-ready-errno", "inconsistent worker admission"),
        ("admission-missing-native", "inconsistent worker admission"),
        ("admission-wrong-priority", "inconsistent worker admission"),
        ("admission-wrong-ok", "inconsistent worker admission"),
        ("admission-observation-no-error", "inconsistent worker admission"),
        ("admission-observation-complete", "inconsistent worker admission"),
        ("admission-refusal-created", "worker helper created after admission refusal"),
        ("zero-error-errno", "invalid worker errno"),
        ("negative-kill-errno", "invalid worker errno"),
        ("zero-reap-errno", "invalid worker errno"),
        ("empty-error", "invalid worker error"),
        ("empty-remedy", "invalid worker error"),
        ("empty-close-error", "invalid worker close errors"),
        ("errno-without-error", "worker failure without error"),
        ("kill-without-error", "worker failure without error"),
        ("close-without-error", "worker failure without error"),
        ("unreaped-without-error", "worker failure without error"),
        ("echild-retained-owner", "worker ECHILD/ownership mismatch"),
        ("lost-owner-without-echild", "worker ECHILD/ownership mismatch"),
        ("lost-owner-other-errno", "worker ECHILD/ownership mismatch"),
        ("unreaped-without-remedy", "worker residual ownership mismatch"),
        ("reaped-with-remedy", "worker residual ownership mismatch"),
        ("lost-owner-with-remedy", "worker residual ownership mismatch"),
        ("wait-without-child", "worker helper failure without creation"),
        ("output-without-child", "worker helper failure without creation"),
        ("close-without-child", "worker helper failure without creation"),
        ("success", None),
        ("failed-read-reaped", None),
        ("failed-kill-reaped", None),
        ("failed-close-reaped", None),
        ("retained-owner", None),
        ("lost-owner", None),
        ("python-refusal", None),
        ("native-refusal", None),
        ("signal-refusal", None),
        ("native-signal-refusal", None),
        ("observation-refusal", None),
        ("observation-refusal-no-errno", None),
    ],
)
def test_worker_failure_consistency(tmp_path, monkeypatch, mode, reason):
    import errno
    from dataclasses import asdict

    import worker_capture

    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    witness = root / f"failure-publication-{mode}-{time.time_ns()}.json"

    def synthetic_capture(argv, deadline):
        # Only the helper receipt is synthetic; no helper is created. Worker
        # admission/fork/reset/framing/channels/decoding/shutdown remain real.
        now = time.monotonic()
        body = inspection.HelperCapture(
            pid=os.getpid(),
            argv=argv,
            admission=inspection.fork_admission(),
            started_monotonic=now,
            created_monotonic=now,
            capture_deadline_monotonic=deadline - min(0.05, (deadline - now) / 4),
            deadline_monotonic=deadline,
            finished_monotonic=now,
            reaped=True,
            reaped_monotonic=now,
            returncode=0,
            stdout=b"synthetic output",
        )
        raw = asdict(body)
        admission = raw["admission"]
        if mode == "admission-missing":
            del admission["native_threads"]
        if mode == "admission-extra":
            admission["unexpected"] = True
        changes = {
            "admission-bool-count": {"python_threads": True},
            "admission-zero-count": {"native_threads": 0},
            "admission-bool-ok": {"ok": 1},
            "admission-bad-signal": {"sigchld": "unknown"},
            "admission-bad-native-signal": {"native_sigchld": "unknown"},
            "admission-zero-errno": {"errno": 0},
            "admission-bool-errno": {"errno": True},
            "admission-ready-thread": {"native_threads": 2},
            "admission-ready-signal": {"sigchld": "handler"},
            "admission-ready-error": {"error": "unexpected observation error"},
            "admission-ready-errno": {"errno": errno.EIO},
            "admission-missing-native": {"native_threads": None},
            "admission-wrong-priority": {
                "ok": False,
                "reason": "native_threads",
                "python_threads": 2,
                "native_threads": 2,
            },
            "admission-wrong-ok": {"ok": False},
            "admission-observation-no-error": {
                "ok": False,
                "reason": "native_observation_error",
                "native_sigchld": None,
            },
            "admission-observation-complete": {
                "ok": False,
                "reason": "native_observation_error",
                "error": "synthetic observation failure",
            },
            "admission-refusal-created": {
                "ok": False,
                "reason": "python_threads",
                "python_threads": 2,
            },
            "python-refusal": {"ok": False, "reason": "python_threads", "python_threads": 2},
            "native-refusal": {"ok": False, "reason": "native_threads", "native_threads": 2},
            "signal-refusal": {"ok": False, "reason": "sigchld", "sigchld": "SIG_IGN"},
            "native-signal-refusal": {
                "ok": False,
                "reason": "sigchld",
                "native_sigchld": "handler",
            },
            "observation-refusal": {
                "ok": False,
                "reason": "native_observation_error",
                "native_threads": None,
                "native_sigchld": None,
                "error": "synthetic observation failure",
                "errno": errno.EIO,
            },
            "observation-refusal-no-errno": {
                "ok": False,
                "reason": "native_observation_error",
                "native_sigchld": None,
                "error": "synthetic observation failure",
            },
        }
        admission.update(changes.get(mode, {}))
        raw_changes = {
            "zero-error-errno": {"error_errno": 0},
            "negative-kill-errno": {"kill_errno": -1, "kill_monotonic": now},
            "zero-reap-errno": {"reap_errno": 0},
            "empty-error": {"error": ""},
            "empty-remedy": {"residual_remedy": ""},
            "empty-close-error": {"close_errors": [""]},
            "errno-without-error": {"error_errno": errno.EIO},
            "kill-without-error": {"kill_monotonic": now},
            "close-without-error": {"close_errors": ["synthetic close failure"]},
            "failed-read-reaped": {"error_errno": errno.EIO, "error": "synthetic read failure"},
            "failed-kill-reaped": {
                "kill_monotonic": now,
                "kill_errno": errno.ESRCH,
                "error": "synthetic kill failure",
            },
            "failed-close-reaped": {
                "close_errors": ["synthetic close failure"],
                "error": "synthetic close failure",
            },
            "reaped-with-remedy": {"residual_remedy": "synthetic remedy", "error": "failure"},
        }
        raw.update(raw_changes.get(mode, {}))
        if mode in {
            "unreaped-without-error",
            "echild-retained-owner",
            "lost-owner-without-echild",
            "lost-owner-other-errno",
            "unreaped-without-remedy",
            "lost-owner-with-remedy",
            "retained-owner",
            "lost-owner",
        }:
            raw.update(
                reaped=False,
                reaped_monotonic=None,
                returncode=None,
                error="synthetic wait failure",
                residual_remedy="synthetic remedy",
            )
            if mode == "unreaped-without-error":
                raw["error"] = None
            if mode in {"echild-retained-owner", "lost-owner-with-remedy", "lost-owner"}:
                raw["reap_errno"] = errno.ECHILD
            if mode.startswith("lost-owner"):
                raw["ownership_lost"] = True
            if mode == "lost-owner-other-errno" or mode == "retained-owner":
                raw["reap_errno"] = errno.EIO
            if mode in {"unreaped-without-remedy", "lost-owner"}:
                raw["residual_remedy"] = None
        if (
            mode.endswith("-refusal")
            or mode == "observation-refusal-no-errno"
            or mode.endswith("without-child")
        ):
            raw.update(
                pid=None,
                created_monotonic=None,
                reaped=False,
                reaped_monotonic=None,
                returncode=None,
                stdout=b"",
                error="synthetic precreation failure",
            )
            if mode == "wait-without-child":
                raw["reap_errno"] = errno.EIO
            if mode == "output-without-child":
                raw["stdout"] = b"impossible output"
            if mode == "close-without-child":
                raw["close_errors"] = ["impossible close failure"]
        body = inspection.HelperCapture(**raw)
        encoded = {**raw, "stdout": body.stdout.hex(), "stderr": body.stderr.hex()}
        witness.write_text(json.dumps({"mode": mode, "synthetic_helper": True, "body": encoded}))
        return body

    monkeypatch.setattr(inspection, "capture_helper", synthetic_capture)
    receipt = worker_capture.capture_helper(
        ("synthetic-failure-publication",), time.monotonic() + 1
    )
    retain(tmp_path, f"failure-{mode}", receipt)
    worker = receipt.worker
    assert worker["reaped"] and not worker["ownership_lost"]
    assert receipt.finished_monotonic <= receipt.deadline_monotonic
    body = json.loads(witness.read_text())["body"]
    assert worker["dispatch_monotonic"] <= body["started_monotonic"]
    assert body["finished_monotonic"] <= worker["receipt_monotonic"] <= body["deadline_monotonic"]
    assert worker["unknown_helper_creation"] == (reason is not None)
    if reason:
        assert receipt.error == f"worker capture failed: {reason}"
        assert receipt.stdout == receipt.stderr == b""
        assert receipt.residual_remedy and worker["published_times"] is None
    else:
        for key in (
            "pid",
            "admission",
            "returncode",
            "reaped",
            "ownership_lost",
            "reap_errno",
            "kill_errno",
            "error_errno",
            "error",
            "residual_remedy",
            "close_errors",
        ):
            assert getattr(receipt, key) == body[key]
        assert receipt.stdout.hex() == body["stdout"]
        assert worker["published_times"] is not None


def test_review_worker_preserves_real_helper_admission_refusal(tmp_path, monkeypatch):
    import threading

    import worker_capture

    original = inspection.capture_helper

    def threaded_capture(argv, deadline):
        stop = threading.Event()
        thread = threading.Thread(target=stop.wait)
        thread.start()
        try:
            return original(argv, deadline)
        finally:
            stop.set()
            thread.join(timeout=0.1)

    monkeypatch.setattr(inspection, "capture_helper", threaded_capture)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 1)
    retain(tmp_path, "real-helper-admission-refusal", receipt)
    assert receipt.worker["reaped"] and not receipt.worker["unknown_helper_creation"]
    assert receipt.error == "helper fork admission rejected: python_threads"
    assert receipt.admission["python_threads"] == receipt.admission["native_threads"] == 2
    assert not receipt.admission["ok"]
    assert receipt.pid is None and receipt.created_monotonic is None
    assert receipt.stdout == receipt.stderr == b"" and not receipt.reaped


@pytest.mark.parametrize(
    ("mode", "reason"),
    [
        ("status-without-reap", "worker status/reap mismatch"),
        ("reap-without-status", "worker status/reap mismatch"),
        ("reaped-with-error", "worker reaped helper has failed ownership"),
        ("reaped-with-lost-owner", "worker reaped helper has failed ownership"),
        ("success", None),
        ("nonzero", None),
        ("unreaped-error", None),
        ("lost-owner-error", None),
    ],
)
def test_worker_terminal_consistency(tmp_path, monkeypatch, mode, reason):
    import errno
    from dataclasses import asdict

    import worker_capture

    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    witness = root / f"terminal-publication-{mode}-{time.time_ns()}.json"

    def synthetic_capture(argv, deadline):
        # Only the helper receipt is synthetic. The ordinary worker child builds
        # its frame, publishes on real channels, and exits through os._exit.
        now = time.monotonic()
        body = inspection.HelperCapture(
            pid=os.getpid(),
            argv=argv,
            admission=inspection.fork_admission(),
            started_monotonic=now,
            created_monotonic=now,
            capture_deadline_monotonic=deadline - min(0.05, (deadline - now) / 4),
            deadline_monotonic=deadline,
            finished_monotonic=now,
            reaped=True,
            reaped_monotonic=now,
            returncode=0,
            stdout=b"synthetic output",
        )
        if mode in {"status-without-reap", "unreaped-error", "lost-owner-error"}:
            body.reaped = False
            body.reaped_monotonic = None
        if mode in {"reap-without-status", "unreaped-error", "lost-owner-error"}:
            body.returncode = None
        if mode in {"reaped-with-error", "unreaped-error", "lost-owner-error"}:
            body.reap_errno = errno.EIO if mode == "unreaped-error" else errno.ECHILD
            body.error = "synthetic wait failure"
        if mode == "unreaped-error":
            body.residual_remedy = "synthetic retained owner requires reconciliation"
        if mode in {"reaped-with-lost-owner", "lost-owner-error"}:
            body.ownership_lost = True
            body.error = "synthetic lost ownership"
        if mode == "nonzero":
            body.returncode = 1
        raw = asdict(body)
        raw.update(stdout=body.stdout.hex(), stderr=body.stderr.hex())
        witness.write_text(json.dumps({"mode": mode, "synthetic_helper": True, "body": raw}))
        return body

    monkeypatch.setattr(inspection, "capture_helper", synthetic_capture)
    started = time.monotonic()
    receipt = worker_capture.capture_helper(("synthetic-terminal-publication",), started + 1)
    retain(tmp_path, f"terminal-{mode}", receipt)
    worker = receipt.worker
    assert worker["reaped"] and not worker["ownership_lost"]
    assert worker["unknown_helper_creation"] == (reason is not None)
    assert receipt.finished_monotonic <= receipt.deadline_monotonic == started + 1
    body = json.loads(witness.read_text())["body"]
    assert worker["dispatch_monotonic"] <= body["started_monotonic"]
    assert body["finished_monotonic"] <= worker["receipt_monotonic"]
    assert worker["receipt_monotonic"] <= body["deadline_monotonic"]
    if reason is not None:
        assert receipt.error == f"worker capture failed: {reason}"
        assert receipt.stdout == receipt.stderr == b""
        assert receipt.residual_remedy and worker["published_times"] is None
    else:
        assert receipt.stdout == b"synthetic output"
        assert receipt.returncode == body["returncode"]
        assert receipt.reaped == body["reaped"]
        assert receipt.ownership_lost == body["ownership_lost"]
        assert receipt.reap_errno == body["reap_errno"]
        assert receipt.error == body["error"]


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

    monkeypatch.setattr(inspection, "_spawn_helper", delayed)
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

    original = inspection._spawn_helper
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

    monkeypatch.setattr(inspection, "_spawn_helper", checked)
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
        "reaped-without-stamp",
        "stamp-without-reaped",
        "zero-pid",
        "negative-pid",
        "negative-group-pid",
        "boolean-pid",
        "string-pid",
        "pid-without-created",
        "created-without-pid",
        "kill-errno-without-kill",
        "wrong-cutoff",
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
                    capture_deadline_monotonic=deadline - min(0.05, (deadline - now) / 4),
                    finished_monotonic=now,
                    admission=inspection.fork_admission(),
                )
            )
            body.update(stdout="", stderr="", token=token)
            if fault == "bad-token":
                body["token"] = "wrong"
            elif fault == "bad-timestamp":
                body["started_monotonic"] = deadline + 1
            elif fault == "bad-field-type":
                body["stdout"] = []
            # Both witnesses are sampled after dispatch. Only helper event order
            # changes; no guessed child scheduling delay shifts the query start.
            finished = time.monotonic()
            body["finished_monotonic"] = finished
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
                "early-created": {"pid": os.getpid(), "created_monotonic": now - 0.001},
                "future-created": {"pid": os.getpid(), "created_monotonic": deadline - 0.001},
                "kill-before-create": {
                    "pid": os.getpid(),
                    "created_monotonic": finished,
                    "kill_monotonic": now,
                },
                "reap-before-kill": {
                    "pid": os.getpid(),
                    "created_monotonic": now,
                    "kill_monotonic": finished,
                    "reaped": True,
                    "reaped_monotonic": now,
                },
                "kill-without-create": {"kill_monotonic": now},
                "reaped-without-stamp": {"reaped": True},
                "stamp-without-reaped": {
                    "pid": os.getpid(),
                    "created_monotonic": now,
                    "reaped_monotonic": now,
                },
                # Matching creation stamps isolate identity validation from the
                # existing PID/timestamp coupling and query-order checks.
                "zero-pid": {"pid": 0, "created_monotonic": now},
                "negative-pid": {"pid": -1, "created_monotonic": now},
                "negative-group-pid": {"pid": -123, "created_monotonic": now},
                "boolean-pid": {"pid": True, "created_monotonic": now},
                "string-pid": {"pid": "123", "created_monotonic": now},
                "pid-without-created": {"pid": os.getpid()},
                "created-without-pid": {"created_monotonic": now},
                "kill-errno-without-kill": {"kill_errno": 1},
                "wrong-cutoff": {"capture_deadline_monotonic": deadline},
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
    expected_errors = {
        "bad-timestamp": "invalid worker timestamp",
        "predispatch-start": "worker query interval outside dispatch/receipt allowance",
        "reversed-query": "worker query interval outside dispatch/receipt allowance",
        "future-finish": "worker query interval outside dispatch/receipt allowance",
        "null-start": "invalid worker timestamp",
        "null-finish": "invalid worker timestamp",
        "null-deadline": "invalid worker timestamp",
        "null-cutoff": "invalid worker timestamp",
        "early-cutoff": "worker capture cutoff outside query allowance",
        "late-cutoff": "invalid worker timestamp",
        "early-created": "worker helper timestamps out of order",
        "future-created": "worker helper timestamps out of order",
        "kill-before-create": "worker helper timestamps out of order",
        "reap-before-kill": "worker helper timestamps out of order",
        "kill-without-create": "worker helper event without creation timestamp",
        "reaped-without-stamp": "worker reap state/timestamp mismatch",
        "stamp-without-reaped": "worker reap state/timestamp mismatch",
        "zero-pid": "invalid worker helper pid",
        "negative-pid": "invalid worker helper pid",
        "negative-group-pid": "invalid worker helper pid",
        "boolean-pid": "invalid worker integer",
        "string-pid": "invalid worker integer",
        "pid-without-created": "worker pid/creation timestamp mismatch",
        "created-without-pid": "worker pid/creation timestamp mismatch",
        "kill-errno-without-kill": "worker kill errno without kill timestamp",
        "wrong-cutoff": "worker capture cutoff mismatch",
        "nan-finish": "invalid worker timestamp",
        "infinite-finish": "invalid worker timestamp",
    }
    if fault in expected_errors:
        assert receipt.error == f"worker capture failed: {expected_errors[fault]}"
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


@pytest.mark.parametrize("mode", ["early-exit", "spawn-failure"])
def test_review_worker_accepts_genuine_terminal_receipts(tmp_path, mode):
    import errno

    from worker_capture import capture_helper

    # /usr/bin/false exits immediately; the other path is absent in a new fixture.
    argv = ("/usr/bin/false",) if mode == "early-exit" else (str(tmp_path / "missing"),)
    receipt = capture_helper(argv, time.monotonic() + 1)
    retain(tmp_path, mode, receipt)
    worker = receipt.worker
    assert worker["reaped"] and not worker["unknown_helper_creation"]
    times = worker["published_times"]
    assert times is not None
    assert times["kill_monotonic"] is None
    if mode == "early-exit":
        assert receipt.error is None
        assert receipt.returncode == 1 and receipt.reaped
        assert receipt.pid is not None and times["created_monotonic"] is not None
        assert times["reaped_monotonic"] is not None
    else:
        assert receipt.error_errno == errno.ENOENT
        assert receipt.error and not receipt.error.startswith("worker capture failed:")
        assert receipt.pid is None and times["created_monotonic"] is None
        assert not receipt.reaped and times["reaped_monotonic"] is None
