"""Real-process controls for the interim per-query creation boundary."""

import errno
import json
import os
import select
import signal
import subprocess
import sys
import time
from pathlib import Path

import pytest

import inspection


@pytest.mark.parametrize("ready", [False, True], ids=["pid", "group"])
@pytest.mark.parametrize(
    "operation", ["final-reap", "stale-term", "stale-kill", "at-cutoff", "cleanup-cutoff"]
)
def test_worker_shutdown_deadline_closes_owner_operations(tmp_path, monkeypatch, ready, operation):
    """A deferred wait observation must not reopen expired worker authority."""
    import worker_capture

    original_waitpid = os.waitpid
    original_kill, original_killpg = os.kill, os.killpg
    original_signal = worker_capture._signal
    original_monotonic = time.monotonic
    cleanup_cutoff = False
    calls = []
    observed = []
    end = time.monotonic() + 0.3

    def child(control, result, token, argv, deadline):
        try:
            inspection.reset_after_fork({control, result})
            if ready:
                message = {
                    "token": token,
                    "pid": os.getpid(),
                    "pgid": os.getpgrp(),
                    "sid": os.getsid(0),
                }
                worker_capture._write(control, json.dumps(message).encode() + b"\n", deadline)
            # No helper is created. TERM/KILL and child lifetime remain real.
            while True:
                signal.pause()
        finally:
            os._exit(70)

    def waitpid(pid, flags):
        now = time.monotonic()
        calls.append({"operation": "waitpid", "pid": pid, "flags": flags, "time": now})
        # Simulated deferred observation, not kernel unreapability. The child
        # can really exit, but its status remains pinned until the independent audit.
        if now < end or operation != "final-reap":
            return 0, 0
        result = original_waitpid(pid, flags)
        observed.append(result)
        return result

    def kill(pid, sig):
        calls.append(
            {"operation": "kill", "pid": pid, "signal": int(sig), "time": time.monotonic()}
        )
        return original_kill(pid, sig)

    def killpg(pid, sig):
        calls.append(
            {"operation": "killpg", "pid": pid, "signal": int(sig), "time": time.monotonic()}
        )
        return original_killpg(pid, sig)

    def monotonic():
        return end if cleanup_cutoff else original_monotonic()

    def signal_at_cleanup_cutoff(owner, sig):
        nonlocal cleanup_cutoff
        if operation == "cleanup-cutoff" and sig == signal.SIGKILL:
            # capture_helper's cleanup loop has admitted this turn before end.
            # Cross to equality at its KILL entry; the following reap sees the
            # same original cutoff. No stale callback is invoked in this case.
            cleanup_cutoff = True
        return original_signal(owner, sig)

    monkeypatch.setattr(time, "monotonic", monotonic)
    monkeypatch.setattr(worker_capture, "_signal", signal_at_cleanup_cutoff)
    monkeypatch.setattr(worker_capture, "_child", child)
    monkeypatch.setattr(os, "waitpid", waitpid)
    monkeypatch.setattr(os, "kill", kill)
    monkeypatch.setattr(os, "killpg", killpg)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), end)
    crossed_in_cleanup = cleanup_cutoff
    cleanup_cutoff = False  # The independent audit uses the real clock.
    worker = receipt.worker
    pid = worker["pid"]
    audit = {"pid": pid, "adapter": "deferred parent WNOHANG observation", "reaped": False}
    try:
        if operation not in {"final-reap", "cleanup-cutoff"}:
            owner = worker_capture._owners[pid]
            with monkeypatch.context() as clock:
                if operation == "at-cutoff":
                    clock.setattr(time, "monotonic", lambda: end)
                    worker_capture._signal(owner, signal.SIGTERM)
                worker_capture._signal(
                    owner, signal.SIGTERM if operation == "stale-term" else signal.SIGKILL
                )
                worker_capture._reap(owner)
        retain(tmp_path, f"worker-deadline-{operation}-{ready}", receipt)
        assert crossed_in_cleanup is (operation == "cleanup-cutoff")
        assert not [call for call in calls if call["time"] >= end]
        assert receipt.error and receipt.residual_remedy
        assert not worker["reaped"] and pid in worker_capture._owners
        assert worker["shutdown_deadline_monotonic"] == end
        assert worker["signal_scope"] == ("group" if ready else "pid")
        assert any(call["operation"] == ("killpg" if ready else "kill") for call in calls)
        assert any(call["operation"] == "waitpid" for call in calls)
        assert all(call["flags"] == os.WNOHANG for call in calls if call["operation"] == "waitpid")
    finally:
        # Outside the runtime and adapter: audit only this exact unreaped child.
        # Bounded polling avoids adding a blocking wait to the evidence checker.
        audit_end = time.monotonic() + 1
        if observed and observed[-1][0] == pid:
            audit.update(reaped=True, status=observed[-1][1], source="late runtime reap")
        else:
            while time.monotonic() < audit_end:
                found, status = original_waitpid(pid, os.WNOHANG)
                if found == pid:
                    audit.update(reaped=True, status=status, source="independent exact-child reap")
                    break
                time.sleep(0.001)
        if audit["reaped"]:
            worker_capture._owners.pop(pid, None)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"worker-deadline-audit-{operation}-{ready}-{time.time_ns()}.json").write_text(
            json.dumps(
                {
                    "audit": audit,
                    "calls": calls,
                    "deadline": end,
                    "cleanup_cutoff": crossed_in_cleanup,
                },
                indent=2,
            )
            + "\n"
        )
        assert audit["reaped"]


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
        ("admission-ready-auto-reap", "inconsistent worker admission"),
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
        ("native-auto-reap-refusal", None),
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

    def synthetic_capture(argv, deadline, *, publish_created=None, publish_fork_error=None):
        # Only the helper receipt is synthetic; no helper is created. Worker
        # admission/fork/reset/framing/channels/decoding/shutdown remain real.
        now = time.monotonic()
        body = inspection.HelperCapture(
            pid=987654321,
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
            "admission-ready-auto-reap": {"native_sigchld": "SA_NOCLDWAIT"},
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
            "native-auto-reap-refusal": {
                "ok": False,
                "reason": "sigchld",
                "native_sigchld": "SA_NOCLDWAIT",
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
        if reason is None and body.pid is not None:
            publish_created(
                {
                    "pid": body.pid,
                    "ppid": os.getpid(),
                    "pgid": os.getpgrp(),
                    "created_monotonic": body.created_monotonic,
                }
            )
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
            if key == "residual_remedy" and mode == "lost-owner":
                assert body[key] is None
                assert receipt.residual_remedy and worker["helper_obligation_retained"]
                assert (
                    worker["helper_identity"]
                    == worker_capture._helper_obligations[worker["helper_identity"]["token"]]
                )
                continue
            assert getattr(receipt, key) == body[key]
        assert receipt.stdout.hex() == body["stdout"]
        assert worker["published_times"] is not None


def test_review_worker_preserves_real_helper_admission_refusal(tmp_path, monkeypatch):
    import threading

    import worker_capture

    original = inspection.capture_helper

    def threaded_capture(argv, deadline, *, publish_created=None, publish_fork_error=None):
        stop = threading.Event()
        thread = threading.Thread(target=stop.wait)
        thread.start()
        try:
            return original(
                argv,
                deadline,
                publish_created=publish_created,
                publish_fork_error=publish_fork_error,
            )
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

    def synthetic_capture(argv, deadline, *, publish_created=None, publish_fork_error=None):
        # Only the helper receipt is synthetic. The ordinary worker child builds
        # its frame, publishes on real channels, and exits through os._exit.
        now = time.monotonic()
        body = inspection.HelperCapture(
            pid=987654321,
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
        if reason is None and body.pid is not None:
            publish_created(
                {
                    "pid": body.pid,
                    "ppid": os.getpid(),
                    "pgid": os.getpgrp(),
                    "created_monotonic": body.created_monotonic,
                }
            )
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
        "malformed": "invalid worker frame length",
        "oversized-frame": "invalid worker frame length",
        "partial-frame": "incomplete worker frame length",
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
    if fault in {"malformed", "oversized-frame", "partial-frame"}:
        assert receipt.worker["receipt_monotonic"] is None
    elif publication.exists():
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
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    process = subprocess.Popen(["/bin/sleep", "10"], start_new_session=True)
    try:
        observation = controller._group_snapshot(process.pid, time.monotonic() + 1)
        helper = observation["helper"]
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
        worker = helper["worker"]
        query = worker["published_times"]
        assert (
            observation["started_monotonic"]
            <= worker["dispatch_monotonic"]
            <= query["started_monotonic"]
            <= query["finished_monotonic"]
            <= worker["receipt_monotonic"]
            <= observation["probe_started_monotonic"]
            <= observation["probe_finished_monotonic"]
            <= observation["finished_monotonic"]
        )
    finally:
        os.killpg(process.pid, signal.SIGKILL)
        status = process.wait(timeout=1)
        (root / f"worker-group-snapshot-audit-{time.time_ns()}.json").write_text(
            json.dumps({"pid": process.pid, "returncode": status, "reaped": True}, indent=2)
        )


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


@pytest.mark.parametrize("mode", ["early-exit", "exec-failure"])
def test_review_worker_accepts_genuine_terminal_receipts(tmp_path, mode):
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
    # Raw fork returns a known child; failed exec uses its explicit _exit(70).
    # No child-error channel exists yet, so this does not claim an exec errno.
    assert receipt.error is None and receipt.error_errno is None
    assert receipt.returncode == (1 if mode == "early-exit" else 70) and receipt.reaped
    assert receipt.pid is not None and times["created_monotonic"] is not None
    assert times["reaped_monotonic"] is not None


def test_review_created_identity_survives_worker_result_loss(tmp_path, monkeypatch):
    """Dropping the result must retain the published real helper identity."""
    import worker_capture

    original = worker_capture._write

    def lose_result(fd, data, deadline):
        if data.startswith(worker_capture.HEADER.pack(len(data) - worker_capture.HEADER.size)):
            raise OSError("injected terminal publication loss")
        return original(fd, data, deadline)

    monkeypatch.setattr(worker_capture, "_write", lose_result)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 1)
    retain(tmp_path, "created-result-loss", receipt)
    worker = receipt.worker
    assert worker["reaped"]
    identity = worker.get("helper_identity")
    assert identity is not None, "worker loss discarded the helper identity"
    assert identity["ppid"] == worker["pid"] == identity["pgid"]
    assert identity["pid"] != worker["pid"]
    assert worker["helper_obligation_retained"]
    assert not worker["unknown_helper_creation"]
    assert worker_capture._helper_obligations[identity["token"]] == identity
    assert receipt.error and receipt.residual_remedy


def test_review_matching_reap_resolves_created_identity(tmp_path):
    import worker_capture

    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 1)
    retain(tmp_path, "created-reap", receipt)
    assert receipt.error is None and receipt.reaped
    identity = receipt.worker.get("helper_identity")
    assert identity is not None, "successful capture omitted the creation publication"
    assert identity["pid"] == receipt.pid
    assert identity["created_monotonic"] == receipt.created_monotonic
    assert not receipt.worker["helper_obligation_retained"]
    assert identity["token"] not in worker_capture._helper_obligations


@pytest.mark.parametrize(
    "mode",
    [
        "duplicate",
        "token",
        "ppid",
        "pgid",
        "boolean-pid",
        "stale",
        "future",
        "deadline",
        "extra",
        "result-pid",
        "result-created",
    ],
)
def test_review_created_identity_rejects_conflicts(tmp_path, monkeypatch, mode):
    import worker_capture

    original = worker_capture._write

    def corrupt(fd, data, deadline):
        if data.startswith(b"{"):
            message = json.loads(data)
            if "created_monotonic" in message:
                if mode == "duplicate":
                    original(fd, data, deadline)
                elif mode == "token":
                    message["token"] = "another-query"
                elif mode in {"ppid", "pgid"}:
                    message[mode] += 1
                elif mode == "boolean-pid":
                    message["pid"] = True
                elif mode == "stale":
                    message["created_monotonic"] = 0
                elif mode == "future":
                    message["created_monotonic"] = deadline + 1
                elif mode == "deadline":
                    message["deadline_monotonic"] += 1
                elif mode == "extra":
                    message["unexpected"] = 1
                data = json.dumps(message).encode() + b"\n"
        elif mode.startswith("result-") and len(data) > 4:
            if worker_capture.HEADER.unpack(data[:4])[0] == len(data) - 4:
                message = json.loads(data[4:])
                message["pid" if mode == "result-pid" else "created_monotonic"] += (
                    1 if mode == "result-pid" else 0.000001
                )
                payload = json.dumps(message).encode()
                data = worker_capture.HEADER.pack(len(payload)) + payload
        original(fd, data, deadline)

    monkeypatch.setattr(worker_capture, "_write", corrupt)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.6)
    retain(tmp_path, f"created-conflict-{mode}", receipt)
    worker = receipt.worker
    assert worker["reaped"]
    known = mode in {"duplicate", "result-pid", "result-created"}
    assert worker["unknown_helper_creation"] is not known
    assert worker["helper_obligation_retained"] is known
    assert bool(worker["helper_identity"]) is known
    assert receipt.error and receipt.residual_remedy
    expected = (
        "duplicate helper creation publication"
        if mode == "duplicate"
        else "worker result/created identity mismatch"
        if mode.startswith("result-")
        else "invalid helper creation fields"
        if mode == "extra"
        else "invalid helper creation identity"
    )
    assert receipt.error == f"worker capture failed: {expected}"
    if known:
        identity = worker["helper_identity"]
        assert worker_capture._helper_obligations[identity["token"]] == identity


def test_review_worker_exit_after_publication_keeps_gate_closed(tmp_path, monkeypatch):
    import worker_capture

    marker = tmp_path / "executed"
    original = worker_capture._write

    def exit_after_created(fd, data, deadline):
        original(fd, data, deadline)
        if data.startswith(b"{") and "created_monotonic" in json.loads(data):
            os._exit(70)

    monkeypatch.setattr(worker_capture, "_write", exit_after_created)
    receipt = worker_capture.capture_helper(
        ("/bin/sh", "-c", f"echo executed > {marker}"), time.monotonic() + 0.5
    )
    retain(tmp_path, "created-pre-gate-worker-exit", receipt)
    worker = receipt.worker
    assert worker["reaped"] and worker["returncode"] == 70
    assert not worker["unknown_helper_creation"] and worker["helper_obligation_retained"]
    assert not marker.exists()
    identity = worker["helper_identity"]
    assert worker_capture._helper_obligations[identity["token"]] == identity


@pytest.mark.parametrize("first_eof", ["control", "result"])
def test_fork_failure_lost_result_eof_order(tmp_path, monkeypatch, first_eof):
    """Both real EOF orders preserve an already admitted no-child failure."""
    import worker_capture

    controller_pid = os.getpid()
    original_fork, original_pipe = os.fork, os.pipe
    original_read, original_write = os.read, worker_capture._write
    pairs = []
    failure_seen = False
    observed = []

    def pipe():
        pair = original_pipe()
        if os.getpid() == controller_pid:
            pairs.append(pair)
        return pair

    def fork():
        if os.getpid() != controller_pid:
            raise OSError(errno.EAGAIN, "injected at helper fork boundary")
        return original_fork()

    def write(fd, data, deadline):
        if not data.endswith(b"\n"):
            os._exit(70)
        return original_write(fd, data, deadline)

    def read(fd, size):
        nonlocal failure_seen
        if os.getpid() != controller_pid or len(pairs) < 2:
            return original_read(fd, size)
        names = {pairs[0][0]: "control", pairs[1][0]: "result"}
        name = names.get(fd)
        # Defer observation, never synthesize bytes or EOF. In both schedules
        # the complete failure message precedes the empty result channel.
        if name == "result" and not failure_seen:
            raise BlockingIOError(errno.EAGAIN, "deferred result observation")
        chunk = original_read(fd, size)
        if name == "control" and b'"failed_monotonic"' in chunk:
            failure_seen = True
        if name is not None and not chunk:
            if name != first_eof and first_eof not in observed:
                raise BlockingIOError(errno.EAGAIN, "deferred EOF observation")
            if name not in observed:
                observed.append(name)
        return chunk

    monkeypatch.setattr(os, "pipe", pipe)
    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(os, "read", read)
    monkeypatch.setattr(worker_capture, "_write", write)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 1)
    retain(tmp_path, "fork-eof-" + first_eof, receipt)
    assert observed == [first_eof, "result" if first_eof == "control" else "control"]
    assert receipt.error is not None and receipt.error_errno == errno.EAGAIN
    assert receipt.pid is None and receipt.worker["helper_identity"] is None
    assert receipt.worker["reaped"] and receipt.worker["returncode"] == 70
    assert receipt.worker["fork_failure"]["errno"] == errno.EAGAIN
    assert not receipt.worker["unknown_helper_creation"]
    assert not receipt.worker["helper_obligation_retained"]
    assert not receipt.residual_remedy


@pytest.mark.parametrize("mode", ["open", "cleanup-eof"])
def test_fork_failure_empty_control_channel(tmp_path, monkeypatch, mode):
    """An empty open pipe is not EOF; cleanup EOF may frame an on-time failure.

    Requiring only result EOF, or treating a control EAGAIN as EOF, must fail
    the open case. The holder is a separate exact child of the controller so
    it survives worker exit without leaving an orphan for somebody else to reap.
    """
    import worker_capture

    controller_pid = os.getpid()
    original_pipe, original_fork, original_read = os.pipe, os.fork, os.read
    original_write = worker_capture._write
    pairs = []
    holder = None
    failure_seen = False
    result_eof = False
    control_eagain = 0
    control_eof = None
    release_at = None
    holder_alive = None
    deadline = time.monotonic() + 0.8
    # This allowance takes the existing maximum 50ms reserve. No clock is faked.
    query_cutoff = deadline - 0.05

    def pipe():
        pair = original_pipe()
        if os.getpid() == controller_pid:
            pairs.append(pair)
        return pair

    def fork():
        nonlocal holder
        if os.getpid() != controller_pid:
            raise OSError(errno.EAGAIN, "injected at helper fork boundary")
        assert holder is None and len(pairs) == 2
        control_write = pairs[0][1]
        holder = subprocess.Popen(
            [
                sys.executable,
                "-c",
                "import os,select,sys; select.select([0],[],[],2); os.close(int(sys.argv[1]))",
                str(control_write),
            ],
            pass_fds=(control_write,),
            stdin=subprocess.PIPE,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
            start_new_session=True,
        )
        return original_fork()

    def write(fd, data, cutoff):
        if not data.endswith(b"\n"):
            os._exit(70)
        return original_write(fd, data, cutoff)

    def read(fd, size):
        nonlocal failure_seen, result_eof, control_eagain, control_eof, release_at
        if os.getpid() != controller_pid or len(pairs) < 2:
            return original_read(fd, size)
        control = fd == pairs[0][0]
        if control and mode == "cleanup-eof" and time.monotonic() >= query_cutoff:
            assert holder is not None and holder.stdin is not None
            if release_at is None:
                release_at = time.monotonic()
                holder.stdin.close()
        try:
            chunk = original_read(fd, size)
        except BlockingIOError:
            if control and failure_seen:
                control_eagain += 1
            raise
        if control:
            if b'"failed_monotonic"' in chunk:
                failure_seen = True
            if not chunk:
                control_eof = time.monotonic()
        elif fd == pairs[1][0] and not chunk:
            result_eof = True
        return chunk

    monkeypatch.setattr(os, "pipe", pipe)
    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(os, "read", read)
    monkeypatch.setattr(worker_capture, "_write", write)
    try:
        receipt = worker_capture.capture_helper(("/usr/bin/true",), deadline)
    finally:
        if holder is not None:
            holder_alive = holder.poll() is None
            assert holder.stdin is not None
            holder.stdin.close()
            try:
                holder.wait(timeout=1)
            except subprocess.TimeoutExpired:
                holder.kill()
                holder.wait(timeout=1)
            root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
            root.mkdir(parents=True, exist_ok=True)
            (root / f"empty-control-{mode}-{time.time_ns()}.json").write_text(
                json.dumps(
                    {
                        "mode": mode,
                        "holder_pid": holder.pid,
                        "holder_pgid": holder.pid,
                        "holder_alive_after_capture": holder_alive,
                        "holder_reaped": holder.returncode is not None,
                        "holder_returncode": holder.returncode,
                        "failure_seen": failure_seen,
                        "control_eagain": control_eagain,
                        "control_eof_monotonic": control_eof,
                        "result_eof": result_eof,
                        "release_monotonic": release_at,
                        "query_cutoff": query_cutoff,
                        "deadline": deadline,
                    },
                    indent=2,
                )
            )
    retain(tmp_path, "empty-control-" + mode, receipt)
    assert holder is not None and holder.returncode == 0
    assert failure_seen and result_eof and control_eagain > 0
    assert receipt.error is not None
    assert receipt.worker["fork_failure"]["failed_monotonic"] < query_cutoff
    assert receipt.worker["reaped"] and receipt.worker["returncode"] == 70
    assert receipt.pid is None and receipt.worker["helper_identity"] is None
    assert not receipt.worker["helper_obligation_retained"]
    assert not receipt.worker["creation_conflict"]
    if mode == "open":
        assert holder_alive and control_eof is None
        assert receipt.worker["unknown_helper_creation"]
        assert receipt.residual_remedy and receipt.error_errno is None
    else:
        assert release_at is not None and query_cutoff <= release_at
        assert control_eof is not None and query_cutoff <= control_eof < deadline
        assert not receipt.worker["unknown_helper_creation"]
        assert not receipt.residual_remedy and receipt.error_errno == errno.EAGAIN


@pytest.mark.parametrize("mode", ["fork-error", "lost-result", "setup-error", "silent"])
def test_review_worker_fork_failure_evidence(tmp_path, monkeypatch, mode):
    import errno

    import worker_capture

    controller_pid = os.getpid()
    original_fork = os.fork
    original_pipe = inspection._capture_pipe
    original_write = worker_capture._write

    def fork():
        if os.getpid() != controller_pid:
            if mode == "silent":
                os._exit(70)
            raise OSError(errno.EAGAIN, "injected at helper fork boundary")
        return original_fork()

    def pipe():
        if mode == "setup-error":
            raise OSError(errno.EMFILE, "injected before helper fork")
        return original_pipe()

    def write(fd, data, deadline):
        if mode in {"lost-result", "setup-error"} and not data.endswith(b"\n"):
            os._exit(70)
        return original_write(fd, data, deadline)

    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(inspection, "_capture_pipe", pipe)
    monkeypatch.setattr(worker_capture, "_write", write)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 1)
    retain(tmp_path, "fork-failure-" + mode, receipt)
    worker = receipt.worker
    assert worker["reaped"] is True
    assert receipt.error is not None
    assert receipt.pid is None
    assert worker["helper_identity"] is None
    assert worker["helper_obligation_retained"] is False
    confirmed = mode in {"fork-error", "lost-result"}
    assert worker["unknown_helper_creation"] is (not confirmed)
    if confirmed:
        assert worker["fork_failure"]["errno"] == errno.EAGAIN
        assert receipt.error_errno == errno.EAGAIN
    else:
        assert worker["fork_failure"] is None


@pytest.mark.parametrize(
    "mode",
    [
        "errno-bool",
        "errno-zero",
        "errno-negative",
        "errno-string",
        "errno-missing",
        "extra-field",
        "wrong-token",
        "wrong-deadline",
        "before-dispatch",
        "future",
        "nan",
        "duplicate-key",
        "duplicate",
        "failure-then-created",
        "created-then-failure",
        "terminal-errno",
        "terminal-admission",
        "terminal-time",
        "worker-publication-expired",
    ],
)
def test_review_worker_rejects_invalid_fork_failure(tmp_path, monkeypatch, mode):
    import errno

    import worker_capture

    controller_pid = os.getpid()
    original_fork = os.fork
    original_write = worker_capture._write
    failure_stamp = None

    def fork():
        if os.getpid() != controller_pid:
            raise OSError(errno.EAGAIN, "injected helper fork failure")
        return original_fork()

    def write(fd, data, deadline):
        nonlocal failure_stamp
        if data.startswith(b"{") and b'"failed_monotonic"' in data:
            value = json.loads(data)
            failure_stamp = value["failed_monotonic"]
            if mode.startswith("errno-"):
                if mode == "errno-missing":
                    value.pop("errno")
                else:
                    value["errno"] = {
                        "errno-bool": True,
                        "errno-zero": 0,
                        "errno-negative": -1,
                        "errno-string": "11",
                    }[mode]
            elif mode == "extra-field":
                value["extra"] = 1
            elif mode == "wrong-token":
                value["token"] = "different"
            elif mode == "wrong-deadline":
                value["deadline_monotonic"] += 1
            elif mode == "before-dispatch":
                value["failed_monotonic"] = 0
            elif mode == "future":
                value["failed_monotonic"] = deadline + 1
            elif mode == "nan":
                value["failed_monotonic"] = float("nan")
            elif mode == "worker-publication-expired":
                time.sleep(max(0, deadline - time.monotonic()) + 0.01)
            data = json.dumps(value).encode() + b"\n"
            if mode == "duplicate-key":
                data = data[:-2] + b', "errno": 11}\n'
            created = (
                json.dumps(
                    {
                        "token": value["token"],
                        "deadline_monotonic": deadline,
                        "pid": 987654321,
                        "ppid": os.getpid(),
                        "pgid": os.getpgrp(),
                        "created_monotonic": time.monotonic(),
                    }
                ).encode()
                + b"\n"
            )
            if mode == "created-then-failure":
                data = created + data
            elif mode == "failure-then-created":
                data += created
            elif mode == "duplicate":
                data += data
        elif not data.endswith(b"\n") and mode.startswith("terminal-"):
            value = json.loads(data[worker_capture.HEADER.size :])
            if mode == "terminal-errno":
                value["error_errno"] = errno.ENOMEM
            elif mode == "terminal-admission":
                value["admission"]["ok"] = False
            else:
                value["started_monotonic"] = failure_stamp + 0.000001
            payload = json.dumps(value).encode()
            data = worker_capture.HEADER.pack(len(payload)) + payload
        return original_write(fd, data, deadline)

    before = set(worker_capture._helper_obligations)
    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(worker_capture, "_write", write)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.6)
    retain(tmp_path, "invalid-fork-failure-" + mode, receipt)
    assert receipt.error is not None
    assert receipt.worker["reaped"] is True
    known = mode in {"created-then-failure", "failure-then-created"}
    assert receipt.worker["unknown_helper_creation"] is (not known)
    assert receipt.worker["helper_obligation_retained"] is known
    if known:
        assert receipt.worker["helper_identity"]["pid"] == 987654321
        # This fixture published a synthetic identity; never signal it.
        for token in set(worker_capture._helper_obligations) - before:
            worker_capture._helper_obligations.pop(token)


@pytest.mark.parametrize(
    "mode", ["control-tail", "short-header", "short-frame", "oversized", "open-tail"]
)
def test_review_fork_failure_requires_clean_channel_end(tmp_path, monkeypatch, mode):
    import errno

    import worker_capture

    parent = os.getpid()
    original_fork, original_write = os.fork, worker_capture._write

    def fork():
        if os.getpid() != parent:
            raise OSError(errno.EAGAIN, "injected helper fork refusal")
        return original_fork()

    def write(fd, data, deadline):
        if data.startswith(b"{") and b'"failed_monotonic"' in data:
            original_write(fd, data, deadline)
            if mode in {"control-tail", "open-tail"}:
                original_write(fd, b'{"pid":987654321', deadline)
            if mode == "open-tail":
                time.sleep(2)  # Real open pipe; the parent must retire this worker.
            return
        if not data.endswith(b"\n"):
            data = {
                "control-tail": b"",
                "short-header": b"\x00",
                "short-frame": worker_capture.HEADER.pack(100) + b"{",
                "oversized": worker_capture.HEADER.pack(worker_capture.FRAME_CAP + 1),
            }.get(mode, data)
            if not data:
                os._exit(70)
        return original_write(fd, data, deadline)

    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(worker_capture, "_write", write)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.5)
    retain(tmp_path, "fork-failure-channel-" + mode, receipt)
    assert receipt.worker["fork_failure"]["errno"] == errno.EAGAIN
    assert receipt.error and receipt.residual_remedy
    assert receipt.worker["unknown_helper_creation"]
    assert receipt.worker["reaped"]


@pytest.mark.parametrize("mode", ["timeout", "broken-pipe"])
def test_review_fork_failure_preserves_errno_on_publication_error(tmp_path, monkeypatch, mode):
    import errno

    import worker_capture

    parent = os.getpid()
    original_fork, original_write = os.fork, worker_capture._write

    def fork():
        if os.getpid() != parent:
            raise OSError(errno.EAGAIN, "injected helper fork refusal")
        return original_fork()

    def write(fd, data, deadline):
        if data.startswith(b"{") and b'"failed_monotonic"' in data:
            if mode == "timeout":
                raise TimeoutError("injected publication timeout")
            raise BrokenPipeError(errno.EPIPE, "injected publication broken pipe")
        return original_write(fd, data, deadline)

    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(worker_capture, "_write", write)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.6)
    retain(tmp_path, "fork-failure-callback-" + mode, receipt)
    assert receipt.error_errno == errno.EAGAIN
    assert "injected helper fork refusal" in receipt.error
    assert "injected publication" in receipt.error
    assert receipt.worker["fork_failure"] is None
    assert receipt.worker["reaped"] and receipt.pid is None


def test_review_distinct_created_identities_remain_obligations(tmp_path, monkeypatch):
    import errno

    import worker_capture

    parent = os.getpid()
    original_fork, original_write = os.fork, worker_capture._write
    before = set(worker_capture._helper_obligations)
    fake_pids = {987654321, 987654322}

    def fork():
        if os.getpid() != parent:
            raise OSError(errno.EAGAIN, "injected no real helper")
        return original_fork()

    def write(fd, data, deadline):
        if data.startswith(b"{") and b'"failed_monotonic"' in data:
            failure = json.loads(data)
            data = b"".join(
                json.dumps(
                    {
                        "token": failure["token"],
                        "deadline_monotonic": deadline,
                        "pid": pid,
                        "ppid": os.getpid(),
                        "pgid": os.getpgrp(),
                        "created_monotonic": time.monotonic(),
                    }
                ).encode()
                + b"\n"
                for pid in sorted(fake_pids)
            )
        return original_write(fd, data, deadline)

    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(worker_capture, "_write", write)
    try:
        receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.6)
        retain(tmp_path, "distinct-created-identities", receipt)
        assert receipt.error and receipt.residual_remedy
        obligations = [
            worker_capture._helper_obligations[key]
            for key in set(worker_capture._helper_obligations) - before
        ]
        assert {item["pid"] for item in obligations} == fake_pids
        assert {item["pid"] for item in receipt.worker["helper_identities"]} == fake_pids
    finally:
        # Evidence-only synthetic identities never acquire signal authority.
        for key in set(worker_capture._helper_obligations) - before:
            worker_capture._helper_obligations.pop(key)


@pytest.mark.parametrize("delay", ["read", "select"])
def test_review_parent_rejects_published_late_fork_failure(tmp_path, monkeypatch, delay):
    import errno

    import worker_capture

    parent = os.getpid()
    original_fork, original_write = os.fork, worker_capture._write
    original_read, original_select = os.read, worker_capture.select.select
    marker = tmp_path / "failure-published.json"
    end = time.monotonic() + 0.6
    delayed = False

    def fork():
        if os.getpid() != parent:
            raise OSError(errno.EAGAIN, "injected helper fork refusal")
        return original_fork()

    def write(fd, data, deadline):
        original_write(fd, data, deadline)
        if data.startswith(b"{") and b'"failed_monotonic"' in data:
            marker.write_bytes(data)

    def pause():
        nonlocal delayed
        delayed = True
        time.sleep(max(0, end - time.monotonic()) + 0.01)

    def read(fd, count):
        data = original_read(fd, count)
        if os.getpid() == parent and delay == "read" and b'"failed_monotonic"' in data:
            pause()
        return data

    def select(reads, writes, errors, timeout):
        result = original_select(reads, writes, errors, timeout)
        if os.getpid() == parent and delay == "select" and not delayed:
            until = time.monotonic() + 0.1
            while not marker.exists() and time.monotonic() < until:
                time.sleep(0.001)
            assert marker.exists()
            pause()
        return result

    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(worker_capture, "_write", write)
    monkeypatch.setattr(os, "read", read)
    monkeypatch.setattr(worker_capture.select, "select", select)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), end)
    retain(tmp_path, "parent-late-fork-failure-" + delay, receipt)
    assert marker.exists() and delayed
    assert receipt.error and receipt.residual_remedy
    assert receipt.worker["unknown_helper_creation"]
    assert receipt.worker["fork_failure"] is None


@pytest.mark.parametrize("mode", ["unclean-exit", "unreaped"])
def test_review_confirmed_fork_failure_keeps_worker_failure(tmp_path, monkeypatch, mode):
    import errno

    import worker_capture

    parent = os.getpid()
    original_fork, original_write = os.fork, worker_capture._write

    def fork():
        if os.getpid() != parent:
            raise OSError(errno.EAGAIN, "injected helper fork refusal")
        return original_fork()

    def write(fd, data, deadline):
        original_write(fd, data, deadline)
        if not data.endswith(b"\n"):
            os._exit(23)

    monkeypatch.setattr(os, "fork", fork)
    monkeypatch.setattr(worker_capture, "_write", write)
    if mode == "unreaped":
        monkeypatch.setattr(worker_capture, "_reap", lambda owner: None)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.6)
    retain(tmp_path, "fork-failure-worker-" + mode, receipt)
    worker = receipt.worker
    try:
        assert receipt.error and receipt.error_errno == errno.EAGAIN
        assert not worker["unknown_helper_creation"]
        if mode == "unclean-exit":
            assert worker["reaped"] and worker["returncode"] == 23
        else:
            assert not worker["reaped"] and receipt.residual_remedy
    finally:
        if mode == "unreaped":
            owner = worker_capture._owners[worker["pid"]]
            os.waitpid(owner.pid, 0)  # Independent exact-child cleanup after the adapter.
            worker_capture._owners.pop(owner.pid)
            (tmp_path / "independent-worker-reap.json").write_text(
                json.dumps(
                    {
                        "pid": owner.pid,
                        "reaped": True,
                        "adapter": "withheld parent WNOHANG observation",
                    }
                )
            )


@pytest.mark.parametrize("delay", ["read", "select", "shutdown"])
@pytest.mark.parametrize(
    "publication",
    ["valid", "distinct", "malformed", "overflow", "valid-overflow", "distinct-overflow"],
)
def test_late_created_identity_is_cleanup_only(tmp_path, monkeypatch, delay, publication):
    """Dropping late controls loses identities; decoding late terminal grants false reap."""
    import worker_capture

    parent = os.getpid()
    original_write, original_read = worker_capture._write, os.read
    original_select = worker_capture.select.select
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    root.mkdir(parents=True, exist_ok=True)
    witness = root / f"late-created-{delay}-{publication}-{time.time_ns()}.json"
    end = time.monotonic() + 0.4
    delayed = False
    held = None
    terminated = False
    control_reads = []
    before = set(worker_capture._helper_obligations)

    def term(sig, frame):
        nonlocal terminated
        terminated = True

    def publish_witness(info):
        temporary = witness.with_suffix(".tmp")
        temporary.write_text(json.dumps(info))
        temporary.replace(witness)

    def write(fd, data, deadline):
        nonlocal held
        if data.startswith(b"{") and b'"created_monotonic"' in data:
            identity = json.loads(data)
            publish_witness({"identity": identity, "query_deadline": deadline})
            if publication == "malformed":
                identity["ppid"] = parent
                data = json.dumps(identity).encode() + b"\n"
            elif publication in {"distinct", "distinct-overflow"}:
                other = dict(identity, pid=987654322)
                data += json.dumps(other).encode() + b"\n"
            elif publication == "overflow":
                data = b"x" * 4097 + b"\n" + data
            elif publication == "partial":
                data = data.rstrip(b"\n")
            elif publication in {"token", "boolean-pid", "stale", "future", "deadline", "extra"}:
                if publication == "token":
                    identity["token"] = "other-query"
                elif publication == "boolean-pid":
                    identity["pid"] = True
                elif publication == "stale":
                    identity["created_monotonic"] = 0
                elif publication == "future":
                    identity["created_monotonic"] = deadline
                elif publication == "deadline":
                    identity["deadline_monotonic"] += 1
                else:
                    identity["extra"] = 1
                data = json.dumps(identity).encode() + b"\n"
            if publication in {"valid-overflow", "distinct-overflow"}:
                data += b"x" * 4097
            if delay == "shutdown":
                # TERM-handler adapter, unlike production SIG_DFL. Hold only
                # the publication; the real helper executes and is reaped.
                # TERM triggers actual pipe delivery.
                signal.signal(signal.SIGTERM, term)
                held = (fd, data)
                return
        if delay == "prebuffer" and not data.endswith(b"\n"):
            # Production SIG_DFL: the helper has completed and been reaped;
            # its identity is already in the pipe before parent TERM.
            assert signal.getsignal(signal.SIGTERM) == signal.SIG_DFL
            info = json.loads(witness.read_text())
            info.update(pipe_before_term=True, term_disposition="SIG_DFL")
            publish_witness(info)
            while time.monotonic() < end:
                time.sleep(0.001)
            os._exit(73)  # Parent TERM must win, otherwise the test fails.
        if delay == "shutdown" and not data.endswith(b"\n"):
            while not terminated and time.monotonic() < end:
                time.sleep(0.001)
            assert terminated and held is not None
            os.write(held[0], held[1])
            os.write(fd, data)  # Late terminal/reap must never retire the obligation.
            info = json.loads(witness.read_text())
            info.update(published_monotonic=time.monotonic(), after_term=terminated)
            publish_witness(info)
            return
        original_write(fd, data, deadline)

    def pause():
        nonlocal delayed
        delayed = True
        time.sleep(max(0, end - 0.045 - time.monotonic()))

    def read(fd, count):
        data = original_read(fd, count)
        if os.getpid() == parent and (b'"created_monotonic"' in data or b"x" * 100 in data):
            control_reads.append(data)
        if os.getpid() == parent and delay == "read" and not delayed:
            if b'"created_monotonic"' in data or b"x" * 100 in data:
                pause()
        return data

    def select(reads, writes, errors, timeout):
        result = original_select(reads, writes, errors, timeout)
        if os.getpid() == parent and delay in {"select", "prebuffer"} and not delayed:
            until = time.monotonic() + 0.15
            while time.monotonic() < until:
                if witness.exists():
                    info = json.loads(witness.read_text())
                    if delay == "select" or info.get("pipe_before_term"):
                        break
                time.sleep(0.001)
            else:
                raise AssertionError("worker did not prebuffer the identity")
            pause()
        return result

    monkeypatch.setattr(worker_capture, "_write", write)
    monkeypatch.setattr(os, "read", read)
    monkeypatch.setattr(worker_capture.select, "select", select)
    try:
        receipt = worker_capture.capture_helper(("/usr/bin/true",), end)
        retain(tmp_path, f"late-created-{delay}-{publication}", receipt)
        worker = receipt.worker
        assert receipt.error and receipt.residual_remedy
        assert worker["reaped"] and worker["shutdown_deadline_monotonic"] == end
        assert worker["query_deadline_monotonic"] == end - 0.05
        assert receipt.finished_monotonic <= end + 0.015
        assert receipt.pid is None and not receipt.reaped
        assert worker["fork_failure"] is None
        if delay == "select":
            # Readiness-flag test: this fixture may already be reaped, so it
            # makes no claim about an observed PID-versus-group signal.
            assert worker["signal_scope"] == "pid"
        info = json.loads(witness.read_text())
        if delay == "prebuffer":
            assert info["pipe_before_term"] and info["term_disposition"] == "SIG_DFL"
            assert worker["returncode"] == -signal.SIGTERM
            assert any(item["signal"] == "SIGTERM" for item in worker["signals"])
        if delay == "shutdown":
            assert info["after_term"] and info["published_monotonic"] >= end - 0.05
        else:
            assert delayed
        identities = [
            worker_capture._helper_obligations[key]
            for key in set(worker_capture._helper_obligations) - before
        ]
        expected = (
            {info["identity"]["pid"]}
            if publication in {"valid", "distinct", "valid-overflow", "distinct-overflow"}
            else set()
        )
        if publication in {"distinct", "distinct-overflow"}:
            expected.add(987654322)
        if publication in {"valid-overflow", "distinct-overflow"}:
            # The identity prefix and overflow must actually share a read.
            assert len(control_reads) == 1
            chunk = control_reads[0]
            assert b'"created_monotonic"' in chunk and chunk.endswith(b"x" * 100)
            if publication == "distinct-overflow":
                assert b"987654322" in chunk
            (root / (witness.stem + "-read.json")).write_text(
                json.dumps({"bytes": len(chunk), "hex": chunk.hex()})
            )
        assert {item["pid"] for item in identities} == expected
        assert {item["pid"] for item in worker["helper_identities"]} == expected
        assert worker["unknown_helper_creation"] == (not expected)
    finally:
        # Tests release evidence-only entries; independent audit checks real PIDs.
        for key in set(worker_capture._helper_obligations) - before:
            worker_capture._helper_obligations.pop(key)


@pytest.mark.parametrize(
    "publication", ["token", "boolean-pid", "stale", "future", "deadline", "extra", "partial"]
)
def test_late_created_identity_rejects_invalid_fields(tmp_path, monkeypatch, publication):
    test_late_created_identity_is_cleanup_only(tmp_path, monkeypatch, "shutdown", publication)


@pytest.mark.parametrize("publication", ["valid", "distinct"])
def test_late_created_identity_prebuffered_before_default_term(tmp_path, monkeypatch, publication):
    test_late_created_identity_is_cleanup_only(tmp_path, monkeypatch, "prebuffer", publication)


@pytest.fixture(
    params=[
        "oversized-header",
        "short-header",
        "short-body",
        "trailing",
        "overflow",
        "zero-header",
        "one-header",
        "fragmented-oversized-header",
    ]
)
def invalid_result_frame(tmp_path, monkeypatch, request):
    """EOF-only validation waits out the query despite already-invalid framing."""
    import worker_capture

    mode = request.param
    parent = os.getpid()
    original_read = os.read
    reads = {}
    result_fd = None

    original_pipe = os.pipe
    pipes = []

    def pipe():
        nonlocal result_fd
        pair = original_pipe()
        pipes.append(pair)
        if len(pipes) == 2:
            result_fd = pair[0]
        return pair

    def read(fd, count):
        data = original_read(fd, count)
        if os.getpid() == parent:
            reads.setdefault(fd, []).append({"requested": count, "received": len(data)})
        return data

    def child(control, result, token, argv, deadline):
        try:
            inspection.reset_after_fork({control, result})
            ready = {"token": token, "pid": os.getpid(), "pgid": os.getpgrp(), "sid": os.getsid(0)}
            worker_capture._write(control, json.dumps(ready).encode() + b"\nintent\n", deadline)
            frames = {
                "oversized-header": worker_capture.HEADER.pack(worker_capture.FRAME_CAP + 1),
                "zero-header": worker_capture.HEADER.pack(0),
                "one-header": worker_capture.HEADER.pack(1),
                "fragmented-oversized-header": worker_capture.HEADER.pack(
                    worker_capture.FRAME_CAP + 1
                ),
                "short-header": b"\x00\x00",
                "short-body": worker_capture.HEADER.pack(100) + b"{",
                "trailing": worker_capture.HEADER.pack(2) + b"{}x",
                "overflow": worker_capture.HEADER.pack(worker_capture.FRAME_CAP)
                + b"x" * (worker_capture.FRAME_CAP + worker_capture.TURN_CAP),
            }
            if mode == "fragmented-oversized-header":
                for fragment in (frames[mode][:1], frames[mode][1:3], frames[mode][3:]):
                    worker_capture._write(result, fragment, deadline)
                    time.sleep(0.002)
            else:
                worker_capture._write(result, frames[mode], deadline)
            if mode in {"short-header", "short-body"}:
                os.close(result)
            # Real open control channel; no helper is ever forked in this fixture.
            time.sleep(2)
        finally:
            os._exit(74)

    monkeypatch.setattr(worker_capture, "_child", child)
    monkeypatch.setattr(os, "read", read)
    monkeypatch.setattr(os, "pipe", pipe)
    started = time.monotonic()
    end = started + 0.9
    receipt = worker_capture.capture_helper(("/usr/bin/true",), end)
    retain(tmp_path, "incremental-frame-" + mode, receipt)
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "receipts"))
    (root / f"frame-reads-{mode}-{time.time_ns()}.json").write_text(json.dumps(reads))
    return receipt, reads[result_fd], started, end, mode


def test_result_framing_error(invalid_result_frame):
    receipt, _, _, _, _ = invalid_result_frame
    assert receipt.worker["reaped"] and receipt.worker["unknown_helper_creation"]
    assert receipt.error and receipt.residual_remedy and receipt.pid is None
    assert "worker frame length" in receipt.error
    assert receipt.worker["receipt_monotonic"] is None


def test_result_framing_term(invalid_result_frame):
    receipt, _, started, _, _ = invalid_result_frame
    assert receipt.worker["signals"][0]["signal"] == "SIGTERM"
    assert receipt.worker["signals"][0]["monotonic"] - started < 0.45, "late first TERM"


def test_result_framing_finish(invalid_result_frame):
    receipt, _, started, _, _ = invalid_result_frame
    assert receipt.finished_monotonic - started < 0.45, "late framing return"


def test_result_framing_deadline(invalid_result_frame):
    receipt, _, _, end, _ = invalid_result_frame
    assert receipt.worker["shutdown_deadline_monotonic"] == end
    assert receipt.worker["query_deadline_monotonic"] == end - 0.05
    assert receipt.finished_monotonic <= end + 0.015


def test_result_framing_read_budget(invalid_result_frame):
    import worker_capture

    _, reads, _, _, mode = invalid_result_frame
    received = 0
    declared = {"short-body": 100, "trailing": 2, "overflow": worker_capture.FRAME_CAP}
    for item in reads:
        assert received < 4 or mode in declared, "result read after rejected header"
        cap = (
            4 - received
            if received < 4
            else min(worker_capture.TURN_CAP // 2, 4 + declared[mode] - received + 1)
        )
        assert item["requested"] <= cap, "result read request exceeds framing budget"
        received += item["received"]


def test_result_framing_byte_count(invalid_result_frame):
    import worker_capture

    _, reads, _, _, mode = invalid_result_frame
    expected_bytes = {
        "oversized-header": 4,
        "fragmented-oversized-header": 4,
        "zero-header": 4,
        "one-header": 4,
        "short-header": 2,
        "short-body": 5,
        "trailing": 7,
        "overflow": 4 + worker_capture.FRAME_CAP + 1,
    }
    assert sum(item["received"] for item in reads) == expected_bytes[mode], (
        "result byte count exceeds framing boundary"
    )


@pytest.mark.parametrize("exact_cap", [False, True])
def test_result_frame_accepts_fragmented_delivery(tmp_path, monkeypatch, exact_cap):
    import worker_capture

    original_write = worker_capture._write

    def write(fd, data, deadline):
        if not data.endswith(b"\n"):
            if exact_cap:
                body = data[worker_capture.HEADER.size :]
                body += b" " * (worker_capture.FRAME_CAP - len(body))
                assert len(body) == worker_capture.FRAME_CAP
                data = worker_capture.HEADER.pack(len(body)) + body
            for fragment in (data[:1], data[1:4], data[4:17], data[17:]):
                original_write(fd, fragment, deadline)
                time.sleep(0.002)
        else:
            original_write(fd, data, deadline)

    monkeypatch.setattr(worker_capture, "_write", write)
    receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.6)
    retain(tmp_path, f"fragmented-frame-{exact_cap}", receipt)
    assert receipt.error is None and receipt.reaped and receipt.returncode == 0
    assert receipt.worker["reaped"] and not receipt.worker["helper_obligation_retained"]


def test_complete_result_frame_requires_control_eof(tmp_path, monkeypatch):
    import worker_capture

    original_write = worker_capture._write
    before = set(worker_capture._helper_obligations)

    def write(fd, data, deadline):
        original_write(fd, data, deadline)
        if not data.endswith(b"\n"):
            time.sleep(2)  # Complete terminal in pipe, both channels still open.

    monkeypatch.setattr(worker_capture, "_write", write)
    try:
        receipt = worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 0.3)
        retain(tmp_path, "complete-frame-open-control", receipt)
        assert receipt.error and not receipt.reaped
        assert receipt.worker["reaped"] and receipt.worker["helper_obligation_retained"]
        assert receipt.worker["published_times"] is None
    finally:
        for key in set(worker_capture._helper_obligations) - before:
            worker_capture._helper_obligations.pop(key)


def test_persistent_worker_reuses_exact_direct_child(tmp_path):
    import worker_capture

    worker = worker_capture.Worker()
    try:
        assert worker.start()
        pid = worker.owner.pid
        assert os.getpgid(pid) == pid
        receipts = [
            worker_capture.capture_helper(("/usr/bin/true",), time.monotonic() + 1, worker=worker)
            for _ in range(3)
        ]
        for index, receipt in enumerate(receipts):
            retain(tmp_path, f"persistent-reuse-{index}", receipt)
            assert receipt.error is None
            assert receipt.reaped and receipt.returncode == 0
            assert receipt.worker["pid"] == pid
            assert receipt.worker["reaped"] is False
        assert len({item.worker["request_token"] for item in receipts}) == 3
        assert len({item.worker["session_token"] for item in receipts}) == 1
        assert os.waitpid(pid, os.WNOHANG) == (0, 0)
    finally:
        worker.close()
    assert worker.owner.reaped
    assert worker.owner.returncode == 0
    with pytest.raises(ChildProcessError):
        os.waitpid(pid, os.WNOHANG)


def test_persistent_worker_failure_never_restarts(tmp_path, monkeypatch):
    import worker_capture

    worker = worker_capture.Worker()
    assert worker.start()
    pid = worker.owner.pid
    os.kill(pid, signal.SIGKILL)
    original_fork = os.fork
    forks = []

    def fork():
        forks.append(True)
        return original_fork()

    monkeypatch.setattr(os, "fork", fork)
    try:
        first = worker_capture.capture_helper(
            ("/usr/bin/true",), time.monotonic() + 0.3, worker=worker
        )
        second = worker_capture.capture_helper(
            ("/usr/bin/true",), time.monotonic() + 0.3, worker=worker
        )
        retain(tmp_path, "persistent-failure-first", first)
        retain(tmp_path, "persistent-failure-second", second)
        assert first.error and second.error
        assert worker.failure
        assert worker.start() is False
        assert not forks
        assert worker.owner.pid == pid
    finally:
        worker.close()
    assert worker.owner.reaped
    with pytest.raises(ChildProcessError):
        os.waitpid(pid, os.WNOHANG)


def test_persistent_worker_rejects_startup_admission(monkeypatch):
    import worker_capture

    calls = []
    original = inspection.fork_admission

    def deny():
        value = original()
        value.update(ok=False, reason="native_threads", native_threads=2)
        return value

    monkeypatch.setattr(inspection, "fork_admission", deny)
    monkeypatch.setattr(os, "fork", lambda: calls.append(True))
    worker = worker_capture.Worker()
    assert worker.start() is False
    assert worker.failure
    worker.close()
    assert not calls


@pytest.mark.parametrize("fault", ["silent", "identity", "late"])
def test_persistent_worker_startup_fault_is_bounded_and_reaped(tmp_path, monkeypatch, fault):
    import worker_capture

    def service(command, control, result, session, deadline):
        try:
            inspection.reset_after_fork({command, control, result})
            if fault == "identity":
                os.write(control, b'{"session":"wrong"}\n')
            elif fault == "late":
                time.sleep(max(0, deadline - time.monotonic()) + 0.02)
                os.write(control, b'{"session":"late"}\n')
            while True:
                signal.pause()
        finally:
            os._exit(70)

    monkeypatch.setattr(worker_capture, "_service", service)
    worker = worker_capture.Worker()
    assert worker.start() is False
    assert worker.failure and worker.closed
    assert worker.owner.reaped
    assert worker.startup_finished - worker.startup_started < 1.5
    path = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
    path.mkdir(parents=True, exist_ok=True)
    (path / f"persistent-startup-{fault}-{time.time_ns()}.json").write_text(
        json.dumps(worker.evidence(), indent=2)
    )
    with pytest.raises(ChildProcessError):
        os.waitpid(worker.owner.pid, os.WNOHANG)


def test_persistent_worker_rechecks_admission_after_preparation(monkeypatch):
    import worker_capture

    original = inspection.fork_admission
    snapshots = []
    forks = []

    def admission():
        value = original()
        snapshots.append(value)
        if len(snapshots) == 2:
            value.update(ok=False, reason="native_threads", native_threads=2)
        return value

    monkeypatch.setattr(inspection, "fork_admission", admission)
    monkeypatch.setattr(os, "fork", lambda: forks.append(True))
    worker = worker_capture.Worker()
    assert worker.start() is False
    assert worker.admission is snapshots[-1]
    assert len(snapshots) == 2 and not forks
    assert not worker.channels


@pytest.mark.parametrize(
    "mode",
    [
        "orderly",
        "term",
        "kill",
        "failure",
        "retiring-term",
        "retiring-kill",
        "expired",
        "expired-polled",
        "denied-term",
        "denied-kill",
        "command-failure",
    ],
)
def test_persistent_worker_close_has_one_fixed_grace(tmp_path, monkeypatch, mode):
    """Skipping TERM, repeating retirement or renewing authority must fail this control."""
    import worker_capture

    def service(command, control, result, session, deadline):
        try:
            inspection.reset_after_fork({command, control, result})
            if mode not in {"term", "expired"}:
                signal.signal(signal.SIGTERM, signal.SIG_IGN)
            identity = {
                "session": session,
                "pid": os.getpid(),
                "pgid": os.getpgrp(),
                "sid": os.getsid(0),
            }
            worker_capture._write(control, json.dumps(identity).encode() + b"\n", deadline)
            while True:
                signal.pause()
        finally:
            os._exit(70)

    if mode not in {"orderly", "expired-polled"}:
        monkeypatch.setattr(worker_capture, "_service", service)
    worker = worker_capture.Worker()
    assert worker.start()
    owner = worker.owner
    prior_deadline = None
    original_close = os.close
    command = worker.command
    close_faults = []
    exit_observed = False
    polls_before_close = 0
    last_poll_observed = None

    def close_command(fd):
        if fd == command:
            close_faults.append({"fd": fd, "monotonic": time.monotonic(), "errno": errno.EIO})
            raise OSError(errno.EIO, "controlled command close failure")
        return original_close(fd)

    audit = {"pid": owner.pid, "reaped": False, "source": "independent exact-child WNOHANG"}
    try:
        if mode == "expired-polled":
            receipt = worker_capture.capture_helper(
                ("/usr/bin/true",), time.monotonic() + 1, worker=worker
            )
            retain(tmp_path, "persistent-expired-polled-request", receipt)
            assert receipt.error is None and receipt.reaped and receipt.returncode == 0
            assert not owner.reaped and owner.reap_attempts > 0
            polls_before_close = owner.reap_attempts
            last_poll_observed = time.monotonic()
        if mode == "command-failure":
            monkeypatch.setattr(os, "close", close_command)
        if mode.startswith("denied-"):
            # A seeded failed attempt is history, not a claim of actual kernel denial.
            # The child ignores TERM; only the subsequent real KILL can reap it.
            owner.signals.append(
                {
                    "signal": "SIGTERM" if mode == "denied-term" else "SIGKILL",
                    "monotonic": time.monotonic(),
                    "errno": errno.EPERM,
                }
            )
            prior_deadline = time.monotonic() + 0.4
            owner.shutdown_deadline_monotonic = prior_deadline
        if mode == "failure":
            worker.failure = "controlled sticky failure"
        if mode.startswith("retiring") or mode in {"expired", "expired-polled"}:
            if mode in {"expired", "expired-polled"}:
                # Observe the real exit without consuming status. Register first so
                # close's final reap is guaranteed to encounter a waitable child.
                queue = select.kqueue()
                try:
                    event = select.kevent(
                        owner.pid,
                        filter=select.KQ_FILTER_PROC,
                        flags=select.KQ_EV_ADD,
                        fflags=select.KQ_NOTE_EXIT,
                    )
                    queue.control([event], 0, 0)
                    worker_capture._signal(owner, signal.SIGTERM)
                    events = queue.control(None, 1, 1)
                    exit_observed = any(item.fflags & select.KQ_NOTE_EXIT for item in events)
                    assert exit_observed
                finally:
                    queue.close()
            else:
                worker_capture._signal(
                    owner, signal.SIGKILL if mode == "retiring-kill" else signal.SIGTERM
                )
            prior_deadline = time.monotonic() + (
                0 if mode in {"expired", "expired-polled"} else 0.4
            )
            owner.shutdown_deadline_monotonic = prior_deadline
        worker.close()
        first = worker.evidence()
        worker.close()
        assert worker.evidence() == first
        if prior_deadline is not None:
            assert owner.shutdown_deadline_monotonic == prior_deadline
        signals = [item["signal"] for item in owner.signals]
        if mode == "orderly":
            assert owner.reaped and owner.returncode == 0 and not signals
            assert worker.failure is None
        elif mode == "expired":
            assert signals == ["SIGTERM"]
            assert owner.reaped and owner.returncode == -signal.SIGTERM
            assert owner.pid not in worker_capture._owners and not first["residual_remedy"]
            assert first["shutdown"]["overrun"] and worker.failure
            assert first["owner"]["reap_attempts"] == 1
        elif mode == "expired-polled":
            assert signals == ["SIGTERM"]
            assert last_poll_observed < owner.signals[0]["monotonic"] < prior_deadline
            assert not owner.reaped and owner.pid in worker_capture._owners
            assert first["residual_remedy"] and worker.failure
            assert first["shutdown"]["overrun"]
            assert first["owner"]["reap_attempts"] == polls_before_close
        elif mode == "denied-kill":
            assert signals == ["SIGKILL"] and owner.signals[0]["errno"] == errno.EPERM
            assert not owner.reaped and owner.pid in worker_capture._owners
            assert first["residual_remedy"]
        else:
            assert owner.reaped
            assert owner.returncode == -signal.SIGKILL
            assert signals == (
                ["SIGKILL"] if mode in {"retiring-kill", "term", "kill"} else ["SIGTERM", "SIGKILL"]
            )
            assert worker.failure
            if mode in {"term", "kill"}:
                assert 0.04 <= owner.signals[0]["monotonic"] - worker.shutdown_started < 0.5
            if mode == "failure":
                assert owner.signals[0]["monotonic"] - worker.shutdown_started < 0.1
            if mode == "command-failure":
                assert len(close_faults) == 1
                assert close_faults[0]["fd"] == command and close_faults[0]["errno"] == errno.EIO
                assert "worker command close failed" in worker.failure
                assert first["shutdown"]["mode"] == "failure"
                assert owner.signals[0]["monotonic"] - worker.shutdown_started < 0.04
                assert 1.9 <= owner.signals[1]["monotonic"] - worker.shutdown_started <= 2.05
            if mode == "denied-term":
                assert owner.signals[0]["errno"] == errno.EPERM
                assert 0 <= owner.signals[1]["monotonic"] - worker.shutdown_started < 0.04
            if len(signals) == 2:
                assert 0 <= owner.signals[1]["monotonic"] - owner.signals[0]["monotonic"] <= 2.05
                assert 0 <= worker.shutdown_finished - owner.signals[1]["monotonic"] < 2
            assert all(
                item["monotonic"] < owner.shutdown_deadline_monotonic for item in owner.signals
            )
    finally:
        monkeypatch.setattr(os, "close", original_close)
        if mode == "command-failure":
            original_close(command)  # Fault left this fixture-owned descriptor open.
        # Evidence is captured before independent cleanup changes the owner ledger.
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"persistent-close-{mode}-{time.time_ns()}.json").write_text(
            json.dumps(
                {
                    "runtime": worker.evidence(),
                    "close_faults": close_faults,
                    "exit_observed_without_reap": exit_observed,
                    "polls_before_close": polls_before_close,
                    "last_poll_observed": last_poll_observed,
                },
                indent=2,
            )
            + "\n"
        )
        if owner.reaped:
            with pytest.raises(ChildProcessError):
                os.waitpid(owner.pid, os.WNOHANG)
            audit["reaped"] = True
        else:
            # The unreaped exact direct child pins this fixture identity.
            if not exit_observed:
                os.killpg(owner.pid, signal.SIGKILL)
            end = time.monotonic() + 1
            while time.monotonic() < end:
                found, status = os.waitpid(owner.pid, os.WNOHANG)
                if found == owner.pid:
                    audit.update(reaped=True, status=status)
                    if mode in {"expired", "expired-polled"}:
                        assert os.waitstatus_to_exitcode(status) == -signal.SIGTERM
                    worker_capture._owners.pop(owner.pid, None)
                    break
                time.sleep(0.001)
        (root / f"persistent-close-audit-{mode}-{time.time_ns()}.json").write_text(
            json.dumps(audit, indent=2) + "\n"
        )
        assert audit["reaped"]


def test_persistent_worker_close_before_ready_uses_pid(tmp_path, monkeypatch):
    """A TERM-ignoring child with no ready identity is owned only by exact PID."""
    import worker_capture

    read_fd, write_fd = os.pipe()
    pid = os.fork()
    if pid == 0:
        try:
            inspection.reset_after_fork({write_fd}, new_session=False)
            signal.signal(signal.SIGTERM, signal.SIG_IGN)
            os.write(write_fd, b"R")  # Fixture synchronization, never a ready message.
            while True:
                signal.pause()
        finally:
            os._exit(70)
    os.close(write_fd)
    owner = worker_capture._Owner(pid, float("inf"))
    worker_capture._owners[pid] = owner
    worker = worker_capture.Worker(owner=owner)
    calls = []
    original_kill, original_killpg = os.kill, os.killpg
    audit = {"pid": pid, "reaped": False}

    def kill(target, sig):
        calls.append({"scope": "pid", "target": target, "signal": int(sig)})
        return original_kill(target, sig)

    def killpg(target, sig):
        calls.append({"scope": "group", "target": target, "signal": int(sig)})
        return original_killpg(target, sig)

    try:
        readable, _, _ = select.select([read_fd], [], [], 1)
        assert readable and os.read(read_fd, 1) == b"R"
        monkeypatch.setattr(os, "kill", kill)
        monkeypatch.setattr(os, "killpg", killpg)
        worker.close()
        first = worker.evidence()
        worker.close()
        assert worker.evidence() == first
        assert not owner.ready
        assert calls == [
            {"scope": "pid", "target": pid, "signal": signal.SIGTERM},
            {"scope": "pid", "target": pid, "signal": signal.SIGKILL},
        ]
        assert owner.reaped and owner.returncode == -signal.SIGKILL
        assert 0 <= owner.signals[0]["monotonic"] - worker.shutdown_started < 0.04
        assert 1.9 <= owner.signals[1]["monotonic"] - worker.shutdown_started <= 2.05
        assert all(item["monotonic"] < owner.shutdown_deadline_monotonic for item in owner.signals)
    finally:
        os.close(read_fd)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        runtime = worker.evidence()
        if owner.reaped:
            with pytest.raises(ChildProcessError):
                os.waitpid(pid, os.WNOHANG)
            audit["reaped"] = True
        else:
            original_kill(pid, signal.SIGKILL)
            end = time.monotonic() + 1
            while time.monotonic() < end:
                found, status = os.waitpid(pid, os.WNOHANG)
                if found == pid:
                    audit.update(reaped=True, status=status)
                    worker_capture._owners.pop(pid, None)
                    break
                time.sleep(0.001)
        (root / f"persistent-close-pid-{time.time_ns()}.json").write_text(
            json.dumps({"runtime": runtime, "calls": calls, "audit": audit}, indent=2) + "\n"
        )
        assert audit["reaped"]


@pytest.mark.parametrize("fault", ["missing", "duplicate", "token"])
def test_persistent_worker_rejects_invalid_completion(tmp_path, monkeypatch, fault):
    import worker_capture

    original = worker_capture._write

    def write(fd, data, deadline):
        if data.startswith(b'{"complete":'):
            if fault == "missing":
                return
            if fault == "duplicate":
                data += data
            else:
                data = b'{"complete":"wrong"}\n'
        original(fd, data, deadline)

    monkeypatch.setattr(worker_capture, "_write", write)
    worker = worker_capture.Worker()
    assert worker.start()
    try:
        receipt = worker_capture.capture_helper(
            ("/usr/bin/true",), time.monotonic() + 0.3, worker=worker
        )
        retain(tmp_path, f"persistent-completion-{fault}", receipt)
        assert receipt.error and worker.failure
        assert worker.closed and worker.owner.reaped
        assert receipt.worker["helper_obligation_retained"]
        assert receipt.worker["helper_identity"] is not None
    finally:
        worker.close()
    with pytest.raises(ChildProcessError):
        os.waitpid(worker.owner.pid, os.WNOHANG)


def test_persistent_worker_expired_request_is_sticky(tmp_path):
    import worker_capture

    worker = worker_capture.Worker()
    assert worker.start()
    pid = worker.owner.pid
    try:
        receipt = worker_capture.capture_helper(
            ("/usr/bin/true",), time.monotonic() - 0.01, worker=worker
        )
        retain(tmp_path, "persistent-expired-request", receipt)
        assert receipt.error and worker.failure and worker.closed
        assert worker.evidence()["residual_remedy"]
        assert worker.start() is False
        second = worker_capture.capture_helper(
            ("/usr/bin/true",), time.monotonic() + 0.5, worker=worker
        )
        assert second.error and second.pid is None and second.worker is None
    finally:
        worker.close()
        # The request had no remaining ownership allowance. Independently reconcile
        # this exact still-owned child after command EOF; never extend runtime authority.
        if not worker.owner.reaped:
            end = time.monotonic() + 1
            while time.monotonic() < end:
                waited, status = os.waitpid(pid, os.WNOHANG)
                if waited == pid:
                    worker.owner.reaped = True
                    worker.owner.returncode = os.waitstatus_to_exitcode(status)
                    worker_capture._owners.pop(pid, None)
                    break
                time.sleep(0.001)
        assert worker.owner.reaped
        path = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        path.mkdir(parents=True, exist_ok=True)
        (path / f"persistent-expired-independent-{time.time_ns()}.json").write_text(
            json.dumps(worker.evidence(), indent=2)
        )


@pytest.mark.parametrize(
    "message", ["command", "ready", "intent", "created", "fork-error", "complete", "result"]
)
@pytest.mark.parametrize(
    "damage",
    [
        "run_id",
        "worker_token",
        "worker_pid",
        "request_token",
        "workload_pid",
        "workload_pgid",
        "signal_epoch",
        "phase",
        "deadline_monotonic",
        "missing",
        "boolean",
    ],
)
def test_request_identity_rejects_changed_envelope(tmp_path, monkeypatch, message, damage):
    """Alter actual pipe traffic after real helper reap; no fabricated death receipt."""
    import worker_capture

    original = worker_capture._write
    parent = os.getpid()
    pending = []
    result_sent = False
    applied = tmp_path / "identity-mutation-applied"

    def write(fd, data, deadline):
        nonlocal result_sent
        # Startup identity predates requests and retains its own session checks.
        framed = data[:1] != b"{"
        if os.getpid() != parent and data.startswith(b'{"session":'):
            return original(fd, data, deadline)
        if os.getpid() == parent:
            kind = "command"
            value = json.loads(data)
        elif framed:
            kind = "result"
            value = json.loads(data[worker_capture.HEADER.size :])
        else:
            value = json.loads(data)
            kind = (
                "ready"
                if "sid" in value
                else "complete"
                if "complete" in value
                else "fork-error"
                if "errno" in value
                else "created"
                if "ppid" in value
                else "intent"
            )
        if kind == message:
            applied.write_text(kind)
            identity = value["identity"]
            if damage == "missing":
                value.pop("identity")
            elif damage == "boolean":
                identity["signal_epoch"] = True
            elif isinstance(identity[damage], str):
                identity[damage] += "-stale"
            else:
                identity[damage] += 1
        encoded = json.dumps(value).encode()
        data = (
            worker_capture.HEADER.pack(len(encoded)) + encoded
            if kind == "result"
            else encoded + b"\n"
        )
        if os.getpid() != parent and kind != "result" and not result_sent:
            pending.append((fd, data))
            return
        if kind == "result":
            # All helpers are really reaped before any damaged control reaches
            # the controller. Pipe rejection cannot strand a live helper here.
            for channel, buffered in pending:
                original(channel, buffered, deadline)
            pending.clear()
            result_sent = True
        original(fd, data, deadline)

    monkeypatch.setattr(worker_capture, "_write", write)
    if message == "fork-error":
        publish = worker_capture._publish

        def fail_fork(*args, **kwargs):
            def denied():
                raise OSError(errno.EAGAIN, "injected fork admission race")

            monkeypatch.setattr(os, "fork", denied)
            publish(*args, **kwargs)

        monkeypatch.setattr(worker_capture, "_publish", fail_fork)
    worker = worker_capture.Worker(run_id="identity-run")
    before = set(worker_capture._helper_obligations)
    try:
        assert worker.start()
        receipt = worker_capture.capture_helper(
            ("/usr/bin/true",),
            time.monotonic() + 0.5,
            worker=worker,
            scope=worker_capture.RequestScope(parent, os.getpgrp(), 1, "final"),
        )
        retain(tmp_path, f"request-identity-{message}-{damage}", receipt)
        assert applied.read_text() == message
        assert receipt.error and worker.failure and worker.closed
        assert receipt.worker["published_times"] is None
        if message in {"result", "complete"}:
            assert receipt.worker["helper_obligation_retained"]
            assert receipt.worker["helper_identity"] is not None
        elif message in {"ready", "intent", "created", "fork-error"}:
            assert receipt.worker["helper_identity"] is None
            assert receipt.worker["unknown_helper_creation"] is True
            assert receipt.worker["creation_conflict"] is True
            assert receipt.residual_remedy
        assert worker.start() is False
    finally:
        worker.close()
        assert worker.owner.reaped
        with pytest.raises(ChildProcessError):
            os.waitpid(worker.owner.pid, os.WNOHANG)
        # Test-local evidence is conservative; release only this test's ledger
        # entries after independently confirming the exact published PID is gone.
        audit = []
        for key in set(worker_capture._helper_obligations) - before:
            identity = worker_capture._helper_obligations[key]
            with pytest.raises(ProcessLookupError):
                os.kill(identity["pid"], 0)
            audit.append({"pid": identity["pid"], "absent": True})
            worker_capture._helper_obligations.pop(key)
        path = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        path.mkdir(parents=True, exist_ok=True)
        (path / f"identity-audit-{message}-{damage}-{time.time_ns()}.json").write_text(
            json.dumps(
                {"worker_pid": worker.owner.pid, "worker_reaped": True, "helper_absence": audit},
                indent=2,
            )
        )


@pytest.mark.parametrize("second", ["replay", "changed-run", "fresh"])
def test_request_identity_worker_second_command(tmp_path, monkeypatch, second):
    """Challenge worker admission directly after a genuinely completed request."""
    import worker_capture

    publish = worker_capture._publish
    publications = 0
    admitted = tmp_path / "second-command-admitted"

    def publish_once(*args, **kwargs):
        nonlocal publications
        publications += 1
        if publications == 1:
            return publish(*args, **kwargs)
        # An admitted second command needs no helper for this admission test.
        # This also keeps a deliberately weakened worker from creating a child.
        admitted.write_text("admitted")

    monkeypatch.setattr(worker_capture, "_publish", publish_once)
    worker = worker_capture.Worker(run_id="pinned-run")
    first = None
    command = None
    try:
        assert worker.start()
        first = worker_capture.capture_helper(
            ("/usr/bin/true",),
            time.monotonic() + 1,
            worker=worker,
            scope=worker_capture.RequestScope(os.getpid(), os.getpgrp(), 0, "final"),
        )
        retain(tmp_path, f"request-identity-second-{second}-first", first)
        assert first.error is None and first.reaped
        assert not first.worker["helper_obligation_retained"]
        identity = dict(first.worker["request_identity"])
        identity["deadline_monotonic"] = time.monotonic() + 1
        identity["signal_epoch"] = 1
        if second != "replay":
            identity["request_token"] += "-fresh"
        if second == "changed-run":
            identity["run_id"] = "different-run"
        command = {
            "session": worker.session_token,
            "token": identity["request_token"],
            "argv": ["/usr/bin/true"],
            "deadline": identity["deadline_monotonic"],
            "identity": identity,
        }
        worker_capture._write(
            worker.command,
            json.dumps(command).encode() + b"\n",
            identity["deadline_monotonic"],
        )
    finally:
        worker.close()
        assert worker.owner.reaped
        with pytest.raises(ChildProcessError):
            os.waitpid(worker.owner.pid, os.WNOHANG)
        if first is not None and first.pid is not None:
            with pytest.raises(ProcessLookupError):
                os.kill(first.pid, 0)
        path = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        path.mkdir(parents=True, exist_ok=True)
        (path / f"identity-second-{second}-{time.time_ns()}.json").write_text(
            json.dumps(
                {
                    "command": command,
                    "worker": worker.evidence(),
                    "worker_reaped": True,
                    "helper_pid": first.pid if first else None,
                    "helper_absent": first is not None and first.pid is not None,
                    "second_admitted": admitted.exists(),
                    "second_helper_creation": False,
                },
                indent=2,
            )
        )
    assert worker.owner.returncode == (0 if second == "fresh" else 70)
    assert admitted.exists() is (second == "fresh")


def test_request_identity_valid_sequential_scopes(tmp_path):
    import worker_capture

    worker = worker_capture.Worker(run_id="matching-run")
    scopes = [
        worker_capture.RequestScope(os.getpid(), os.getpgrp(), index, "final") for index in range(3)
    ]
    try:
        assert worker.start()
        pid = worker.owner.pid
        for index, scope in enumerate(scopes):
            receipt = worker_capture.capture_helper(
                ("/usr/bin/true",),
                time.monotonic() + 1,
                worker=worker,
                scope=scope,
            )
            retain(tmp_path, f"request-identity-valid-{index}", receipt)
            assert receipt.error is None and receipt.reaped
            identity = receipt.worker["request_identity"]
            assert identity["run_id"] == "matching-run"
            assert identity["worker_token"] == worker.session_token
            assert identity["worker_pid"] == pid
            assert identity["workload_pid"] == os.getpid()
            assert identity["workload_pgid"] == os.getpgrp()
            assert identity["signal_epoch"] == index
            assert identity["phase"] == "final"
            assert identity["request_token"] == receipt.worker["request_token"]
            assert identity["deadline_monotonic"] == receipt.worker["query_deadline_monotonic"]
            assert not receipt.worker["helper_obligation_retained"]
    finally:
        worker.close()
    assert worker.owner.reaped and worker.owner.returncode == 0


def test_request_identity_matching_explicit_fork_failure(tmp_path, monkeypatch):
    import worker_capture

    publish = worker_capture._publish

    def failed_publish(*args, **kwargs):
        def denied():
            raise OSError(errno.EAGAIN, "injected fork admission race")

        monkeypatch.setattr(os, "fork", denied)
        publish(*args, **kwargs)

    monkeypatch.setattr(worker_capture, "_publish", failed_publish)
    worker = worker_capture.Worker(run_id="fork-failure-run")
    try:
        assert worker.start()
        receipt = worker_capture.capture_helper(
            ("/usr/bin/true",),
            time.monotonic() + 0.5,
            worker=worker,
            scope=worker_capture.RequestScope(os.getpid(), os.getpgrp(), 0, "final"),
        )
        retain(tmp_path, "request-identity-matching-fork-failure", receipt)
        assert receipt.error and worker.failure
        assert receipt.worker["unknown_helper_creation"] is False
        assert receipt.worker["creation_conflict"] is False
        assert receipt.worker["fork_failure"]["errno"] == errno.EAGAIN
        assert receipt.worker["helper_identity"] is None
        assert receipt.worker["helper_obligation_retained"] is False
    finally:
        worker.close()
        assert worker.owner.reaped
        with pytest.raises(ChildProcessError):
            os.waitpid(worker.owner.pid, os.WNOHANG)
