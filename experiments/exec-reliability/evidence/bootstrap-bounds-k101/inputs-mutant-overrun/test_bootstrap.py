"""Declared preparation bounds exercised against exact owned local children."""

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


@pytest.mark.parametrize("phase,ceiling", [("readiness", 1), ("term_grace", 2), ("kill_reap", 2)])
@pytest.mark.parametrize("invalid", [0, -1, float("nan"), float("inf"), True, None, "0.1", "over"])
def test_bootstrap_bounds_rejected_before_freeze_or_fork(
    tmp_path, monkeypatch, phase, ceiling, invalid
):
    """Removing validation must permit a side effect or an invalid worker start."""
    values = {phase: ceiling + 0.01 if invalid == "over" else invalid}
    bounds = worker_capture.BootstrapBounds(**values)
    plan = controller.Plan(uuid.uuid4().hex, (), bootstrap=bounds)

    def forbidden(*args, **kwargs):
        raise AssertionError("invalid bootstrap bound reached freeze/fork")

    monkeypatch.setattr(controller, "_hashes", forbidden)
    monkeypatch.setattr(os, "fork", forbidden)
    with pytest.raises(ValueError, match="bootstrap"):
        controller.execute(plan, tmp_path / "evidence")
    assert not (tmp_path / "evidence").exists()
    worker = worker_capture.Worker(bootstrap=bounds)
    with pytest.raises(ValueError, match="bootstrap"):
        worker.start()
    assert worker.owner is None and not worker.channels


@pytest.mark.parametrize("mode", ["ready", "term", "kill", "deferred-reap", "overrun"])
def test_declared_bootstrap_real_child_bounds(tmp_path, monkeypatch, mode):
    """Hard-coded readiness/TERM/KILL budgets or hiding an overrun must fail."""
    bounds = worker_capture.BootstrapBounds(readiness=0.15, term_grace=0.07, kill_reap=0.16)
    original_service = worker_capture._service
    original_waitpid = os.waitpid
    original_kill, original_killpg = os.kill, os.killpg
    calls = []
    injected = False

    def service(command, control, result, session, deadline):
        try:
            if mode == "ready":
                time.sleep(0.02)
                original_service(command, control, result, session, deadline)
            inspection.reset_after_fork({command, control, result})
            if mode != "term":
                signal.signal(signal.SIGTERM, signal.SIG_IGN)
            while True:
                signal.pause()
        finally:
            os._exit(70)

    worker = worker_capture.Worker(bootstrap=bounds)

    def waitpid(pid, flags):
        nonlocal injected
        assert flags == os.WNOHANG
        if worker.owner is not None and pid == worker.owner.pid:
            if mode == "deferred-reap":
                return 0, 0  # Simulated observation delay; the real child is killable.
            if mode == "overrun" and worker.kill_at is not None and not injected:
                injected = True
                time.sleep(0.27)  # Simulated syscall-return stall, never a renewed allowance.
        return original_waitpid(pid, flags)

    def kill(pid, sig):
        calls.append({"scope": "pid", "pid": pid, "signal": int(sig), "time": time.monotonic()})
        return original_kill(pid, sig)

    def killpg(pid, sig):
        calls.append({"scope": "group", "pid": pid, "signal": int(sig), "time": time.monotonic()})
        return original_killpg(pid, sig)

    monkeypatch.setattr(worker_capture, "_service", service)
    monkeypatch.setattr(os, "waitpid", waitpid)
    monkeypatch.setattr(os, "kill", kill)
    monkeypatch.setattr(os, "killpg", killpg)
    audit = {"reaped": False, "mode": mode, "independent": "original exact-child WNOHANG"}
    runtime = None
    try:
        assert worker.start() is (mode == "ready")
        worker.close()
        runtime = worker.evidence()
        assert runtime["bootstrap_bounds"] == {
            "readiness": 0.15,
            "term_grace": 0.07,
            "kill_reap": 0.16,
        }
        owner = worker.owner
        assert owner is not None
        audit["pid"] = owner.pid
        if mode == "ready":
            assert worker.failure is None and owner.reaped and owner.returncode == 0
            assert runtime["bootstrap_shutdown"] is None and not calls
        else:
            assert worker.failure and worker.closed
            phase = runtime["bootstrap_shutdown"]
            assert phase["term_deadline_monotonic"] == pytest.approx(worker.shutdown_started + 0.07)
            assert phase["kill_deadline_monotonic"] == pytest.approx(worker.shutdown_started + 0.23)
            assert all(item["scope"] == "pid" and item["pid"] == owner.pid for item in calls)
            assert all(item["time"] < owner.shutdown_deadline_monotonic for item in calls)
            if mode == "term":
                assert [item["signal"] for item in calls] == [signal.SIGTERM]
                assert owner.reaped and owner.returncode == -signal.SIGTERM
            elif mode == "overrun":
                assert injected and not calls and not owner.reaped
                assert phase["overrun"] is True and runtime["residual_remedy"]
            else:
                assert [item["signal"] for item in calls] == [signal.SIGTERM, signal.SIGKILL]
                assert 0.065 <= calls[1]["time"] - worker.shutdown_started < 0.16
                if mode == "kill":
                    assert owner.reaped and owner.returncode == -signal.SIGKILL
                else:
                    assert not owner.reaped and runtime["residual_remedy"]
                    assert owner.pid in worker_capture._owners
            assert worker.startup_finished - worker.startup_started < 0.75
        first = worker.evidence()
        worker.close()
        assert not worker.start() and worker.evidence() == first
    finally:
        # This checker bypasses the deliberately faulted runtime adapter. A still
        # unreaped direct child pins the exact PID until this independent audit.
        monkeypatch.setattr(os, "waitpid", original_waitpid)
        if worker.owner is not None:
            owner = worker.owner
            if owner.reaped:
                with pytest.raises(ChildProcessError):
                    original_waitpid(owner.pid, os.WNOHANG)
                audit["reaped"] = True
            else:
                original_kill(owner.pid, signal.SIGKILL)
                end = time.monotonic() + 1
                while time.monotonic() < end:
                    found, status = original_waitpid(owner.pid, os.WNOHANG)
                    if found == owner.pid:
                        audit.update(reaped=True, status=status)
                        worker_capture._owners.pop(owner.pid, None)
                        break
                    time.sleep(0.001)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"bootstrap-{mode}-{time.time_ns()}.json").write_text(
            json.dumps(
                {"runtime": runtime or worker.evidence(), "signals": calls, "audit": audit},
                indent=2,
            )
            + "\n"
        )
        assert audit["reaped"]


def test_declared_bootstrap_plan_reaches_real_worker(tmp_path):
    """Dropping Plan's bounds when constructing the worker loses the declaration."""
    bounds = worker_capture.BootstrapBounds(readiness=0.4, term_grace=0.05, kill_reap=0.1)
    plan = controller.Plan(uuid.uuid4().hex, (), bootstrap=bounds)
    path = controller.execute(plan, tmp_path)
    declaration = json.loads((path / "plan.json").read_text())
    result = json.loads((path / "result.json").read_text())
    assert declaration["bootstrap"] == asdict(bounds)
    assert result["ok"] is True
    assert result["inspection_worker"]["bootstrap_bounds"] == asdict(bounds)
    assert result["inspection_worker"]["owner"]["reaped"] is True
    with pytest.raises(ChildProcessError):
        os.waitpid(result["inspection_worker"]["owner"]["pid"], os.WNOHANG)
