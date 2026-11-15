"""Shared driver policy and real bounded-owner composition."""

import pytest

import turns


def clock(monkeypatch):
    now = [10.0]
    sleeps = []

    def sleep(delay):
        assert 0 < delay <= 0.005
        sleeps.append(delay)
        now[0] += delay

    monkeypatch.setattr(turns.time, "monotonic", lambda: now[0])
    monkeypatch.setattr(turns.time, "sleep", sleep)
    return now, sleeps


def test_shared_turns_safety_then_earliest_then_rotation(monkeypatch):
    now, sleeps = clock(monkeypatch)
    seen = []

    def owner(name, wake):
        count = 0

        def advance():
            nonlocal count
            seen.append((name, now[0]))
            count += 1
            return wake if count == 1 else None

        return advance

    turns.run_turns(
        turns.Turn(owner("ordinary", 10.01)),
        turns.Turn(owner("late", 10.02), safety=True),
        turns.Turn(owner("early", 10.01), safety=True),
    )
    assert [name for name, _ in seen] == ["late", "early", "ordinary", "early", "ordinary", "late"]
    assert seen[3][1] == pytest.approx(10.01)
    assert seen[-1][1] == pytest.approx(10.02)
    assert sleeps


def test_shared_turns_ready_safety_precedes_earlier_ordinary(monkeypatch):
    now, sleeps = clock(monkeypatch)
    seen = []

    def advance(name, wake):
        seen.append(name)
        if seen.count(name) == 1:
            now[0] += 0.02
            return wake
        return None

    turns.run_turns(
        turns.Turn(lambda: advance("ordinary", 10.001)),
        turns.Turn(lambda: advance("safety", 10.01), safety=True),
    )
    assert seen == ["safety", "safety", "ordinary", "ordinary"]
    assert not sleeps


def test_shared_turns_equal_ready_wakes_rotate(monkeypatch):
    now, sleeps = clock(monkeypatch)
    seen = []

    def advance(name):
        seen.append(name)
        return now[0] if seen.count(name) < 3 else None

    turns.run_turns(*(turns.Turn(lambda name=name: advance(name)) for name in "abc"))
    assert seen == list("abcabcabc")
    assert not sleeps


def test_shared_turns_empty_and_terminal_are_not_reentered(monkeypatch):
    _, sleeps = clock(monkeypatch)
    seen = []
    turns.run_turns()
    turns.run_turns(turns.Turn(lambda: seen.append("done")))
    assert seen == ["done"]
    assert not sleeps


def test_shared_turns_initial_due_owners_do_not_consume_clock(monkeypatch):
    def unexpected_clock():
        pytest.fail("initially due owners must advance before reading the clock")

    monkeypatch.setattr(turns.time, "monotonic", unexpected_clock)
    seen = []
    turns.run_turns(*(turns.Turn(lambda name=name: seen.append(name)) for name in "abc"))
    assert seen == list("abc")


def test_shared_turns_error_propagates_without_retry_or_sleep(monkeypatch):
    _, sleeps = clock(monkeypatch)
    seen = []

    def fail():
        seen.append("failed")
        raise OSError("bounded owner failure")

    with pytest.raises(OSError, match="bounded owner failure"):
        turns.run_turns(turns.Turn(fail), turns.Turn(lambda: seen.append("later")))
    assert seen == ["failed"]
    assert not sleeps


def test_shared_turns_real_worker_close_and_workload(tmp_path):
    import json
    import os
    import signal
    import subprocess
    import time
    import uuid
    from pathlib import Path

    import controller
    import worker_capture

    workers = [worker_capture.Worker(), worker_capture.Worker()]
    process = None
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
    root.mkdir(parents=True, exist_ok=True)

    def journal(pid):
        observed = subprocess.run(
            ["/bin/ps", "-p", str(pid), "-o", "pid=,pgid=,stat=,lstart=,command="],
            capture_output=True,
            text=True,
            timeout=1,
        )
        assert observed.returncode == 0 and not observed.stderr
        controller.write_json(
            root / f"owned-termination-{uuid.uuid4().hex}.json",
            {"checker_pid": os.getpid(), "identity": {"pid": pid, "stdout": observed.stdout}},
        )

    try:
        for worker in workers:
            assert worker.start(), worker.failure
            journal(worker.owner.pid)
        process = subprocess.Popen(["/bin/sleep", "0.04"], start_new_session=True)
        journal(process.pid)
        waiter = controller._WorkloadWait(process.pid, time.monotonic() + 1)
        # One failed and one healthy worker exercise independent shutdown owners.
        workers[0].failure = "injected worker failure"
        for worker in workers:
            worker.begin_close()
        turns.run_turns(
            turns.Turn(waiter.advance),
            *(turns.Turn(worker.advance_close, safety=True) for worker in workers),
        )
        assert waiter.exited
        assert process.wait(timeout=1) == 0
        assert all(worker.owner.reaped for worker in workers)
        assert workers[0].failure == "injected worker failure"
        assert workers[1].failure is None
        closed = [worker.shutdown_finished for worker in workers]
        turns.run_turns(*(turns.Turn(worker.advance_close, safety=True) for worker in workers))
        assert closed == [worker.shutdown_finished for worker in workers]
        (root / f"shared-turns-{uuid.uuid4().hex}.json").write_text(
            json.dumps(
                {"workload": process.pid, "workers": [worker.evidence() for worker in workers]}
            )
        )
    finally:
        for worker in workers:
            worker.close()
            owner = worker.owner
            if owner is not None and not owner.reaped:
                os.kill(owner.pid, signal.SIGKILL)
                os.waitpid(owner.pid, 0)
        if process is not None and process.poll() is None:
            process.kill()
            process.wait(timeout=1)


def test_shared_turns_orders_overdue_by_wake(monkeypatch):
    now, _ = clock(monkeypatch)
    seen = []

    def advance(name, wake):
        seen.append(name)
        if seen.count(name) == 1:
            if name == "c":
                now[0] = 11.0
            return wake
        return None

    turns.run_turns(
        *(
            turns.Turn(lambda name=name: advance(name, 10.5 if name == "a" else 10.2))
            for name in "abc"
        )
    )
    assert seen == list("abcbca")


@pytest.mark.parametrize("persistent", [False, True])
@pytest.mark.parametrize("stalled", [False, True])
def test_request_turns_yield_with_transport_priority_and_caps(
    tmp_path, monkeypatch, persistent, stalled
):
    import os
    import subprocess
    import time
    import uuid
    from pathlib import Path

    import controller
    import inspection
    import worker_capture

    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
    root.mkdir(parents=True, exist_ok=True)
    worker = worker_capture.Worker() if persistent else None
    journaled = set()
    original_reap = worker_capture._reap

    def reap(owner):
        if owner.pid not in journaled and not owner.reaped:
            observed = subprocess.run(
                ["/bin/ps", "-p", str(owner.pid), "-o", "pid=,pgid=,stat=,lstart=,command="],
                capture_output=True,
                text=True,
                timeout=1,
            )
            if observed.returncode == 0 and not observed.stderr:
                controller.write_json(
                    root / f"owned-termination-{uuid.uuid4().hex}.json",
                    {
                        "checker_pid": os.getpid(),
                        "identity": {"pid": owner.pid, "stdout": observed.stdout},
                    },
                )
            journaled.add(owner.pid)
        original_reap(owner)

    monkeypatch.setattr(worker_capture, "_reap", reap)
    if stalled:

        def stall(*_args, **_kwargs):
            time.sleep(10)
            raise AssertionError("stalled creation escaped fixed retirement")

        monkeypatch.setattr(inspection, "_spawn_helper", stall)
    if worker is not None:
        assert worker.start(), worker.failure

    phases = []
    active = [False]
    bytes_read = [0]
    reads = []
    polls = [0]
    original_read = os.read
    original_select = worker_capture.select.select

    def read(fd, size):
        if active[0]:
            import sys
            reads.append((size, sys._getframe(1).f_code.co_name))
            bytes_read[0] += size
        return original_read(fd, size)

    def select(readers, writers, errors, timeout):
        if active[0]:
            polls[0] += 1
            assert timeout == 0, "a transport turn blocked other ready owners"
        return original_select(readers, writers, errors, timeout)

    def drive(*owners):
        # Query is the first invocation; request failure/standalone retirement
        # follows. Worker.close may add its own later already-reviewed driver.
        phase = len(phases)
        events = []
        phases.append(events)
        assert len(owners) == 1
        subject = owners[0]

        def advance():
            events.append("owner")
            bytes_read[0] = 0
            active[0] = True
            try:
                wake = subject.advance()
                assert bytes_read[0] <= 65536, reads
                if wake is None:
                    before = bytes_read[0], polls[0]
                    assert subject.advance() is None
                    assert (bytes_read[0], polls[0]) == before, "terminal transport polled again"
                return wake
            finally:
                active[0] = False

        competitor = turns.Turn(lambda: events.append("competitor"), safety=phase == 0)
        measured = turns.Turn(advance, safety=subject.safety)
        # Competing safety must beat the query even when registered second;
        # retiring safety must beat ordinary work registered first.
        turns.run_turns(*((measured, competitor) if phase == 0 else (competitor, measured)))

    monkeypatch.setattr(worker_capture, "run_turns", drive)
    monkeypatch.setattr(worker_capture.os, "read", read)
    monkeypatch.setattr(worker_capture.select, "select", select)
    try:
        receipt = worker_capture.capture_helper(
            ("/bin/sh", "-c", "dd if=/dev/zero bs=131072 count=1 2>/dev/null"),
            time.monotonic() + 1,
            worker=worker,
        )
        assert phases, "request transport bypassed the shared driver"
        assert phases[0][:2] == ["competitor", "owner"]
        assert phases[0].count("owner") > 4
        assert receipt.worker is not None
        if stalled or not persistent:
            assert len(phases) >= 2
            assert phases[1][:2] == ["owner", "competitor"]
            assert receipt.worker["reaped"]
        if stalled:
            assert receipt.error
            assert receipt.worker["unknown_helper_creation"]
        else:
            assert receipt.error is None
            assert receipt.reaped
        controller.write_json(
            root / f"request-turns-{uuid.uuid4().hex}.json",
            {
                "phases": phases,
                "worker": receipt.worker,
                "error": receipt.error,
                "persistent": persistent,
                "stalled": stalled,
            },
        )
    finally:
        if worker is not None:
            worker.close()
