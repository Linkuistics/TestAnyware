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
