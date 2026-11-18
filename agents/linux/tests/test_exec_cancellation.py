"""Cancellation controls of the Linux agent (ADR-0025).

Row "Cancellation, idempotence, tombstone" of the required-control table in
docs/specs/guest-command-lifecycle.md is new behaviour: no old path exists.
The endpoint runs through raw HTTP to the production server class. The race
and the signal order use the owner directly, with the seams of `OwnerSeams`.
"""

from __future__ import annotations

import json
import os
import select
import signal
import threading
import time
from collections.abc import Iterator
from pathlib import Path

import pytest

from testanyware_agent import exec_owner, exec_registry
from testanyware_agent.exec_owner import OwnerSeams, PidfdLeaderWaiter
from tests.exec_validity import record_errors, stream_bytes
from tests.http_agent import get_health, in_process_agent, post_json, timed
from tests.processes import child_survives, kill_if_same, process_state, wait_for_child

pytestmark = pytest.mark.linux_kernel

CANCEL_REPLY_BOUND_S = 5.0  # spec, section "Bounds": a reply with waitMs 5000
OWNER_LATENESS_BOUND_MS = 500


@pytest.fixture(autouse=True)
def capture_root(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    root = tmp_path / "captures"
    monkeypatch.setenv(exec_owner.CAPTURE_ROOT_ENV, str(root))
    return root


@pytest.fixture
def port() -> Iterator[int]:
    with in_process_agent() as agent_port:
        yield agent_port


class Background:
    def __init__(self, port: int, body: dict) -> None:
        self.reply: tuple[int, dict] | None = None
        self._thread = threading.Thread(target=self._run, args=(port, body), daemon=True)
        self._thread.start()

    def _run(self, port: int, body: dict) -> None:
        self.reply = post_json(port, "/exec", body)

    def join(self, limit: float = 10.0) -> tuple[int, dict]:
        self._thread.join(limit)
        assert self.reply is not None, "the exec request did not reply"
        return self.reply


def cancel(port: int, exec_id: str, wait_ms: int | None = None) -> tuple[int, dict]:
    body: dict = {"id": exec_id}
    if wait_ms is not None:
        body["waitMs"] = wait_ms
    return post_json(port, "/exec/cancel", body)


def wait_running(exec_id: str, limit: float = 5.0) -> None:
    """Wait until the registry holds the id as a running command."""
    end = time.monotonic() + limit
    while time.monotonic() < end:
        if exec_registry.REGISTRY.state(exec_id) == exec_registry.RUNNING:
            return
        time.sleep(0.01)
    raise AssertionError(f"{exec_id} never ran")


# ---- Cancellation ends the group --------------------------------------------


def test_cancellation_ends_the_group_and_replies_with_the_record(
    port: int, tmp_path: Path
) -> None:
    pid_file = tmp_path / "child.pid"
    command = f"sleep 120 & echo $! > {pid_file}; wait"
    running = Background(port, {"command": command, "timeout": 60, "id": "c-1"})
    child = wait_for_child(pid_file)
    try:
        elapsed, (status, reply) = timed(cancel, port, "c-1", 5000)
        assert status == 200
        assert elapsed < CANCEL_REPLY_BOUND_S
        assert reply["state"] == "terminal"
        record = reply["record"]
        assert record_errors(record, exec_id="c-1") == []
        assert record["outcome"] == {"kind": "cancelled", "beforeStart": False}
        assert record["lifecycle"]["ownerLatenessMs"] <= OWNER_LATENESS_BOUND_MS
        termination = record["termination"]
        assert (termination["groupEmpty"], termination["withinBounds"]) == ("yes", True)
        assert (record["exitCode"], record["stdout"], record["stderr"]) == (
            -1,
            "",
            "Process cancelled",
        )
        assert not child_survives(*child)
        status, exec_reply = running.join()
        assert (status, exec_reply) == (200, record)
    finally:
        kill_if_same(*child)


def test_cancellation_reply_waits_at_most_wait_ms(port: int, tmp_path: Path) -> None:
    """A leader that ignores SIGTERM keeps the command terminating until
    SIGKILL at T0 + 1.9 s. With `waitMs` 0 the reply comes at once."""
    pid_file = tmp_path / "leader.pid"
    command = f"trap '' TERM; echo $$ > {pid_file}; sleep 30"
    running = Background(port, {"command": command, "timeout": 60, "id": "slow"})
    leader = wait_for_child(pid_file)
    try:
        elapsed, (status, reply) = timed(cancel, port, "slow", 0)
        assert (status, reply) == (200, {"state": "terminating"})
        assert elapsed < 1.0
        elapsed, (status, reply) = timed(cancel, port, "slow", 5000)
        assert elapsed < CANCEL_REPLY_BOUND_S
        assert reply["state"] == "terminal"
        termination = reply["record"]["termination"]
        assert 1800 <= termination["killSentAfterMs"] <= 2000
        assert termination["leaderStatus"] == {"kind": "signalled", "signal": signal.SIGKILL}
        assert running.join()[1] == reply["record"]
    finally:
        kill_if_same(*leader)


# ---- Idempotence ----------------------------------------------------------


def test_a_repeated_cancellation_returns_the_same_record(port: int) -> None:
    running = Background(port, {"command": "sleep 30", "timeout": 60, "id": "twice"})
    wait_running("twice")
    first = cancel(port, "twice", 5000)
    second = cancel(port, "twice", 5000)
    third = cancel(port, "twice")
    assert first[1]["state"] == "terminal"
    assert json.dumps(first, sort_keys=True) == json.dumps(second, sort_keys=True)
    assert json.dumps(first, sort_keys=True) == json.dumps(third, sort_keys=True)
    running.join()


def test_a_cancellation_after_a_timeout_returns_the_timed_out_record(port: int) -> None:
    status, record = post_json(port, "/exec", {"command": "sleep 30", "timeout": 0.3, "id": "to"})
    assert record["outcome"] == {"kind": "timed_out", "timeoutSeconds": 0.3}
    assert cancel(port, "to", 5000) == (200, {"state": "terminal", "record": record})


def test_a_cancellation_during_timeout_termination_keeps_the_timeout(port: int) -> None:
    command = "trap '' TERM; sleep 30"
    running = Background(port, {"command": command, "timeout": 0.3, "id": "to-term"})
    time.sleep(0.8)  # the owner chose timed_out and waits for SIGKILL at T0 + 1.9 s
    status, reply = cancel(port, "to-term", 5000)
    assert reply["state"] == "terminal"
    assert reply["record"]["outcome"] == {"kind": "timed_out", "timeoutSeconds": 0.3}
    assert running.join()[1] == reply["record"]


def test_a_cancellation_after_an_exit_returns_the_exit_record(port: int) -> None:
    status, record = post_json(port, "/exec", {"command": "exit 23", "timeout": 10, "id": "ex"})
    assert record["outcome"] == {"kind": "exited", "exitCode": 23}
    assert cancel(port, "ex", 0) == (200, {"state": "terminal", "record": record})


# ---- Exit-and-cancel race (spec, section "Arbitration") ---------------------


class ExitBlindWaiter(PidfdLeaderWaiter):
    """The owner never hears the leader exit, only the cancellation and the
    deadline. So the cancellation arrives after the leader has exited."""

    def wait_exit(self, timeout: float | None, wake: int | None = None) -> bool:
        limit = 60.0 if timeout is None else max(0.0, timeout)
        if wake is None:
            time.sleep(limit)
        else:
            select.select([wake], [], [], limit)
        return False


def test_the_exit_and_cancel_race_resolves_by_kernel_state() -> None:
    registry = exec_registry.ExecRegistry()
    admission = registry.admit("race")
    assert admission.kind == "start"

    def own() -> None:
        record = exec_owner.run_command(
            "printf 'finished'; exit 7",
            30,
            "race",
            time.monotonic(),
            OwnerSeams(leader_waiter=ExitBlindWaiter),
            cancel=admission.signal,
        )
        registry.publish("race", record)

    owner = threading.Thread(target=own, daemon=True)
    owner.start()
    time.sleep(0.3)  # the leader exits now, unheard
    state, record = registry.cancel("race", 5.0)
    owner.join(5)
    assert state == "terminal" and record is not None
    reply = record.to_dict()
    assert record_errors(reply, exec_id="race") == []
    assert reply["outcome"] == {"kind": "exited", "exitCode": 7}
    assert reply["termination"] is None
    assert reply["remainingGroup"] == "none"
    assert stream_bytes(reply, "stdout") == b"finished"
    assert reply["lifecycle"]["ownerLatenessMs"] <= OWNER_LATENESS_BOUND_MS


# ---- Tombstone (spec, section "Cancel endpoint") ----------------------------


def test_a_tombstone_cancels_a_later_exec_without_a_spawn(port: int, tmp_path: Path) -> None:
    marker = tmp_path / "ran"
    assert cancel(port, "early") == (200, {"state": "unknown_id"})
    assert cancel(port, "early", 5000) == (200, {"state": "unknown_id"})
    status, record = post_json(
        port, "/exec", {"command": f"touch {marker}", "timeout": 10, "id": "early"}
    )
    assert status == 200
    assert record_errors(record, exec_id="early") == []
    assert record["outcome"] == {"kind": "cancelled", "beforeStart": True}
    assert record["termination"] is None
    assert record["lifecycle"]["leaderPid"] is None
    assert record["lifecycle"]["ownerLatenessMs"] == 0
    assert (record["exitCode"], record["stderr"]) == (-1, "Process cancelled")
    time.sleep(0.2)
    assert not marker.exists(), "a tombstoned exec spawned its command"
    assert cancel(port, "early") == (200, {"state": "terminal", "record": record})
    status, again = post_json(port, "/exec", {"command": "true", "timeout": 10, "id": "early"})
    assert (status, again["error"]) == (409, "exec_id_in_use")


def test_a_cancellation_before_the_spawn_starts_nothing(tmp_path: Path) -> None:
    marker = tmp_path / "ran"
    registry = exec_registry.ExecRegistry()
    admission = registry.admit("accepted")
    assert admission.signal is not None
    admission.signal.request(time.monotonic())
    record = exec_owner.run_command(
        f"touch {marker}", 10, "accepted", time.monotonic(), cancel=admission.signal
    ).to_dict()
    assert record_errors(record, exec_id="accepted") == []
    assert record["outcome"] == {"kind": "cancelled", "beforeStart": True}
    assert not marker.exists()


def test_a_duplicate_id_of_a_running_command_is_refused(port: int) -> None:
    running = Background(port, {"command": "sleep 30", "timeout": 60, "id": "dup"})
    wait_running("dup")
    status, reply = post_json(port, "/exec", {"command": "true", "timeout": 5, "id": "dup"})
    assert (status, reply["error"]) == (409, "exec_id_in_use")
    cancel(port, "dup", 5000)
    assert running.join()[1]["outcome"]["kind"] == "cancelled"


# ---- The order of signals and the reap after a cancellation -----------------


def test_no_nonzero_signal_reaches_the_group_after_the_reap_of_a_cancelled_leader(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """The zombie keeps the group ID reserved only until the reap (spec,
    section "Process ownership and identity"). A cancellation follows the same
    order as a timeout: SIGTERM and SIGKILL before the reap, then signal 0."""
    events: list[tuple[str, int, bool]] = []
    real_killpg = os.killpg

    def recording_killpg(group_id: int, signum: int) -> None:
        events.append(("signal", signum, process_state(group_id) is not None))
        real_killpg(group_id, signum)

    class ReapRecordingWaiter(PidfdLeaderWaiter):
        def reap(self):  # type: ignore[no-untyped-def]
            status = super().reap()
            events.append(("reap", 0, False))
            return status

    monkeypatch.setattr(exec_owner.os, "killpg", recording_killpg)
    signal_ = exec_owner.CancelSignal()
    threading.Timer(0.3, signal_.request, args=(time.monotonic() + 0.3,)).start()
    try:
        record = exec_owner.run_command(
            "trap '' TERM; sleep 30",
            60,
            "order",
            time.monotonic(),
            OwnerSeams(leader_waiter=ReapRecordingWaiter),
            cancel=signal_,
        ).to_dict()
    finally:
        signal_.close()
    monkeypatch.undo()

    assert record["outcome"] == {"kind": "cancelled", "beforeStart": False}
    reap_at = events.index(("reap", 0, False))
    before, after = events[:reap_at], events[reap_at + 1 :]
    assert [(s, alive) for _, s, alive in before if s != 0] == [
        (signal.SIGTERM, True),
        (signal.SIGKILL, True),
    ], events
    assert all(s == 0 for _, s, _ in after), events


# ---- Timeouts past the poll range (k234 D5) ---------------------------------


@pytest.mark.parametrize("timeout", [20_000_000_000, 10**400], ids=["2e10-s", "past-float"])
def test_a_huge_timeout_runs_and_cancels(port: int, timeout: int) -> None:
    """The old owner passed the whole wait to `poll`, which raised
    OverflowError after the spawn, so the request got HTTP 500 and no owner
    ended the group. The owner now waits 60 s at most in each kernel call."""
    body = {"command": "sleep 30", "timeout": timeout, "id": "huge"}
    running = Background(port, body)
    wait_running("huge")
    time.sleep(0.3)
    assert get_health(port, 1.0)[0] == 200
    status, reply = cancel(port, "huge", 5000)
    assert reply["state"] == "terminal"
    assert reply["record"]["outcome"] == {"kind": "cancelled", "beforeStart": False}
    status, exec_reply = running.join()
    assert status == 200
    assert record_errors(exec_reply, exec_id="huge") == []


def test_the_leader_wait_never_passes_more_than_60_s_to_one_poll(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    waits: list[int] = []
    real_poll = select.poll

    class RecordingPoll:
        def __init__(self) -> None:
            self._poll = real_poll()

        def register(self, fd: int, mask: int) -> None:
            self._poll.register(fd, mask)

        def poll(self, timeout_ms: int) -> list[tuple[int, int]]:
            waits.append(timeout_ms)
            return self._poll.poll(0)  # answer at once, so the test stays short

    signal_ = exec_owner.CancelSignal()
    with pytest.MonkeyPatch.context() as patch:
        patch.setattr(exec_owner.select, "poll", RecordingPoll)
        threading.Timer(0.05, signal_.request, args=(time.monotonic(),)).start()
        waiter = PidfdLeaderWaiter(os.getpid())
        try:
            assert waiter.wait_exit(20_000_000_000, signal_.wake_fd) is False
        finally:
            waiter.close()
            signal_.close()
    assert waits and max(waits) <= 60_000


# ---- Request validation ---------------------------------------------------


@pytest.mark.parametrize(
    "body",
    [
        {},
        {"id": "has space"},
        {"id": "x" * 65},
        {"id": 7},
        {"id": "ok", "waitMs": -1},
        {"id": "ok", "waitMs": 10001},
        {"id": "ok", "waitMs": 1.5},
        {"id": "ok", "waitMs": True},
        {"id": "ok", "waitMs": "5"},
    ],
)
def test_a_malformed_cancellation_is_refused(port: int, body: dict) -> None:
    status, reply = post_json(port, "/exec/cancel", body)
    assert (status, reply["error"]) == (400, "exec_failed")


@pytest.mark.parametrize("literal", ["NaN", "Infinity", "-Infinity"])
def test_a_non_finite_timeout_is_refused(port: int, literal: str) -> None:
    import http.client

    conn = http.client.HTTPConnection("127.0.0.1", port, timeout=10)
    try:
        conn.request("POST", "/exec", body=f'{{"command": "true", "timeout": {literal}}}')
        response = conn.getresponse()
        status, reply = response.status, json.loads(response.read())
    finally:
        conn.close()
    assert (status, reply["error"]) == (400, "exec_failed")
