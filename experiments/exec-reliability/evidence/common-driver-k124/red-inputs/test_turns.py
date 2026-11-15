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
