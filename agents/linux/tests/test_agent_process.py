"""Controls that run the agent as its own process, through its `main` entry.

Each test names its row of the required-control table in
docs/specs/guest-command-lifecycle.md, section "Test seams and required
controls", and its kind (see tests/test_exec_lifecycle.py). These controls use
only the HTTP interface and the entry point, so the old-fail controls run
unchanged against the agent before leaf k220.
"""

from __future__ import annotations

import json
import socket
import struct
import threading
import time
from collections.abc import Iterator
from pathlib import Path
from typing import Any

import pytest

from tests.exec_validity import record_errors, stream_bytes
from tests.http_agent import (
    AgentProcess,
    get_health,
    post_json,
    post_request,
    raw_exchange,
    timed,
)
from tests.processes import child_survives, kill_if_same, wait_for_child

pytestmark = pytest.mark.linux_kernel

HEALTH_BOUND_S = 1.0  # spec, section "Bounds": health during a running command
SHUTDOWN_BOUND_S = 4.0  # the two root bounds of 2 s, from SIGTERM to the last record
EXIT_MARGIN_S = 0.5  # interpreter exit and process reaping after the last record


@pytest.fixture
def agent(tmp_path: Path) -> Iterator[AgentProcess]:
    process = AgentProcess(tmp_path / "captures")
    try:
        yield process
    finally:
        process.stop()


class Background:
    """One request on its own thread, so the test can act while it runs."""

    def __init__(self, call: Any, *args: Any) -> None:
        self.result: Any = None
        self.error: BaseException | None = None
        self._thread = threading.Thread(target=self._run, args=(call, *args), daemon=True)
        self._thread.start()

    def _run(self, call: Any, *args: Any) -> None:
        try:
            self.result = call(*args)
        except BaseException as e:  # the test inspects every failure
            self.error = e

    def join(self, limit: float) -> Any:
        self._thread.join(limit)
        assert not self._thread.is_alive(), "the background request did not end"
        return self.result


# ---- Responsive agent during a command (old-fail) -------------------------


def test_health_exec_and_download_reply_during_a_command(
    agent: AgentProcess, tmp_path: Path
) -> None:
    """Old path: `diagnostic:linux-disconnect-container` (k6). The old agent
    served one request at a time, so health waited for the running command
    until its deadline. Each request here has its own 1 s client deadline."""
    pid_file = tmp_path / "leader.pid"
    partial = tmp_path / "partial.txt"
    partial.write_bytes(b"partial evidence\n")
    command = f"echo $$ > {pid_file}; exec sleep 30"
    running = Background(post_json, agent.port, "/exec", {"command": command, "timeout": 3})
    leader = wait_for_child(pid_file)
    try:
        elapsed, (status, health) = timed(get_health, agent.port, HEALTH_BOUND_S)
        assert status == 200 and health["platform"] == "linux"
        assert elapsed < HEALTH_BOUND_S
        elapsed, (status, reply) = timed(
            post_json, agent.port, "/exec", {"command": "printf ok", "timeout": 5}, HEALTH_BOUND_S
        )
        assert (status, reply["stdout"]) == (200, "ok")
        elapsed, raw = timed(
            raw_exchange, agent.port, post_request(f"/download?path={partial}"), HEALTH_BOUND_S
        )
        assert (raw.status, raw.body, raw.extra) == (200, b"partial evidence\n", b"")
        assert elapsed < HEALTH_BOUND_S
        assert child_survives(*leader), "a request ended the running command"
    finally:
        running.join(10)
        kill_if_same(*leader)


# ---- Download never sends extra bytes (old-fail) ---------------------------


def test_download_never_sends_bytes_past_the_declared_length(agent: AgentProcess) -> None:
    """Old path: `diagnostic:linux-download-consistency-container` (k14). The
    old agent read to end-of-file, not to the length that `fstat` gave. A
    procfs file reports size 0 and still reads bytes, which is the same growth
    without a race. The new agent sends exactly the declared 0 bytes."""
    raw = raw_exchange(agent.port, post_request("/download?path=/proc/self/status"))
    assert raw.status == 200
    assert raw.declared == 0
    assert raw.extra == b"", f"{len(raw.extra)} bytes followed the declared length"


# ---- Disconnect changes nothing, record retained (corrected-pass) ----------


def _start_and_drop(port: int, body: dict[str, Any], how: str) -> None:
    s = socket.create_connection(("127.0.0.1", port), timeout=5)
    s.sendall(post_request("/exec", json.dumps(body).encode()))
    time.sleep(0.3)
    if how == "close":
        s.close()
    elif how == "reset":
        # SO_LINGER with a zero time makes the close send RST (socket(7)).
        s.setsockopt(socket.SOL_SOCKET, socket.SO_LINGER, struct.pack("ii", 1, 0))
        s.close()
    else:
        s.shutdown(socket.SHUT_WR)
        s.settimeout(10)
        s.recv(1)  # keep reading on the half-closed connection
        s.close()


@pytest.mark.parametrize("how", ["close", "reset", "half-close"])
def test_disconnect_changes_nothing_and_the_record_is_retained(
    agent: AgentProcess, how: str
) -> None:
    """The old-fail case of this row is macOS (k13). The command runs to its
    outcome after the client drops the connection. A later cancellation of the
    finished id returns the retained record and changes nothing."""
    exec_id = f"drop-{how}"
    body = {"command": "sleep 0.6; printf done", "timeout": 10, "id": exec_id}
    dropper = threading.Thread(target=_start_and_drop, args=(agent.port, body, how), daemon=True)
    dropper.start()
    time.sleep(2.0)
    status, reply = post_json(agent.port, "/exec/cancel", {"id": exec_id})
    dropper.join(10)
    assert (status, reply["state"]) == (200, "terminal")
    record = reply["record"]
    assert record_errors(record, exec_id=exec_id) == []
    assert record["outcome"] == {"kind": "exited", "exitCode": 0}
    assert stream_bytes(record, "stdout") == b"done"
    status, again = post_json(agent.port, "/exec/cancel", {"id": exec_id})
    assert again == reply


# ---- Agent shutdown at SIGTERM (new behaviour) -----------------------------


def test_sigterm_during_a_command_ends_the_group_and_exits_in_bounds(
    agent: AgentProcess, tmp_path: Path
) -> None:
    pid_file = tmp_path / "child.pid"
    command = f"sleep 120 & echo $! > {pid_file}; wait"
    running = Background(
        post_json, agent.port, "/exec", {"command": command, "timeout": 60, "id": "at-sigterm"}
    )
    child = wait_for_child(pid_file)
    try:
        exit_after = agent.terminate_and_time(SHUTDOWN_BOUND_S + 5)
        assert exit_after is not None, "the agent did not exit"
        assert exit_after < SHUTDOWN_BOUND_S + EXIT_MARGIN_S
        assert agent.process.returncode == 0, agent.output()
        assert not child_survives(*child), "the command group outlived the agent"
        running.join(5)
        if running.result is not None:
            status, record = running.result
            assert status == 200
            assert record_errors(record, exec_id="at-sigterm") == []
            assert record["outcome"] == {"kind": "cancelled", "beforeStart": False}
            assert record["termination"]["groupEmpty"] == "yes"
    finally:
        kill_if_same(*child)


def test_sigterm_does_not_wait_for_a_leader_whose_status_stays_unavailable(
    tmp_path: Path,
) -> None:
    """The seam module hides every leader exit from the owner, so the status
    stays unavailable past K + 1.4 s (spec, section "Agent shutdown")."""
    agent = AgentProcess(tmp_path / "captures", seam_module="tests.seam_hidden_exit")
    try:
        leader_file = tmp_path / "leader.pid"
        command = f"echo $$ > {leader_file}; exec sleep 120"
        running = Background(
            post_json, agent.port, "/exec", {"command": command, "timeout": 60, "id": "held"}
        )
        leader = wait_for_child(leader_file)
        exit_after = agent.terminate_and_time(SHUTDOWN_BOUND_S + 5)
        assert exit_after is not None, "the agent did not exit"
        assert exit_after < SHUTDOWN_BOUND_S + EXIT_MARGIN_S
        assert not child_survives(*leader), "the leader outlived SIGKILL"
        running.join(5)
        if running.result is not None:
            status, record = running.result
            assert record_errors(record, exec_id="held") == []
            assert record["termination"]["leaderStatus"] == {"kind": "unavailable"}
    finally:
        agent.stop()
