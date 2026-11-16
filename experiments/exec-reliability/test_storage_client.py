"""Tests for storage_client.py against the real writer and against faulted writers.

The contract is docs/specs/diagnostic-evidence-storage.md: "Ownership",
P0 and P1 in "Phases and bounds", and "Channel contract".
"""

from __future__ import annotations

import errno
import json
import os
import signal
import subprocess
import sys
import time
from collections.abc import Callable, Iterator
from pathlib import Path

import pytest

from storage_client import (
    ACK_BUDGET,
    OUTBOX_CAP,
    READINESS_CEILING,
    REAP_GRACE,
    WRITE_BUDGET,
    WRITER,
    StorageClient,
)
from turns import Turn, run_turns
from writer import MAX_PAYLOAD

HERE = Path(__file__).resolve().parent
FAULTS = HERE / "writer_faults.py"
PLAN = b'{"run_id": "run"}\n'
# Scheduling slack for bounds measured on this host.
TOLERANCE = 0.5
# A turn against a stopped writer does bounded work, far below this.
TURN_LIMIT = 0.1
# A fake writer that acknowledges the setup frame, then closes its frame pipe
# and stays alive. The client's next frame write then reports EPIPE.
CLOSES_INPUT = """
import json, os, struct, time
header_length, _ = struct.unpack(">II", os.read(0, 8))
header = json.loads(os.read(0, header_length))
line = {"seq": header["seq"], "op": "setup", "status": "ok", "run_dir": "/", "subjects": {}}
os.write(1, json.dumps(line).encode() + b"\\n")
os.close(0)
time.sleep(60)
"""

# A fake writer that leaves a grandchild holding the acknowledgement pipe and
# exits at once. The grandchild ends by itself after three seconds.
HOLDS_ACK_PIPE = """
import os, subprocess, sys
subprocess.Popen([sys.executable, "-c", "import time; time.sleep(3)"], stdout=1)
os._exit(70)
"""


def faulty(*faults: str) -> tuple[str, ...]:
    return (sys.executable, "-I", str(FAULTS), *faults)


def wait_for_zombie(pid: int, timeout: float = 5.0) -> None:
    """Wait until the child exits, without reaping it. macOS has no waitid."""
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        state = subprocess.run(
            ["ps", "-o", "stat=", "-p", str(pid)], capture_output=True, text=True, check=False
        ).stdout.strip()
        if state.startswith("Z"):
            return
        time.sleep(0.01)
    raise AssertionError(f"process {pid} did not exit within {timeout:g} s")


def until(predicate: Callable[[], bool], client: StorageClient) -> None:
    run_turns(client.turn(predicate))


@pytest.fixture
def clients() -> Iterator[list[StorageClient]]:
    made: list[StorageClient] = []
    yield made
    # Teardown uses the client's own exact-PID kill and reap.
    for client in made:
        client.kill()
        until(lambda: False, client)
        assert client.pid is None or client.reaped_at is not None, "a writer stayed unreaped"


def make(clients: list[StorageClient], *args: object, **kwargs: object) -> StorageClient:
    client = StorageClient(*args, **kwargs)  # type: ignore[arg-type]
    clients.append(client)
    return client


def ready(clients: list[StorageClient], root: Path, **kwargs: object) -> StorageClient:
    client = make(clients, **kwargs)
    assert client.start(str(root), "run", PLAN, [str(HERE / "writer.py")]), client.readiness_reason
    return client


def assert_killed_and_reaped(client: StorageClient) -> None:
    assert client.killed_at is not None, "the writer received no KILL"
    assert client.reaped_at is not None, "the writer was not reaped"
    assert client.reaped_at - client.killed_at <= REAP_GRACE, "the reap took longer than 2 s"
    assert client.residual is False


# Defaults and spawn -----------------------------------------------------------


def test_defaults_are_the_real_writer_and_the_spec_bounds():
    client = StorageClient()
    assert client.command == WRITER
    assert Path(client.command[-1]).name == "writer.py"
    assert client.readiness_bound == READINESS_CEILING == 5.0
    assert client.outbox_cap == OUTBOX_CAP == 64 * 1024 * 1024
    assert (WRITE_BUDGET, ACK_BUDGET, REAP_GRACE) == (256 * 1024, 64 * 1024, 2.0)


@pytest.mark.parametrize("bound", [0.0, -1.0, READINESS_CEILING + 0.1])
def test_readiness_bound_above_the_ceiling_is_refused(bound):
    with pytest.raises(ValueError):
        StorageClient(readiness_bound=bound)


def test_spawn_error_is_a_readiness_failure_without_a_writer(clients, tmp_path):
    client = make(clients, (str(tmp_path / "missing-writer"),))
    assert client.start(str(tmp_path), "run", PLAN, []) is False
    assert client.storage == "unavailable"
    assert client.readiness_reason == "spawn_error"
    assert client.spawn_error is not None
    assert client.pid is None and client.killed_at is None


# Healthy exchange against the real writer -------------------------------------


def test_real_writer_reaches_readiness_and_applies_queued_records(clients, tmp_path):
    client = ready(clients, tmp_path)
    assert client.storage == "ready"
    ack = client.readiness_ack
    assert ack is not None and ack["status"] == "ok"
    run_dir = Path(str(ack["run_dir"]))
    assert run_dir == (tmp_path / "run").resolve()
    assert client.ready_at is not None and client.spawned_at is not None
    assert client.ready_at - client.spawned_at <= READINESS_CEILING

    big = os.urandom(MAX_PAYLOAD)
    sequences = [
        client.enqueue("create", "intent-1.json", b'{"identity": "vm"}'),
        client.enqueue("create", "workload.stdout", big),
        client.enqueue("append", "workload.stdout", big),
    ]
    close = client.request_close()
    assert all(seq is not None for seq in sequences)
    until(lambda: client.reaped_at is not None, client)

    for seq in sequences:
        reply = client.ack(seq)  # type: ignore[arg-type]
        assert reply is not None and reply["status"] == "ok", reply
    assert close is not None
    closed = client.ack(close)
    assert closed is not None and closed["status"] == "ok", closed
    assert {"identity", "storage_close"} <= closed.keys()
    assert client.storage == "closed"
    assert client.exit_status == 0
    assert client.first_failure is None
    assert client.killed_at is None, "an orderly close needs no KILL"
    assert (run_dir / "workload.stdout").read_bytes() == big + big
    assert client.drops == {} and client.abandoned == {}


def test_close_acknowledgement_survives_a_reap_in_the_same_turn(clients, tmp_path):
    # The writer acknowledges close and exits before the next turn. The turn
    # reaps it first and must still read the close acknowledgement.
    client = ready(clients, tmp_path)
    assert client.pid is not None
    close = client.request_close()
    while client.queued:
        client.advance()
    wait_for_zombie(client.pid)
    client.advance()
    assert client.reaped_at is not None
    assert close is not None and client.ack(close) is not None
    assert client.storage == "closed", client.first_failure
    assert client.exit_status == 0


def test_readiness_waits_as_scheduler_turns(clients, tmp_path):
    # Another turn keeps running while readiness is pending.
    client = make(clients, faulty("stall:@start"), readiness_bound=0.5)
    assert client.spawn()
    client.request_readiness(str(tmp_path), "run", PLAN, [])
    ticks: list[float] = []

    def tick() -> float | None:
        ticks.append(time.monotonic())
        return None if client.readiness != "pending" else time.monotonic() + 0.05

    run_turns(client.turn(lambda: client.readiness != "pending"), Turn(tick))
    assert client.readiness == "failed"
    assert len(ticks) >= 5, f"the other turn ran only {len(ticks)} times"


# Readiness failures -------------------------------------------------------------


@pytest.mark.parametrize("stall", ["@start", "plan.json", "subjects-before.json"])
def test_missing_readiness_kills_and_reaps_within_the_bounds(clients, tmp_path, stall):
    bound = 0.5
    client = make(clients, faulty(f"stall:{stall}"), readiness_bound=bound)
    started = time.monotonic()
    assert client.start(str(tmp_path), "run", PLAN, [str(HERE / "writer.py")]) is False
    assert time.monotonic() - started <= bound + REAP_GRACE + TOLERANCE
    assert client.storage == "unavailable"
    assert client.readiness_reason == "timeout"
    assert client.spawned_at is not None and client.killed_at is not None
    late = client.killed_at - (client.spawned_at + bound)
    assert 0 <= late <= TOLERANCE, f"KILL came {late:+.3f} s after the readiness bound"
    assert_killed_and_reaped(client)
    assert client.exit_status == -signal.SIGKILL


def test_failed_setup_acknowledgement_kills_and_reaps(clients, tmp_path):
    (tmp_path / "run").mkdir()
    client = make(clients)
    assert client.start(str(tmp_path), "run", PLAN, []) is False
    assert client.storage == "unavailable"
    assert client.readiness_reason == "failed"
    assert client.acks[1]["errno"] == errno.EEXIST
    assert_killed_and_reaped(client)


def test_writer_exit_before_readiness_is_unavailable(clients, tmp_path):
    client = make(clients, faulty("exit:@start"))
    assert client.start(str(tmp_path), "run", PLAN, []) is False
    assert client.storage == "unavailable"
    assert client.readiness_reason in ("writer_exit", "ack_eof")
    assert client.reaped_at is not None
    assert client.residual is False


def test_queued_frames_are_abandoned_after_readiness_failure(clients, tmp_path):
    client = make(clients, faulty("stall:@start"), readiness_bound=0.3)
    assert client.spawn()
    client.request_readiness(str(tmp_path), "run", PLAN, [])
    # Frames that reached the pipe buffer left the outbox. These frames are
    # larger than any pipe buffer, so the later ones stay queued.
    for index in range(4):
        assert client.enqueue("create", f"stream-{index}", b"s" * MAX_PAYLOAD) is not None
    until(lambda: client.readiness != "pending", client)
    assert client.storage == "unavailable"
    assert client.abandoned.get("stream-3") == 1
    assert set(client.abandoned) <= {f"stream-{index}" for index in range(4)}
    assert client.queued == 0 and client.outbox_bytes == 0
    assert client.enqueue("create", "late.json", b"{}") is None
    assert client.abandoned["late.json"] == 1


# A stopped writer never blocks a turn -----------------------------------------


def test_stopped_writer_never_blocks_a_turn(clients, tmp_path):
    client = ready(clients, tmp_path)
    assert client.pid is not None
    os.kill(client.pid, signal.SIGSTOP)
    payload = b"x" * MAX_PAYLOAD
    for index in range(8):
        assert client.enqueue("create", f"stream-{index}", payload) is not None
    durations: list[float] = []
    written: list[int] = []
    for _ in range(50):
        started = time.monotonic()
        client.advance()
        durations.append(time.monotonic() - started)
        written.append(client.turn_written)
    assert max(durations) < TURN_LIMIT, f"the slowest turn took {max(durations):.3f} s"
    assert max(written) <= WRITE_BUDGET
    assert written[-1] == 0, "the pipe never filled, so EAGAIN was not reached"
    assert client.queued > 0, "EAGAIN dropped the unwritten frames"
    assert client.storage == "ready"
    assert client.drops == {}


def test_eagain_keeps_a_partial_frame_until_the_writer_resumes(clients, tmp_path):
    client = ready(clients, tmp_path)
    assert client.pid is not None
    os.kill(client.pid, signal.SIGSTOP)
    payload = os.urandom(MAX_PAYLOAD)
    seq = client.enqueue("create", "large.bin", payload)
    for _ in range(10):
        client.advance()
    assert client.queued == 1, "the partial frame left the outbox"
    os.kill(client.pid, signal.SIGCONT)
    until(lambda: client.ack(seq) is not None, client)  # type: ignore[arg-type]
    assert client.ack(seq)["status"] == "ok"  # type: ignore[arg-type,index]
    assert client.readiness_ack is not None
    run_dir = Path(str(client.readiness_ack["run_dir"]))
    assert (run_dir / "large.bin").read_bytes() == payload


def test_kill_reaps_a_stopped_writer_within_the_grace(clients, tmp_path):
    client = ready(clients, tmp_path)
    assert client.pid is not None
    os.kill(client.pid, signal.SIGSTOP)
    client.kill()
    until(lambda: client.reaped_at is not None, client)
    assert_killed_and_reaped(client)
    assert client.storage == "failed"
    assert client.first_failure in ("writer_exit", "ack_eof")


def test_kill_uses_the_given_reap_deadline(clients, tmp_path):
    client = ready(clients, tmp_path)
    deadline = time.monotonic() + 0.5
    client.kill(deadline)
    assert client.reap_deadline == deadline
    until(lambda: client.reaped_at is not None, client)
    assert_killed_and_reaped(client)


def test_expected_exit_without_an_exit_becomes_a_residual_without_kill(clients, tmp_path):
    client = ready(clients, tmp_path)
    assert client.pid is not None
    pid = client.pid
    os.kill(pid, signal.SIGSTOP)
    deadline = time.monotonic() + 0.2
    client.expect_exit(deadline)
    started = time.monotonic()
    until(lambda: True, client)
    assert time.monotonic() - started <= 0.2 + TOLERANCE
    assert client.residual is True
    assert client.killed_at is None, "an expected exit sent KILL"
    assert client.reaped_at is None
    assert client._frame_fd is None and client._ack_fd is None
    # The residual writer is outside the client now, so the test ends it.
    os.kill(pid, signal.SIGKILL)
    os.waitpid(pid, 0)
    clients.remove(client)


# Per-turn budgets ---------------------------------------------------------------


class AcceptingClient(StorageClient):
    """A client whose frame writes accept every byte, to measure the budget."""

    calls: list[int]

    def _write(self, fd: int, data: memoryview) -> int:  # type: ignore[override]
        self.calls.append(len(data))
        return len(data)


def test_frame_writes_stop_at_the_turn_budget(clients):
    client = AcceptingClient(command=faulty("stall:@start"))
    client.calls = []
    clients.append(client)
    assert client.spawn()
    for index in range(3):
        assert client.enqueue("create", f"part-{index}", b"y" * MAX_PAYLOAD) is not None
    total = client.outbox_bytes
    turns: list[int] = []
    while client.queued:
        client.advance()
        turns.append(client.turn_written)
    assert max(turns) == WRITE_BUDGET
    assert sum(turns) == total
    assert all(size <= WRITE_BUDGET for size in client.calls)


class FloodedClient(StorageClient):
    """A client whose acknowledgement reads always return a full buffer."""

    def _read(self, fd: int, count: int) -> bytes:  # type: ignore[override]
        return b" " * count


def test_acknowledgement_reads_stop_at_the_turn_budget(clients):
    client = FloodedClient(command=faulty("stall:@start"))
    clients.append(client)
    assert client.spawn()
    client.advance()
    assert client.turn_read == ACK_BUDGET
    client.advance()
    assert client.turn_read == ACK_BUDGET
    # The flood never drains, so teardown reads through the real call.
    client._read = os.read  # type: ignore[method-assign]


def test_reaped_writer_settles_when_another_process_holds_the_ack_pipe(clients, tmp_path):
    # The holder is a sleeping grandchild of the wrapper, so ack EOF never comes.
    client = make(clients, (sys.executable, "-I", "-c", HOLDS_ACK_PIPE))
    started = time.monotonic()
    assert client.start(str(tmp_path), "run", PLAN, []) is False
    assert time.monotonic() - started < 1.0, "the turn waited for an ack EOF"
    assert client.readiness_reason == "writer_exit"
    assert client.exit_status == 70
    assert client._ack_fd is None and client._frame_fd is None


# Broken pipes and protocol errors ---------------------------------------------


def test_epipe_marks_storage_failed_and_abandons_the_outbox(clients, tmp_path):
    client = make(clients, (sys.executable, "-I", "-c", CLOSES_INPUT))
    assert client.start(str(tmp_path), "run", PLAN, [])
    assert client.enqueue("create", "intent-1.json", b"{}") is not None
    assert client.enqueue("create", "workload.stdout", b"out") is not None
    until(lambda: client.storage != "ready", client)
    assert client.storage == "failed"
    assert client.first_failure == "EPIPE"
    assert client.abandoned == {"intent-1.json": 1, "workload.stdout": 1}
    assert client.reaped_at is None, "EPIPE came from the open pipe, not from a reap"
    assert client.enqueue("create", "later.json", b"{}") is None
    assert client.abandoned["later.json"] == 1


def test_error_ack_after_readiness_fails_storage_with_its_errno(clients, tmp_path):
    client = make(clients, faulty("deny:intent-1.json"))
    assert client.start(str(tmp_path), "run", PLAN, [])
    seq = client.enqueue("create", "intent-1.json", b"{}")
    assert seq is not None
    deadline = time.monotonic() + 2.0
    until(lambda: client.storage != "ready" or time.monotonic() >= deadline, client)
    assert client.storage == "failed"
    assert client.first_failure == "EACCES"
    assert client.failure_ack is not None
    assert client.failure_ack["seq"] == seq and client.failure_ack["errno"] == errno.EACCES
    assert client.enqueue("create", "later.json", b"{}") is None
    assert client.abandoned["later.json"] == 1
    # The failure closed the frame pipe, so the writer exits without a close.
    deadline = time.monotonic() + 2.0
    until(lambda: client.reaped_at is not None or time.monotonic() >= deadline, client)
    assert client.exit_status is not None and client.exit_status != 0


def test_writer_death_after_readiness_marks_storage_failed(clients, tmp_path):
    client = make(clients, faulty("exit:intent-1.json"))
    assert client.start(str(tmp_path), "run", PLAN, [])
    assert client.enqueue("create", "intent-1.json", b"{}") is not None
    until(lambda: client.reaped_at is not None, client)
    assert client.storage == "failed"
    assert client.first_failure in ("writer_exit", "ack_eof")
    assert client.exit_status == 70
    assert client.killed_at is None


def lose_writer(client: StorageClient) -> None:
    """Kill and reap the writer outside the client, so its next wait fails."""
    assert client.pid is not None
    os.kill(client.pid, signal.SIGKILL)
    os.waitpid(client.pid, 0)


def record_signals(monkeypatch: pytest.MonkeyPatch) -> list[tuple[int, int]]:
    sent: list[tuple[int, int]] = []
    monkeypatch.setattr(os, "kill", lambda pid, sig: sent.append((pid, sig)))
    return sent


def assert_lost_and_quiet(
    clients: list[StorageClient], client: StorageClient, sent: list[tuple[int, int]]
) -> None:
    """A lost writer gets no signal, and later turns stay bounded."""
    client.kill()
    for _ in range(3):
        started = time.monotonic()
        until(lambda: False, client)
        assert time.monotonic() - started < TURN_LIMIT
    assert sent == [], "the client signalled a PID it no longer owns"
    assert client.residual is True
    assert client.killed_at is None and client.reaped_at is None
    assert client.reap_error is not None and "ChildProcessError" in client.reap_error
    assert client._frame_fd is None and client._ack_fd is None
    assert client.queued == 0 and client.outbox_bytes == 0
    # The test reaped this writer itself, so the fixture has nothing to reap.
    clients.remove(client)


def test_writer_lost_during_readiness_receives_no_kill(clients, tmp_path, monkeypatch):
    client = make(clients, faulty("stall:@start"), readiness_bound=5.0)
    assert client.spawn()
    client.request_readiness(str(tmp_path), "run", PLAN, [])
    # These frames are larger than any pipe buffer, so the later ones stay queued.
    for index in range(4):
        assert client.enqueue("create", f"stream-{index}", b"s" * MAX_PAYLOAD) is not None
    lose_writer(client)
    sent = record_signals(monkeypatch)
    until(lambda: client.readiness != "pending", client)
    assert client.storage == "unavailable"
    assert client.readiness_reason == "writer_lost"
    assert client.abandoned.get("stream-3") == 1
    assert client.enqueue("create", "late.json", b"{}") is None
    assert client.abandoned["late.json"] == 1
    assert_lost_and_quiet(clients, client, sent)


def test_writer_lost_after_readiness_receives_no_kill(clients, tmp_path, monkeypatch):
    client = ready(clients, tmp_path)
    lose_writer(client)
    sent = record_signals(monkeypatch)
    until(lambda: client.storage != "ready", client)
    assert client.storage == "failed"
    assert client.first_failure == "writer_lost"
    assert client.enqueue("create", "late.json", b"{}") is None
    assert client.abandoned["late.json"] == 1
    assert_lost_and_quiet(clients, client, sent)


def test_unknown_acknowledgement_is_a_protocol_failure(clients, tmp_path):
    client = ready(clients, tmp_path)
    client._ack_buffer += b'{"seq": 99, "op": "create", "status": "ok"}\n'
    client._parse_acks()
    assert client.storage == "failed"
    assert client.first_failure == "protocol"


# Outbox cap -----------------------------------------------------------------------


def test_cap_drops_frames_and_counts_them_by_record_name():
    client = StorageClient(outbox_cap=4096)
    assert client.enqueue("create", "a.json", b"a" * 3000) == 1
    assert client.enqueue("append", "a.json", b"a" * 3000) is None
    assert client.enqueue("create", "b.json", b"b" * 3000) is None
    assert client.enqueue("create", "c.json", b"c" * 500) == 2
    assert client.drops == {"a.json": 1, "b.json": 1}
    assert client.outbox_bytes <= 4096


def test_full_default_outbox_drops_without_blocking(clients, tmp_path):
    client = ready(clients, tmp_path)
    assert client.pid is not None
    os.kill(client.pid, signal.SIGSTOP)
    payload = b"z" * MAX_PAYLOAD
    started = time.monotonic()
    queued = 0
    while client.enqueue("create", f"chunk-{queued}", payload) is not None:
        queued += 1
    assert client.enqueue("append", "chunk-0", payload) is None
    assert time.monotonic() - started < 2.0, "enqueue waited for room"
    assert client.outbox_bytes <= OUTBOX_CAP
    # Each frame is the payload plus a small header, so 63 frames fit in 64 MiB.
    assert queued == OUTBOX_CAP // MAX_PAYLOAD - 1
    assert client.drops == {f"chunk-{queued}": 1, "chunk-0": 1}


def test_oversized_payload_is_refused_at_enqueue():
    with pytest.raises(ValueError):
        StorageClient().enqueue("create", "big", b"x" * (MAX_PAYLOAD + 1))


# Descriptors ----------------------------------------------------------------------


def test_writer_receives_only_standard_descriptors_and_its_run_directory(clients, tmp_path):
    report = tmp_path / "descriptors.jsonl"
    leaked_read, leaked_write = os.pipe()
    os.set_inheritable(leaked_read, True)
    os.set_inheritable(leaked_write, True)
    try:
        client = ready(clients, tmp_path / "evidence", command=faulty(f"report:{report}"))
    finally:
        os.close(leaked_read)
        os.close(leaked_write)
    assert client._frame_fd is not None and client._ack_fd is not None
    assert not os.get_inheritable(client._frame_fd)
    assert not os.get_inheritable(client._ack_fd)
    assert os.get_blocking(client._frame_fd) is False
    assert os.get_blocking(client._ack_fd) is False

    snapshots = [json.loads(line) for line in report.read_text().splitlines()]
    start = snapshots[0]
    assert start["point"] == "start"
    assert start["descriptors"] == {"0": "fifo", "1": "fifo", "2": "null"}
    setup = snapshots[1]
    assert setup["point"] == "ack"
    assert sorted(setup["descriptors"].values()) == ["directory", "fifo", "fifo", "null"]
