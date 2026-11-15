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
        "reserve",
        "aged-term",
    ],
)
@pytest.mark.parametrize("sample", range(3))
@pytest.mark.parametrize("driver", ["blocking", "incremental"])
def test_declared_shutdown_real_child_phases(tmp_path, monkeypatch, mode, sample, driver):
    """Real signals/reap; deferred and late wait returns are explicitly simulated."""
    original_service = worker_capture._service
    original_waitpid = os.waitpid
    original_sleep = time.sleep
    original_kill, original_killpg = os.kill, os.killpg
    failure = worker_capture.FailureBounds(term_grace=0.07, kill_reap=0.16)
    if mode in {"reserve", "aged-term"}:
        failure = worker_capture.FailureBounds(term_grace=0.25, kill_reap=0.16)
    close = worker_capture.CloseBounds(grace=0.04, kill_reap=0.13)
    worker = worker_capture.Worker(failure_bounds=failure, close_bounds=close)
    calls, probes, turns = [], [], []
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
                original_sleep(0.27)
        return original_waitpid(pid, flags)

    def killpg(pid, sig):
        calls.append({"pid": pid, "signal": sig.name, "monotonic": time.monotonic()})
        return original_killpg(pid, sig)

    monkeypatch.setattr(worker_capture, "_service", service)
    audit = {
        "reaped": False,
        "source": "original exact-child WNOHANG",
        "mode": mode,
        "sample": sample,
        "driver": driver,
    }
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
        if mode == "reserve":
            old_end = time.monotonic() + 0.30
        elif mode == "aged-term":
            worker_capture._signal(owner, signal.SIGTERM)
            seeded = list(owner.signals)
            # Age the real attempt so renewing grace moves the deadline observably.
            time.sleep(0.03)
            old_end = time.monotonic() + 0.60
        owner.shutdown_deadline_monotonic = old_end
        if mode != "orderly" and mode != "healthy-stall":
            worker.failure = "controlled sticky failure"
        monkeypatch.setattr(os, "waitpid", waitpid)
        monkeypatch.setattr(os, "killpg", killpg)
        if driver == "blocking":
            worker.close()
        else:
            # A hidden sleep or drain loop blocks the caller's other obligations.
            def forbidden_sleep(seconds):
                raise AssertionError("shutdown step slept inside caller turn")

            with monkeypatch.context() as step:
                step.setattr(time, "sleep", forbidden_sleep)
                worker.begin_close()
            assert not probes and not calls
            frozen_end = owner.shutdown_deadline_monotonic
            frozen_start = worker.shutdown_started
            while True:
                before = len(probes)
                # The overrun control explicitly models one delayed wait return.
                with monkeypatch.context() as step:
                    step.setattr(time, "sleep", forbidden_sleep)
                    worker.begin_close()
                    wake = worker.advance_close()
                turns.append({"monotonic": time.monotonic(), "wake": wake})
                assert len(probes) - before <= 1
                assert owner.shutdown_deadline_monotonic == frozen_end
                assert worker.shutdown_started == frozen_start
                if wake is None:
                    break
                assert wake <= frozen_end
                # Independent caller work is possible while the child is alive.
                assert worker.shutdown_finished is None
                time.sleep(max(0, min(0.001, wake - time.monotonic())))
            if mode in {"healthy-stall", "failure-kill"}:
                kill_time = owner.signals[-1]["monotonic"]
                assert sum(turn["monotonic"] < kill_time for turn in turns) > 1
            assert worker.advance_close() is None
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
        elif mode == "reserve":
            assert signals == ["SIGTERM", "SIGKILL"] and owner.reaped
            assert worker.kill_at == pytest.approx(old_end - failure.kill_reap, abs=1e-9, rel=0)
            assert worker.kill_at > start
            assert end == old_end
        elif mode == "aged-term":
            assert signals == ["SIGTERM", "SIGKILL"] and owner.reaped
            assert worker.kill_at == pytest.approx(
                seeded[0]["monotonic"] + failure.term_grace, abs=1e-9, rel=0
            )
            assert worker.kill_at < start + failure.term_grace - 0.02
        elif mode == "expired":
            assert not signals and len(probes) == 1 and probes[0] >= end and not owner.reaped
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
        if mode != "expired":
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
        (root / f"shutdown-{driver}-{mode}-{time.time_ns()}.json").write_text(
            json.dumps(
                {
                    "runtime": runtime,
                    "calls": calls,
                    "probes": probes,
                    "turns": turns,
                    "audit": audit,
                },
                indent=2,
            )
            + "\n"
        )
        assert audit["reaped"]


def test_incremental_shutdown_evidence_retains_live_authority(monkeypatch):
    """A begun close must not claim terminal authority while turns remain."""
    clock = [100.0]
    worker = worker_capture.Worker(
        failure_bounds=worker_capture.FailureBounds(term_grace=0.07, kill_reap=0.16)
    )
    worker.owner = worker_capture._Owner(999999, float("inf"), ready=True)
    worker.failure = "controlled failure"
    monkeypatch.setattr(time, "monotonic", lambda: clock[0])
    monkeypatch.setattr(os, "waitpid", lambda pid, flags: (0, 0))
    monkeypatch.setattr(os, "killpg", lambda pid, sig: None)
    worker.begin_close()
    evidence = worker.evidence()
    assert evidence["shutdown_started_monotonic"] == 100.0
    assert evidence["shutdown_finished_monotonic"] is None
    assert evidence["shutdown"] is None
    assert evidence["residual_remedy"] is None
    assert worker.advance_close() == 100.001
    assert worker.evidence()["residual_remedy"] is None
    clock[0] = 100.23
    assert worker.advance_close() is None
    evidence = worker.evidence()
    assert evidence["shutdown_finished_monotonic"] == 100.23
    assert evidence["shutdown"]["outcome"] == "residual_ownership"
    assert evidence["residual_remedy"] is not None


@pytest.mark.parametrize("orderly", [False, True], ids=["failure", "orderly"])
@pytest.mark.parametrize(
    "first,wake,signals,grace_timing",
    [
        (100.0, 100.001, ["SIGTERM"], "late"),
        (100.0695, 100.07, ["SIGTERM"], "late"),
        (100.07, 100.071, ["SIGTERM", "SIGKILL"], "at_deadline"),
        (100.073, 100.074, ["SIGTERM", "SIGKILL"], "late"),
        (100.2295, 100.23, ["SIGTERM", "SIGKILL"], "late"),
        (100.23, None, [], "late"),
        (100.233, None, [], "late"),
    ],
)
@pytest.mark.parametrize("prior_polls", [0, 1], ids=["never-polled", "previously-polled"])
def test_incremental_shutdown_exact_wakes_and_starved_caller(
    monkeypatch, orderly, first, wake, signals, grace_timing, prior_polls
):
    """Catch late wake/renewed grace; only OS calls and the clock are simulated."""
    clock = [100.0]
    probes = []
    worker = worker_capture.Worker(
        failure_bounds=worker_capture.FailureBounds(term_grace=0.07, kill_reap=0.16),
        close_bounds=worker_capture.CloseBounds(grace=0.07, kill_reap=0.16),
    )
    owner = worker_capture._Owner(999999, float("inf"), ready=True, reap_attempts=prior_polls)
    worker.owner = owner
    if not orderly:
        worker.failure = "controlled failure"

    def waitpid(pid, flags):
        assert pid == 999999 and flags == os.WNOHANG
        probes.append(clock[0])
        return 0, 0

    def forbidden_sleep(seconds):
        raise AssertionError("incremental shutdown slept")

    monkeypatch.setattr(time, "monotonic", lambda: clock[0])
    monkeypatch.setattr(time, "sleep", forbidden_sleep)
    monkeypatch.setattr(os, "waitpid", waitpid)
    monkeypatch.setattr(os, "killpg", lambda pid, sig: None)
    worker.begin_close()
    assert probes == [] and owner.signals == []
    assert worker.kill_at == 100.07
    assert owner.shutdown_deadline_monotonic == 100.23
    clock[0] = first
    worker.begin_close()
    assert worker.advance_close() == wake
    expected_signals = [value for value in signals if not orderly or value != "SIGTERM"]
    assert [value["signal"] for value in owner.signals] == expected_signals
    assert all(value["monotonic"] == first for value in owner.signals)
    assert len(probes) == (0 if prior_polls and first >= 100.23 else 1)
    assert worker.kill_at == 100.07
    assert owner.shutdown_deadline_monotonic == 100.23
    # Finish exactly at the original cutoff. No turn may renew signal/poll authority.
    clock[0] = max(first, 100.23)
    before = len(probes)
    assert worker.advance_close() is None
    assert worker.advance_close() is None
    assert len(probes) == before
    assert [value["signal"] for value in owner.signals] == expected_signals
    receipt = worker.evidence()["shutdown"]
    assert receipt["deadline_exhausted"] is True
    assert receipt["overrun"] is (first > 100.23)
    assert receipt["phases"]["grace"]["timing_outcome"] == grace_timing
    assert ("grace overrun" in worker.failure) is (grace_timing == "late")


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


@pytest.mark.parametrize("delay", [-0.04, 0.0, 0.003])
def test_declared_shutdown_exact_escalation_clock(monkeypatch, delay):
    """Value/clock seam supplements real signals; it owns no OS child."""
    clock = [100.0]
    attempts = []
    worker = worker_capture.Worker(
        failure_bounds=worker_capture.FailureBounds(term_grace=0.07, kill_reap=0.16)
    )
    owner = worker_capture._Owner(999999, float("inf"))
    owner.ready = True
    worker.owner = owner
    worker.failure = "controlled failure"

    def sleep(seconds):
        # Model early TERM exit, escalation at T, or a turn 3 ms after T.
        clock[0] = 100.07 + delay if clock[0] == 100.0 else clock[0] + seconds

    def reap(child):
        if len(attempts) == 2 or (delay < 0 and attempts):
            child.reaped = True
            child.returncode = -signal.SIGKILL

    def send(child, sig):
        attempts.append(sig)
        child.signals.append({"signal": sig.name, "monotonic": clock[0], "errno": None})

    monkeypatch.setattr(time, "monotonic", lambda: clock[0])
    monkeypatch.setattr(time, "sleep", sleep)
    monkeypatch.setattr(worker_capture, "_reap", reap)
    monkeypatch.setattr(worker_capture, "_signal", send)
    worker.close()
    grace = worker.shutdown["phases"]["grace"]
    assert worker.kill_at == pytest.approx(100.07, abs=1e-10, rel=0)
    assert attempts == ([signal.SIGTERM] if delay < 0 else [signal.SIGTERM, signal.SIGKILL])
    assert grace["finished_monotonic"] == pytest.approx(100.07 + delay, abs=1e-10, rel=0)
    expected = "on_time" if delay < 0 else "at_deadline" if delay == 0 else "late"
    assert grace["timing_outcome"] == expected
    assert grace["overrun"] is (delay > 0)
    assert ("grace overrun" in worker.failure) is (delay > 0)


@pytest.mark.parametrize("at", [99.9, 100.0, 100.1], ids=["before", "equal", "after"])
@pytest.mark.parametrize("ready", [False, True], ids=["pid", "group"])
@pytest.mark.parametrize(
    "outcome", ["exited", "live", "late-exited", "late-live", "echild", "eio", "late-eio"]
)
def test_worker_first_reap_at_fixed_cutoff(tmp_path, monkeypatch, at, ready, outcome):
    """Real exact children; only clocks, deferred results and errno are simulated."""
    original_waitpid = os.waitpid
    original_monotonic = time.monotonic
    read_fd, write_fd = os.pipe()
    release_read, release_write = os.pipe()
    pid = os.fork()
    if pid == 0:
        try:
            inspection.reset_after_fork({write_fd, release_read})
            os.write(write_fd, b"R")
            if outcome in {"live", "late-live"}:
                while True:
                    signal.pause()
            os.read(release_read, 1)
            os._exit(7)
        finally:
            os._exit(70)
    os.close(write_fd)
    os.close(release_read)
    owner = worker_capture._Owner(pid, 100.0, ready=ready)
    worker_capture._owners[pid] = owner
    now = [at]
    calls = []
    runtime = None
    audit = {"pid": pid, "reaped": False, "source": "original exact-child WNOHANG"}

    def waitpid(child, flags):
        assert child == pid and flags == os.WNOHANG
        calls.append({"pid": child, "flags": flags, "monotonic": now[0]})
        if outcome.startswith("late-"):
            now[0] = 100.2
        if outcome in {"echild", "eio", "late-eio"}:
            number = errno.ECHILD if outcome == "echild" else errno.EIO
            raise OSError(number, "simulated worker reap errno")
        return original_waitpid(child, flags)

    def forbidden_signal(*args):
        raise AssertionError("expired or terminal owner regained signal authority")

    try:
        # Readiness is bounded independently of the synthetic runtime clock.
        import select

        assert select.select([read_fd], [], [], 1)[0]
        assert os.read(read_fd, 1) == b"R"
        if outcome not in {"live", "late-live"}:
            # Register before releasing exit; observe without consuming status.
            queue = select.kqueue()
            try:
                event = select.kevent(
                    pid,
                    filter=select.KQ_FILTER_PROC,
                    flags=select.KQ_EV_ADD,
                    fflags=select.KQ_NOTE_EXIT,
                )
                queue.control([event], 0, 0)
                os.write(release_write, b"X")
                events = queue.control(None, 1, 1)
                assert any(item.fflags & select.KQ_NOTE_EXIT for item in events)
            finally:
                queue.close()
        with monkeypatch.context() as patch:
            patch.setattr(time, "monotonic", lambda: now[0])
            patch.setattr(os, "waitpid", waitpid)
            patch.setattr(os, "kill", forbidden_signal)
            patch.setattr(os, "killpg", forbidden_signal)
            worker_capture._reap(owner)
            assert len(calls) == 1
            if outcome == "live" and at < 100:
                # A second pre-cutoff poll is still permitted.
                now[0] = 99.99
                worker_capture._reap(owner)
                assert len(calls) == 2
            attempts = len(calls)
            for expired in (100.0, 100.3, 100.5):
                now[0] = expired
                worker_capture._reap(owner)
                worker_capture._signal(owner, signal.SIGTERM)
                worker_capture._signal(owner, signal.SIGKILL)
            assert len(calls) == attempts
            assert owner.reap_attempts == attempts
            assert owner.shutdown_deadline_monotonic == 100.0 and not owner.signals
            assert owner.reaped is (outcome in {"exited", "late-exited"})
            assert owner.returncode == (7 if owner.reaped else None)
            expected_errno = (
                errno.ECHILD
                if outcome == "echild"
                else errno.EIO
                if outcome in {"eio", "late-eio"}
                else None
            )
            assert owner.reap_errno == expected_errno
            assert owner.ownership_lost is (outcome == "echild")
            assert (pid in worker_capture._owners) is not (owner.reaped or owner.ownership_lost)
            runtime = asdict(owner)
    finally:
        os.close(read_fd)
        os.close(release_write)
        # The audit bypasses injected errors/clocks. A real child stays pinned
        # until this exact wait consumes it, including simulated ECHILD cases.
        if owner.reaped:
            with pytest.raises(ChildProcessError):
                original_waitpid(pid, os.WNOHANG)
            audit.update(reaped=True, source="runtime reap independently confirmed ECHILD")
        else:
            found, status = original_waitpid(pid, os.WNOHANG)
            if found == 0:
                os.kill(pid, signal.SIGKILL)
                end = original_monotonic() + 1
                while original_monotonic() < end:
                    found, status = original_waitpid(pid, os.WNOHANG)
                    if found == pid:
                        break
                    time.sleep(0.001)
            audit.update(reaped=found == pid, status=status)
        if audit["reaped"]:
            worker_capture._owners.pop(pid, None)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"first-reap-{at}-{ready}-{outcome}-{time.time_ns()}.json").write_text(
            json.dumps({"runtime": runtime, "calls": calls, "audit": audit}, indent=2) + "\n"
        )
        assert audit["reaped"]
