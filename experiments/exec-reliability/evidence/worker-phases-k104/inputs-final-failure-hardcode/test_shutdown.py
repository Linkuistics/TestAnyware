"""Declared shutdown phases, with independent cleanup of exact fixture children."""

import errno
import json
import os
import signal
import time
import uuid
from dataclasses import asdict
from pathlib import Path

import pytest

import controller
import inspection
import worker_capture


@pytest.mark.parametrize(
    "kind,phase",
    [
        ("failure", "term_grace"),
        ("failure", "kill_reap"),
        ("close", "grace"),
        ("close", "kill_reap"),
    ],
)
@pytest.mark.parametrize("invalid", [0, -1, 2.01, float("nan"), float("inf"), True, None, "0.1"])
def test_shutdown_bounds_rejected_before_freeze_or_fork(
    tmp_path, monkeypatch, kind, phase, invalid
):
    cls = worker_capture.FailureBounds if kind == "failure" else worker_capture.CloseBounds
    bounds = cls(**{phase: invalid})
    options = {f"{kind}_bounds": bounds}

    def forbidden(*args, **kwargs):
        raise AssertionError("invalid shutdown bound reached freeze/fork")

    monkeypatch.setattr(controller, "_hashes", forbidden)
    monkeypatch.setattr(os, "fork", forbidden)
    with pytest.raises(ValueError, match="bounds|failure|close"):
        controller.execute(controller.Plan(uuid.uuid4().hex, (), **options), tmp_path / "run")
    assert not (tmp_path / "run").exists()
    worker = worker_capture.Worker(**options)
    with pytest.raises(ValueError, match="bounds|failure|close"):
        worker.start()
    assert worker.owner is None and not worker.channels


@pytest.mark.parametrize("kind", ["failure", "close"])
@pytest.mark.parametrize("invalid", [None, {}, object(), worker_capture.BootstrapBounds()])
def test_shutdown_bounds_object_rejected_before_freeze_or_fork(
    tmp_path, monkeypatch, kind, invalid
):
    options = {f"{kind}_bounds": invalid}

    def forbidden(*args, **kwargs):
        raise AssertionError("invalid bounds object reached freeze/fork")

    monkeypatch.setattr(controller, "_hashes", forbidden)
    monkeypatch.setattr(os, "fork", forbidden)
    with pytest.raises(ValueError, match="bounds"):
        controller.execute(controller.Plan(uuid.uuid4().hex, (), **options), tmp_path / "run")
    assert not (tmp_path / "run").exists()
    worker = worker_capture.Worker(**options)
    with pytest.raises(ValueError, match="bounds"):
        worker.start()
    assert worker.owner is None and not worker.channels


@pytest.mark.parametrize(
    "mode",
    [
        "orderly",
        "healthy-stall",
        "failure-term",
        "failure-kill",
        "deferred-reap",
        "overrun",
        "prior-term",
        "denied-term",
        "prior-kill",
        "denied-kill",
        "finite",
        "expired",
    ],
)
def test_declared_shutdown_real_child_phases(tmp_path, monkeypatch, mode):
    """Real signals/reap; deferred and late wait returns are explicitly simulated."""
    original_service = worker_capture._service
    original_waitpid = os.waitpid
    original_kill, original_killpg = os.kill, os.killpg
    failure = worker_capture.FailureBounds(term_grace=0.07, kill_reap=0.16)
    close = worker_capture.CloseBounds(grace=0.04, kill_reap=0.13)
    worker = worker_capture.Worker(failure_bounds=failure, close_bounds=close)
    calls, probes = [], []
    injected = False

    def service(command, control, result, session, deadline):
        if mode == "orderly":
            original_service(command, control, result, session, deadline)
        try:
            inspection.reset_after_fork({command, control, result})
            if mode != "failure-term":
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

    def waitpid(pid, flags):
        nonlocal injected
        assert flags == os.WNOHANG
        if worker.owner is not None and pid == worker.owner.pid:
            probes.append(time.monotonic())
            if mode == "deferred-reap":
                return 0, 0
            if mode == "overrun" and not injected:
                injected = True
                time.sleep(0.27)
        return original_waitpid(pid, flags)

    def killpg(pid, sig):
        calls.append({"pid": pid, "signal": sig.name, "monotonic": time.monotonic()})
        return original_killpg(pid, sig)

    monkeypatch.setattr(worker_capture, "_service", service)
    audit = {"reaped": False, "source": "original exact-child WNOHANG", "mode": mode}
    runtime = None
    try:
        assert worker.start()
        owner = worker.owner
        assert owner is not None
        seeded = []
        old_end = float("inf")
        if mode.startswith(("prior-", "denied-")):
            sig = signal.SIGTERM if mode.endswith("term") else signal.SIGKILL
            if mode.startswith("prior-"):
                worker_capture._signal(owner, sig)
            else:
                # Failed-attempt history only, never evidence of actual UID EPERM.
                owner.signals.append(
                    {"signal": sig.name, "monotonic": time.monotonic(), "errno": errno.EPERM}
                )
            seeded = list(owner.signals)
            old_end = time.monotonic() + 0.11
        if mode in {"finite", "expired"}:
            old_end = time.monotonic() + (0.1 if mode == "finite" else -0.001)
        owner.shutdown_deadline_monotonic = old_end
        if mode != "orderly" and mode != "healthy-stall":
            worker.failure = "controlled sticky failure"
        monkeypatch.setattr(os, "waitpid", waitpid)
        monkeypatch.setattr(os, "killpg", killpg)
        worker.close()
        runtime = worker.evidence()
        worker.close()
        assert worker.evidence() == runtime
        assert runtime["failure_bounds"] == asdict(failure)
        assert runtime["close_bounds"] == asdict(close)
        receipt = runtime["shutdown"]
        end = owner.shutdown_deadline_monotonic
        start = worker.shutdown_started
        assert end <= old_end
        assert owner.signals[: len(seeded)] == seeded
        signals = [item["signal"] for item in owner.signals]
        if mode == "orderly":
            assert owner.reaped and owner.returncode == 0 and not signals
            assert receipt["mode"] == "orderly"
            assert worker.failure is None
        elif mode == "healthy-stall":
            assert signals == ["SIGKILL"] and owner.reaped
            assert receipt["mode"] == "orderly"
            assert worker.kill_at == start + close.grace
            assert end == worker.kill_at + close.kill_reap
            assert owner.signals[0]["monotonic"] >= worker.kill_at
            assert owner.signals[0]["monotonic"] - worker.kill_at < 0.04
        elif mode == "failure-term":
            assert signals == ["SIGTERM"] and owner.reaped
            assert owner.returncode == -signal.SIGTERM
        elif mode in {"failure-kill", "deferred-reap"}:
            assert signals == ["SIGTERM", "SIGKILL"]
            assert worker.kill_at == start + failure.term_grace
            assert end == worker.kill_at + failure.kill_reap
            assert owner.signals[1]["monotonic"] >= worker.kill_at
            assert owner.signals[1]["monotonic"] - worker.kill_at < 0.04
            assert owner.reaped is (mode != "deferred-reap")
        elif mode in {"prior-term", "denied-term"}:
            assert signals == ["SIGTERM", "SIGKILL"] and owner.reaped
            assert worker.kill_at <= seeded[0]["monotonic"] + failure.term_grace
            assert end == old_end
        elif mode == "prior-kill":
            assert signals == ["SIGKILL"] and owner.reaped
        elif mode == "denied-kill":
            assert signals == ["SIGKILL"] and not owner.reaped
            assert owner.pid in worker_capture._owners
        elif mode == "finite":
            assert signals == ["SIGTERM", "SIGKILL"] and owner.reaped
            assert end == old_end
        elif mode == "expired":
            assert not signals and not probes and not owner.reaped
        elif mode == "overrun":
            assert not signals and injected
            assert receipt["phases"]["kill_reap"]["started_monotonic"] is None
            assert receipt["phases"]["kill_reap"]["outcome"] == "not_entered"
            assert receipt["phases"]["grace"]["outcome"] == "residual_ownership"
        if mode in {"prior-kill", "denied-kill"}:
            assert receipt["phases"]["grace"]["started_monotonic"] is None
            assert receipt["phases"]["grace"]["outcome"] == "already_retiring"
            assert receipt["phases"]["kill_reap"]["started_monotonic"] == seeded[0]["monotonic"]
        assert receipt["kill_deadline_monotonic"] == end
        assert receipt["term_deadline_monotonic"] == worker.kill_at
        assert receipt["finished_monotonic"] == worker.shutdown_finished
        assert receipt["overrun"] is (worker.shutdown_finished > end)
        assert receipt["lateness_seconds"] == max(0, worker.shutdown_finished - end)
        assert receipt["deadline_exhausted"] is (worker.shutdown_finished >= end)
        assert receipt["outcome"] == ("reaped" if owner.reaped else "residual_ownership")
        assert receipt["signals"] == owner.signals
        for phase in receipt["phases"].values():
            assert phase["overrun"] is (phase["finished_monotonic"] > phase["deadline_monotonic"])
            assert phase["lateness_seconds"] == max(
                0, phase["finished_monotonic"] - phase["deadline_monotonic"]
            )
            assert phase["outcome"]
        assert all(item["monotonic"] < end for item in calls)
        assert all(item < end for item in probes)
        if mode != "orderly":
            assert worker.failure
    finally:
        monkeypatch.setattr(os, "waitpid", original_waitpid)
        monkeypatch.setattr(os, "killpg", original_killpg)
        owner = worker.owner
        if owner is not None:
            if owner.reaped:
                with pytest.raises(ChildProcessError):
                    original_waitpid(owner.pid, os.WNOHANG)
                audit["reaped"] = True
            else:
                # Still an unreaped direct child: fixture identity remains pinned.
                original_kill(owner.pid, signal.SIGKILL)
                deadline = time.monotonic() + 1
                while time.monotonic() < deadline:
                    found, status = original_waitpid(owner.pid, os.WNOHANG)
                    if found == owner.pid:
                        audit.update(reaped=True, status=status)
                        worker_capture._owners.pop(owner.pid, None)
                        break
                    time.sleep(0.001)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"shutdown-{mode}-{time.time_ns()}.json").write_text(
            json.dumps(
                {"runtime": runtime, "calls": calls, "probes": probes, "audit": audit}, indent=2
            )
            + "\n"
        )
        assert audit["reaped"]


def test_plan_propagates_declared_shutdown_bounds(tmp_path):
    failure = worker_capture.FailureBounds(term_grace=0.08, kill_reap=0.17)
    close = worker_capture.CloseBounds(grace=0.03, kill_reap=0.14)
    plan = controller.Plan(uuid.uuid4().hex, (), failure_bounds=failure, close_bounds=close)
    path = controller.execute(plan, tmp_path / "plan")
    result = json.loads((path / "result.json").read_text())
    declaration = json.loads((path / "plan.json").read_text())
    assert declaration["failure_bounds"] == asdict(failure)
    assert declaration["close_bounds"] == asdict(close)
    assert result["inspection_worker"]["failure_bounds"] == asdict(failure)
    assert result["inspection_worker"]["close_bounds"] == asdict(close)
