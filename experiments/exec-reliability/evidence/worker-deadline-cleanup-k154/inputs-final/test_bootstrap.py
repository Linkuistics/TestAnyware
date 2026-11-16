"""Declared preparation bounds exercised against exact owned local children."""

import json
import os
import select
import signal
import subprocess
import sys
import time
import uuid
from dataclasses import asdict
from pathlib import Path
from types import SimpleNamespace

import pytest

import controller
import inspection
import worker_capture
from turns import Turn

_real_kill = os.kill
_real_waitpid = os.waitpid


def _remove_unreaped_child(owner):
    """Remove the exact child that the runtime left unreaped after its deadline.

    The runtime must refuse to signal after its deadline, so the fixture removes
    its own child. An unreaped direct child keeps its PID, so the signal cannot
    reach another process. Return the removal record, or None after a runtime reap.
    """
    import worker_capture

    if owner is None or owner.reaped:
        return None
    removal = {"pid": owner.pid, "signal": int(signal.SIGKILL), "time": time.monotonic()}
    try:
        _real_kill(owner.pid, signal.SIGKILL)
    except OSError as error:
        removal["kill_errno"] = error.errno
    end = time.monotonic() + 1
    while time.monotonic() < end:
        try:
            found, status = _real_waitpid(owner.pid, os.WNOHANG)
        except ChildProcessError:
            removal["reap"] = "ECHILD"
            break
        if found == owner.pid:
            removal.update(reap="fixture", status=status)
            break
        time.sleep(0.001)
    if "reap" in removal:
        worker_capture._owners.pop(owner.pid, None)
    return removal


@pytest.mark.parametrize(
    "fault,message",
    [
        ("overflow", "invalid worker startup frame"),
        ("empty-eof", "invalid worker startup frame"),
        ("partial-eof", "invalid worker startup frame"),
        ("tail", "invalid worker startup identity"),
        ("session", "invalid worker startup identity"),
        ("pid", "invalid worker startup identity"),
        ("pgid", "invalid worker startup identity"),
        ("sid", "invalid worker startup identity"),
        ("missing", "invalid worker startup identity"),
        ("extra", "invalid worker startup identity"),
        ("duplicate", "duplicate worker publication key"),
        ("nesting", "worker publication nesting exceeds decoder limit"),
        ("json", "Expecting property name enclosed in double quotes: line 1 column 2 (char 1)"),
    ],
)
def test_startup_frame_rejection_is_failed_and_reaped(tmp_path, monkeypatch, fault, message):
    """Dropping EOF, cap, tail, identity or decoder rejection must fail before expiry."""
    worker = worker_capture.Worker(
        bootstrap=worker_capture.BootstrapBounds(readiness=0.3, term_grace=0.07, kill_reap=0.16)
    )
    real_read = os.read
    chunks = []
    journaled = False

    def read(fd, size):
        nonlocal journaled
        if fd == worker.control and not journaled:
            _journal_startup_worker(worker, tmp_path)
            journaled = True
        data = real_read(fd, size)
        if fd == worker.control:
            chunks.append(len(data))
        return data

    def service(command, control, result, session, deadline):
        try:
            inspection.reset_after_fork({command, control, result})
            identity = {
                "session": session,
                "pid": os.getpid(),
                "pgid": os.getpgrp(),
                "sid": os.getsid(0),
            }
            if fault in {"session", "pid", "pgid", "sid"}:
                identity[fault] = "wrong" if fault == "session" else os.getpid() + 1
            elif fault == "missing":
                del identity["sid"]
            elif fault == "extra":
                identity["unexpected"] = 1
            frame = json.dumps(identity).encode() + b"\n"
            if fault == "overflow":
                frame = b"x" * 4097
            elif fault == "empty-eof":
                os._exit(0)
            elif fault == "partial-eof":
                frame = b'{"session":'
            elif fault == "tail":
                frame += b"x"
            elif fault == "duplicate":
                frame = b'{"session":"wrong",' + frame[1:]
            elif fault == "nesting":
                frame = b"[" * 1100 + b"0" + b"]" * 1100 + b"\n"
            elif fault == "json":
                frame = b"{invalid}\n"
            assert os.write(control, frame) == len(frame)
            if fault == "partial-eof":
                os._exit(0)
            while True:
                signal.pause()
        finally:
            os._exit(70)

    monkeypatch.setattr(worker_capture, "_service", service)
    monkeypatch.setattr(os, "read", read)
    recursion_limit = sys.getrecursionlimit()
    if fault == "nesting":
        # Native decoding accepts every tested depth permitted by this frame cap.
        # Exercise the defensive path with the stdlib's recursive Python scanner.
        monkeypatch.setattr(json.scanner, "make_scanner", json.scanner.py_make_scanner)
        sys.setrecursionlimit(1000)
    try:
        assert not worker.start()
        assert worker.failure == message
        assert worker.readiness_outcome == "failed"
        assert worker.readiness_finished < worker.startup_deadline
        if fault == "overflow":
            # Once the 4097th byte arrives, no additional read (even size zero) is allowed.
            assert sum(chunks) == 4097 and chunks[-1] > 0
        assert worker.owner is not None and worker.owner.reaped
        assert worker.bootstrap_shutdown["outcome"] == "reaped"
        assert worker.bootstrap_shutdown["overrun"] is False
        assert worker.shutdown_finished < worker.shutdown_started + 0.23
        assert worker.startup_finished - worker.startup_started < 0.3 + 0.23
        assert not worker.start()
    finally:
        sys.setrecursionlimit(recursion_limit)
        worker.close()
        owner = worker.owner
        removal = _remove_unreaped_child(owner)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"startup-frame-{fault}-{time.time_ns()}.json").write_text(
            json.dumps(
                {
                    "runtime": worker.evidence(),
                    "chunks": chunks,
                    "audit": {"reaped": removal is None, "fixture_kill": removal},
                    "decoder": "stdlib Python scanner; recursion limit 1000"
                    if fault == "nesting"
                    else "native",
                },
                indent=2,
            )
            + "\n"
        )
        # A child that only the fixture removed means the runtime missed its deadline.
        assert owner is not None and owner.reaped and removal is None
        with pytest.raises(ChildProcessError):
            os.waitpid(owner.pid, os.WNOHANG)


def test_startup_frame_written_at_real_cutoff_is_late(tmp_path, monkeypatch):
    """Accepting a real cutoff write or relabelling its postparse result must fail."""
    worker = worker_capture.Worker(
        bootstrap=worker_capture.BootstrapBounds(readiness=0.3, term_grace=0.07, kill_reap=0.16)
    )
    report_read, report_write = os.pipe()
    real_read = os.read
    entered = []
    written = None

    def service(command, control, result, session, deadline):
        try:
            inspection.reset_after_fork({command, control, result, report_write})
            frame = (
                json.dumps(
                    {
                        "session": session,
                        "pid": os.getpid(),
                        "pgid": os.getpgrp(),
                        "sid": os.getsid(0),
                    }
                ).encode()
                + b"\n"
            )
            time.sleep(max(0, deadline - time.monotonic()))
            written = time.monotonic()
            assert os.write(control, frame) == len(frame)
            os.write(report_write, str(written).encode())
            os.close(report_write)
            while True:
                signal.pause()
        finally:
            os._exit(70)

    def read(fd, size):
        if fd == worker.control:
            _journal_startup_worker(worker, tmp_path)
            entered.append(time.monotonic())
            # A real pipe read begins before cutoff and completes on the child's write.
            assert select.select([fd], [], [], 0.5)[0] == [fd]
        return real_read(fd, size)

    monkeypatch.setattr(worker_capture, "_service", service)
    monkeypatch.setattr(os, "read", read)
    try:
        assert not worker.start()
        os.close(report_write)
        report_write = None
        written = float(real_read(report_read, 100))
        assert len(entered) == 1 and entered[0] < worker.startup_deadline
        assert worker.startup_deadline <= written <= worker.readiness_finished
        assert worker.failure == "late worker readiness"
        assert worker.readiness_outcome == "late"
        assert worker.readiness_finished - worker.startup_deadline < 0.1
        assert worker.owner is not None and not worker.owner.ready and worker.owner.reaped
        assert worker.bootstrap_shutdown["outcome"] == "reaped"
        assert worker.bootstrap_shutdown["overrun"] is False
        assert worker.shutdown_finished < worker.shutdown_started + 0.23
    finally:
        if report_write is not None:
            os.close(report_write)
        os.close(report_read)
        worker.close()
        owner = worker.owner
        removal = _remove_unreaped_child(owner)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"startup-frame-real-cutoff-{time.time_ns()}.json").write_text(
            json.dumps(
                {
                    "runtime": worker.evidence(),
                    "read_entered": entered,
                    "child_write_started": written,
                    "audit": {"reaped": removal is None, "fixture_kill": removal},
                    "injection": "real pipe read delayed until child's real cutoff write",
                },
                indent=2,
            )
            + "\n"
        )
        # A child that only the fixture removed means the runtime missed its deadline.
        assert owner is not None and owner.reaped and removal is None
        with pytest.raises(ChildProcessError):
            os.waitpid(owner.pid, os.WNOHANG)


def _journal_startup_worker(worker, tmp_path):
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
    root.mkdir(parents=True, exist_ok=True)
    observed = subprocess.run(
        ["/bin/ps", "-p", str(worker.owner.pid), "-o", "pid=,pgid=,stat=,lstart=,command="],
        capture_output=True,
        text=True,
        timeout=1,
    )
    assert observed.returncode == 0 and not observed.stderr
    controller.write_json(
        root / f"owned-termination-{uuid.uuid4().hex}.json",
        {
            "checker_pid": os.getpid(),
            "identity": {"pid": worker.owner.pid, "stdout": observed.stdout},
        },
    )


@pytest.mark.parametrize("fragment", [1, 7, 4097])
def test_startup_turns_yield_and_drop_terminal_interests(tmp_path, monkeypatch, fragment):
    """Real fragmented readiness yields after each bounded nonblocking read."""
    worker = worker_capture.Worker()
    original_run = worker_capture.run_turns
    original_read = os.read
    reads = []
    observations = []
    captured = []

    def read(fd, size):
        if fd == worker.control:
            reads.append(size)
            return original_read(fd, min(fragment, size))
        return original_read(fd, size)

    def run(*turns):
        if worker.closed:
            return original_run(*turns)
        assert len(turns) == 1
        startup = turns[0]
        captured.append(startup)
        _journal_startup_worker(worker, tmp_path)
        assert startup.readiness is not None
        assert startup.readiness() == ((worker.control,), ())

        def advance():
            before = len(reads)
            wake = startup.advance()
            assert len(reads) - before <= 1
            if wake is not None:
                assert wake <= worker.startup_deadline
            return wake

        def other_owner():
            observations.append((len(reads), worker.started))
            return None if worker.started else time.monotonic()

        return original_run(Turn(advance, readiness=startup.readiness), Turn(other_owner))

    monkeypatch.setattr(os, "read", read)
    monkeypatch.setattr(worker_capture, "run_turns", run)
    try:
        assert worker.start()
        assert len(captured) == 1
        assert reads and all(0 < size <= 4097 for size in reads)
        if fragment < 4097:
            assert any(count > 0 and not ready for count, ready in observations)
        startup = captured[0]
        before = len(reads)
        assert startup.advance() is None
        assert startup.readiness() == ((), ())
        assert worker.start()
        assert len(captured) == 1 and len(reads) == before
    finally:
        if not captured and worker.owner is not None and not worker.closed:
            _journal_startup_worker(worker, tmp_path)
        worker.close()
    assert startup.advance() is None
    assert startup.readiness() == ((), ())
    assert not worker.start()


def test_startup_turns_expired_callback_never_reads(tmp_path, monkeypatch):
    """A delayed first turn cannot renew readiness or consume a late identity."""
    worker = worker_capture.Worker()
    original_run = worker_capture.run_turns
    original_read = os.read
    captured = []

    def read(fd, size):
        assert fd != worker.control, "expired startup read its readiness channel"
        return original_read(fd, size)

    def run(*turns):
        if worker.closed:
            return original_run(*turns)
        captured.extend(turns)
        _journal_startup_worker(worker, tmp_path)
        with monkeypatch.context() as patch:
            patch.setattr(
                worker_capture, "time", SimpleNamespace(monotonic=lambda: worker.startup_deadline)
            )
            return original_run(*turns)

    monkeypatch.setattr(os, "read", read)
    monkeypatch.setattr(worker_capture, "run_turns", run)
    try:
        assert not worker.start()
        assert captured, "startup did not enter the common driver"
        assert worker.failure == "worker readiness deadline exhausted"
        assert worker.readiness_finished == worker.startup_deadline
        assert worker.readiness_outcome == "expired"
        assert captured[0].advance() is None
        assert captured[0].readiness() == ((), ())
        assert not worker.start()
    finally:
        if not captured and worker.owner is not None and not worker.closed:
            _journal_startup_worker(worker, tmp_path)
        worker.close()


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
        assert runtime["startup_deadline_monotonic"] == pytest.approx(
            worker.startup_started + 0.15, rel=0, abs=1e-9
        )
        observed = runtime["readiness_finished_monotonic"]
        deadline = runtime["startup_deadline_monotonic"]
        assert runtime["readiness_outcome"] == ("ready" if mode == "ready" else "expired")
        assert runtime["readiness_overrun"] is (mode != "ready")
        assert runtime["readiness_lateness_seconds"] == max(0.0, observed - deadline)
        if mode != "ready":
            # Observation slack is a test-host allowance, never runtime authority.
            assert 0 <= observed - deadline < 0.05
        owner = worker.owner
        assert owner is not None
        audit["pid"] = owner.pid
        if mode == "ready":
            assert worker.failure is None and owner.reaped and owner.returncode == 0
            assert runtime["bootstrap_shutdown"] is None and not calls
        else:
            assert worker.failure and worker.closed
            phase = runtime["bootstrap_shutdown"]
            assert phase["term_deadline_monotonic"] == pytest.approx(
                worker.shutdown_started + 0.07, rel=0, abs=1e-9
            )
            assert phase["kill_deadline_monotonic"] == pytest.approx(
                worker.shutdown_started + 0.23, rel=0, abs=1e-9
            )
            assert phase["outcome"] == (
                "residual_ownership" if mode in {"deferred-reap", "overrun"} else "reaped"
            )
            assert phase["overrun"] is (mode in {"deferred-reap", "overrun"})
            assert phase["deadline_exhausted"] is phase["overrun"]
            assert phase["lateness_seconds"] == max(
                0.0, phase["finished_monotonic"] - phase["kill_deadline_monotonic"]
            )
            if mode == "deferred-reap":
                assert 0 < phase["lateness_seconds"] < 0.05
            elif mode == "overrun":
                assert phase["lateness_seconds"] >= 0.04
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
                assert 0.07 <= calls[1]["time"] - worker.shutdown_started < 0.10
                if mode == "kill":
                    assert owner.reaped and owner.returncode == -signal.SIGKILL
                else:
                    assert not owner.reaped and runtime["residual_remedy"]
                    assert owner.pid in worker_capture._owners
            assert worker.startup_finished - worker.startup_started < 0.15 + 0.23 + 0.10
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


@pytest.mark.parametrize("offset", [-0.001, 0.0, 0.01])
def test_declared_bootstrap_valid_frame_deadline(tmp_path, monkeypatch, offset):
    """A real valid frame at equality or later cannot grant readiness authority."""
    worker = worker_capture.Worker(
        bootstrap=worker_capture.BootstrapBounds(readiness=0.15, term_grace=0.07, kill_reap=0.16)
    )
    real_clock, real_read = time.monotonic, os.read
    sampled = False
    shifted_at = None

    def clock():
        nonlocal sampled
        if shifted_at is None:
            return real_clock()
        if not sampled:
            sampled = True
            return worker.startup_deadline + offset
        return worker.startup_deadline + offset + real_clock() - shifted_at

    def read(fd, length):
        nonlocal shifted_at
        data = real_read(fd, length)
        if fd == worker.control and b"\n" in data and shifted_at is None:
            shifted_at = real_clock()
        return data

    monkeypatch.setattr(worker_capture, "time", SimpleNamespace(monotonic=clock, sleep=time.sleep))
    monkeypatch.setattr(os, "read", read)
    try:
        assert worker.start() is (offset < 0)
        assert sampled and worker.owner is not None
        assert worker.owner.ready is (offset < 0)
        receipt = worker.evidence()
        assert receipt["readiness_finished_monotonic"] == worker.startup_deadline + offset
        assert receipt["readiness_outcome"] == ("ready" if offset < 0 else "late")
        assert receipt["readiness_overrun"] is (offset > 0)
        assert receipt["readiness_lateness_seconds"] == max(
            0.0, receipt["readiness_finished_monotonic"] - worker.startup_deadline
        )
    finally:
        worker.close()
        owner = worker.owner
        assert owner is not None and owner.reaped
        with pytest.raises(ChildProcessError):
            os.waitpid(owner.pid, os.WNOHANG)
        root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
        root.mkdir(parents=True, exist_ok=True)
        (root / f"bootstrap-frame-{offset}-{time.time_ns()}.json").write_text(
            json.dumps(
                {
                    "runtime": worker.evidence(),
                    "audit": {"pid": owner.pid, "reaped": True},
                    "injection": "parent observation clock after real valid frame read",
                    "offset": offset,
                },
                indent=2,
            )
            + "\n"
        )


def test_declared_bootstrap_no_child_receipt(tmp_path, monkeypatch):
    """A fork error preserves failed readiness and the not-created outcome."""

    def fail_fork():
        raise OSError(11, "injected fork failure")

    monkeypatch.setattr(os, "fork", fail_fork)
    worker = worker_capture.Worker()
    assert not worker.start()
    receipt = worker.evidence()
    assert receipt["readiness_outcome"] == "failed"
    assert receipt["readiness_overrun"] is False
    assert receipt["readiness_lateness_seconds"] == 0
    phase = receipt["bootstrap_shutdown"]
    assert phase["outcome"] == "not_created"
    assert phase["overrun"] is False
    assert phase["deadline_exhausted"] is False
    assert phase["lateness_seconds"] == 0
    assert worker.owner is None and not worker.channels
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
    root.mkdir(parents=True, exist_ok=True)
    (root / f"bootstrap-no-child-{time.time_ns()}.json").write_text(
        json.dumps({"runtime": receipt, "audit": {"created": False}}, indent=2) + "\n"
    )
