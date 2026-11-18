"""Lifecycle controls of the Linux exec owner.

Each test names its row of the required-control table in
docs/specs/guest-command-lifecycle.md, section "Test seams and required
controls", and its kind:

- old-fail: the old path fails the control, and the new code passes it.
- corrected-pass: the new code passes. The old failure is on another agent.
- new behaviour: no old path exists. The new code passes.

Every record that a control receives must also pass the validity rules.
"""

from __future__ import annotations

import base64
import json
import os
import signal
import threading
import time
from pathlib import Path
from typing import BinaryIO

import pytest

from testanyware_agent import exec_owner
from testanyware_agent.exec_owner import OwnerSeams, PidfdLeaderWaiter
from tests.exec_validity import record_errors, stream_bytes
from tests.limits import with_one_process_limit

pytestmark = pytest.mark.linux_kernel

ORACLE_STDOUT = bytes.fromhex("4578706c61696e2074686973206368616e67652e")
ORACLE_STDERR = bytes.fromhex("7365706172617465207374646572720a")
ORACLE_BINARY = bytes.fromhex("00ff80410a")
ORACLE_COMMAND = "printf 'Explain this change.'; printf 'separate stderr\\n' >&2"


@pytest.fixture(autouse=True)
def capture_root(tmp_path: Path, monkeypatch: pytest.MonkeyPatch) -> Path:
    root = tmp_path / "captures"
    monkeypatch.setenv(exec_owner.CAPTURE_ROOT_ENV, str(root))
    return root


def run(command: str, timeout: float, seams: OwnerSeams | None = None) -> dict:
    record = exec_owner.run_command(
        command, timeout, "ctl-1", time.monotonic(), seams or exec_owner.DEFAULT_SEAMS
    ).to_dict()
    assert record_errors(record, exec_id="ctl-1") == []
    return record


def start_ticks(pid: int) -> str | None:
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return None
    return text[text.rindex(")") + 2 :].split()[19]


def process_state(pid: int) -> str | None:
    try:
        text = Path(f"/proc/{pid}/stat").read_text()
    except OSError:
        return None
    return text[text.rindex(")") + 2 :].split()[0]


def read_child(path: Path) -> tuple[int, str]:
    pid = int(path.read_text().split()[0])
    ticks = start_ticks(pid)
    assert ticks is not None, "the child had already gone when the test read it"
    return pid, ticks


def child_survives(pid: int, ticks: str) -> bool:
    """True when the same process (PID and start time) still runs."""
    return start_ticks(pid) == ticks and process_state(pid) != "Z"


class ChildRecorder:
    """Read the child PID file while the command runs, before the owner ends it."""

    def __init__(self, path: Path) -> None:
        self.path = path
        self.child: tuple[int, str] | None = None
        self._thread = threading.Thread(target=self._watch, daemon=True)
        self._thread.start()

    def _watch(self) -> None:
        end = time.monotonic() + 1.5
        while time.monotonic() < end:
            try:
                if self.path.read_text().strip():
                    self.child = read_child(self.path)
                    return
            except (OSError, ValueError, AssertionError):
                pass
            time.sleep(0.01)

    def result(self) -> tuple[int, str]:
        self._thread.join(2)
        assert self.child is not None, "the command never wrote its child PID"
        return self.child


# ---- Group ends after a timeout (old-fail) --------------------------------


def test_group_ends_after_a_timeout_fork_and_wait(tmp_path: Path) -> None:
    """Old path: `diagnostic:linux-timeout-lifecycle-old-path` (k5 issue-fork-wait).

    The old agent killed only the leader. The background `sleep 120` outlived
    the reply. The owner ends the whole group and reports it empty.
    """
    pid_file = tmp_path / "child.pid"
    recorder = ChildRecorder(pid_file)
    started = time.monotonic()
    record = run(f"sleep 120 & p=$!; echo $p > {pid_file}; wait", 2)
    elapsed = time.monotonic() - started
    child_pid, child_ticks = recorder.result()

    assert record["outcome"] == {"kind": "timed_out", "timeoutSeconds": 2}
    termination = record["termination"]
    assert termination["groupEmpty"] == "yes"
    assert termination["withinBounds"] is True
    assert not child_survives(child_pid, child_ticks)
    assert elapsed < 2 + 4 + 0.5
    assert (record["exitCode"], record["stdout"], record["stderr"]) == (
        -1,
        "",
        "Process timed out",
    )


# ---- Grace and SIGKILL bounds (corrected-pass) -----------------------------


def test_child_that_ignores_sigterm_receives_sigkill_after_the_grace(tmp_path: Path) -> None:
    """The old-fail case of this row is macOS (k3). This is the Linux pass."""
    pid_file = tmp_path / "child.pid"
    recorder = ChildRecorder(pid_file)
    record = run(f"trap '' TERM; sleep 30 & p=$!; echo $p > {pid_file}; wait", 2)
    child_pid, child_ticks = recorder.result()

    termination = record["termination"]
    assert termination["termSentAfterMs"] <= 100
    assert 1800 <= termination["killSentAfterMs"] <= 2000
    assert termination["leaderStatus"] == {"kind": "signalled", "signal": signal.SIGKILL}
    assert termination["leaderExitAfterMs"] >= termination["killSentAfterMs"]
    assert termination["groupEmpty"] == "yes"
    assert termination["withinBounds"] is True
    assert record["lifecycle"]["finalisedAfterMs"] - termination["killSentAfterMs"] <= 2000
    assert not child_survives(child_pid, child_ticks)


def test_leader_that_ends_on_sigterm_gets_sigkill_at_its_exit() -> None:
    record = run("sleep 30", 1)
    termination = record["termination"]
    assert termination["leaderStatus"] == {"kind": "signalled", "signal": signal.SIGTERM}
    assert termination["killSentAfterMs"] <= 200
    assert termination["withinBounds"] is True


class OrderLog:
    """The signal operations of the owner and its reap, in the order they ran.

    Each signal entry also records whether the leader PID still named a
    process (running or an unreaped zombie) at the moment of the signal.
    """

    events: list[tuple[str, int, bool]] = []


class ReapRecordingWaiter(PidfdLeaderWaiter):
    def reap(self):  # type: ignore[no-untyped-def]
        status = super().reap()
        OrderLog.events.append(("reap", 0, False))
        return status


@pytest.mark.parametrize(
    ("command", "timeout", "terminated"),
    [
        pytest.param("sleep 30", 1, True, id="leader-exits-before-sigkill"),
        pytest.param("trap '' TERM; sleep 30", 1, True, id="leader-exits-after-sigkill"),
        pytest.param("exit 3", 10, False, id="normal-exit"),
    ],
)
def test_no_nonzero_signal_reaches_the_group_after_the_reap(
    command: str, timeout: float, terminated: bool, monkeypatch: pytest.MonkeyPatch
) -> None:
    """The tombstone order (spec, section "Termination after a timeout or a
    cancellation", steps 3 to 6): every SIGTERM and SIGKILL goes to the group
    while the leader is unreaped, and after the reap only signal-0 probes go to
    the group ID. A reaped leader no longer pins the group ID, so a later
    nonzero signal could reach a reused group."""
    OrderLog.events = []
    real_killpg = os.killpg

    def recording_killpg(group_id: int, signum: int) -> None:
        OrderLog.events.append(("signal", signum, process_state(group_id) is not None))
        real_killpg(group_id, signum)

    monkeypatch.setattr(exec_owner.os, "killpg", recording_killpg)
    record = run(command, timeout, OwnerSeams(leader_waiter=ReapRecordingWaiter))
    monkeypatch.undo()

    events = OrderLog.events
    assert [e for e in events if e[0] == "reap"] == [("reap", 0, False)]
    reap_at = events.index(("reap", 0, False))
    before, after = events[:reap_at], events[reap_at + 1 :]
    assert all(signum == 0 for _, signum, _ in after), events
    nonzero = [(signum, alive) for _, signum, alive in before if signum != 0]
    if terminated:
        assert nonzero == [(signal.SIGTERM, True), (signal.SIGKILL, True)], events
        assert record["termination"]["groupEmpty"] == "yes"
    else:
        assert nonzero == [], events
        assert record["outcome"] == {"kind": "exited", "exitCode": 3}


# ---- Leader status unavailable at K + 1.4 s (new behaviour) ----------------


class HiddenExitWaiter(PidfdLeaderWaiter):
    """The narrowest seam at the leader-wait source: it hides the leader exit
    from the owner until the test releases it. The leader itself is real."""

    released = threading.Event()

    def wait_exit(self, timeout: float | None) -> bool:
        if not self.released.wait(timeout):
            return False
        return super().wait_exit(timeout)

    def peek(self):  # type: ignore[no-untyped-def]
        if not self.released.is_set():
            return None
        return super().peek()


def test_leader_status_unavailable_at_k_plus_1_4() -> None:
    HiddenExitWaiter.released.clear()
    before = exec_owner.held_leader_count()
    started = time.monotonic()
    record = run("sleep 30", 0.5, OwnerSeams(leader_waiter=HiddenExitWaiter))
    elapsed = time.monotonic() - started
    published = json.dumps(record, sort_keys=True)

    termination = record["termination"]
    assert termination["leaderStatus"] == {"kind": "unavailable"}
    assert termination["leaderExitAfterMs"] is None
    assert termination["groupEmpty"] == "no"
    assert termination["groupEmptyAfterMs"] is None
    assert termination["withinBounds"] is False
    kill = termination["killSentAfterMs"]
    assert 1800 <= kill <= 2000
    # The owner publishes on the same schedule: finalised by K + 1.9 s.
    assert 1400 <= record["lifecycle"]["finalisedAfterMs"] - kill <= 2000
    assert elapsed < 0.5 + 4 + 0.5

    leader = record["lifecycle"]["leaderPid"]
    # The owner still holds the leader, so its PID is an unreaped zombie.
    assert process_state(leader) == "Z"
    assert exec_owner.held_leader_count() == before + 1

    HiddenExitWaiter.released.set()
    end = time.monotonic() + 3
    while exec_owner.held_leader_count() > before and time.monotonic() < end:
        time.sleep(0.01)
    assert exec_owner.held_leader_count() == before
    assert process_state(leader) is None
    assert json.dumps(record, sort_keys=True) == published


# ---- Capture is large or stalls (new behaviour) ----------------------------


def test_large_capture_is_complete_and_exact() -> None:
    size = 32 * 1024 * 1024
    record = run(f"head -c {size} /dev/zero | tr '\\0' 'a'; printf 'tail\\n' >&2", 30)
    stdout = record["evidence"]["stdout"]
    assert stdout["capture"] == "complete"
    assert stdout["bytes"] == stdout["fileBytes"] == size
    assert stream_bytes(record, "stdout") == b"a" * size
    assert record["stdout"] == "a" * size
    assert record["lifecycle"]["finalisedAfterMs"] <= 2000


class StallingFile:
    """A capture file whose reads stop after the first five bytes."""

    release = threading.Event()
    opens = 0
    reads = 0

    def __init__(self, path: Path) -> None:
        StallingFile.opens += 1
        self._file = open(path, "rb")
        self._sent = False

    def read(self, n: int = -1) -> bytes:
        StallingFile.reads += 1
        if not self._sent:
            self._sent = True
            return self._file.read(min(5, n))
        self.release.wait(30)
        return self._file.read(n)

    def __enter__(self) -> StallingFile:
        return self

    def __exit__(self, *exc: object) -> None:
        self._file.close()


def stalling_open(path: Path) -> BinaryIO:
    return StallingFile(path)  # type: ignore[return-value]


def test_stalled_capture_publishes_the_prefix_by_the_deadline() -> None:
    StallingFile.release.clear()
    StallingFile.opens = StallingFile.reads = 0
    started = time.monotonic()
    record = run(
        "printf 'hello world'; printf 'err\\n' >&2", 10, OwnerSeams(open_capture=stalling_open)
    )
    elapsed = time.monotonic() - started
    StallingFile.release.set()

    stdout = record["evidence"]["stdout"]
    assert stdout["capture"] == "incomplete"
    assert stdout["reason"] == "deadline"
    assert stdout["fileBytes"] == 11
    assert stream_bytes(record, "stdout") == b"hello"
    # The single worker never reached stderr, so its bytes are missing too.
    assert record["evidence"]["stderr"]["capture"] == "incomplete"
    assert record["stdout"] == "hello"
    assert 1800 <= record["lifecycle"]["finalisedAfterMs"] <= 2000
    assert elapsed < 2.5
    # The abandoned worker finishes its stalled read and then reads nothing
    # more: not the rest of stdout, and not stderr.
    time.sleep(0.3)
    assert (StallingFile.opens, StallingFile.reads) == (1, 2)


def test_the_published_record_needs_no_work_in_proportion_to_its_bytes(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    """Outcome to published record (spec, section "Bounds"). The digest, the
    base64 text and the legacy projection exist before the owner samples
    `finalisedAfterMs`, so that time covers them. Serialising the published
    record afterwards must not need them again."""
    from testanyware_agent import models

    size = 8 * 1024 * 1024
    record = exec_owner.run_command(
        f"head -c {size} /dev/zero | tr '\\0' 'b'", 30, "ctl-1", time.monotonic()
    )

    def forbidden(*_: object) -> object:
        raise AssertionError("the record derived its evidence after publication")

    monkeypatch.setattr(models.base64, "b64encode", forbidden)
    monkeypatch.setattr(models.hashlib, "sha256", forbidden)
    monkeypatch.setattr(models, "linux_legacy_projection", forbidden)
    published = json.dumps(record.to_dict())
    monkeypatch.undo()

    reply = json.loads(published)
    assert record_errors(reply, exec_id="ctl-1") == []
    assert reply["evidence"]["stdout"]["bytes"] == size
    assert reply["stdout"] == "b" * size
    assert reply["lifecycle"]["finalisedAfterMs"] <= 2000


def test_unlinked_capture_path_keeps_its_bytes() -> None:
    """The command unlinks its own capture paths while its descriptors stay
    open. The owner holds the files, so the record stays valid and exact."""
    record = run(
        'rm -- "$(readlink /proc/$$/fd/1)" "$(readlink /proc/$$/fd/2)"; '
        "printf x; printf y >&2",
        10,
    )
    for name, data in (("stdout", b"x"), ("stderr", b"y")):
        stream = record["evidence"][name]
        assert (stream["capture"], stream["bytes"], stream["fileBytes"]) == ("complete", 1, 1)
        assert stream_bytes(record, name) == data


def test_unlinked_empty_capture_path_is_complete() -> None:
    record = run('rm -- "$(readlink /proc/$$/fd/1)"', 10)
    stdout = record["evidence"]["stdout"]
    assert (stdout["capture"], stdout["bytes"], stdout["fileBytes"]) == ("complete", 0, 0)


def failing_open(path: Path) -> BinaryIO:
    raise PermissionError(13, "Permission denied", str(path))


def test_capture_read_error_is_incomplete() -> None:
    record = run("printf 'x'", 10, OwnerSeams(open_capture=failing_open))
    stdout = record["evidence"]["stdout"]
    assert (stdout["capture"], stdout["reason"], stdout["bytes"]) == ("incomplete", "read_error", 0)
    assert stdout["fileBytes"] == 1


# ---- Exact bytes, exit 23, signal death, spawn errno (corrected-pass) ------


def test_oracle_bytes_survive_exactly() -> None:
    record = run(ORACLE_COMMAND, 10)
    assert record["outcome"] == {"kind": "exited", "exitCode": 0}
    assert stream_bytes(record, "stdout") == ORACLE_STDOUT
    assert stream_bytes(record, "stderr") == ORACLE_STDERR
    assert record["remainingGroup"] == "none"


def test_binary_bytes_survive_although_the_legacy_fields_cannot_hold_them() -> None:
    record = run("printf '\\000\\377\\200A\\n'", 10)
    assert stream_bytes(record, "stdout") == ORACLE_BINARY
    assert record["exitCode"] == -1
    assert record["stdout"] == ""
    assert record["stderr"] == (
        "'utf-8' codec can't decode byte 0xff in position 1: invalid start byte"
    )


def test_exit_23_is_a_numeric_guest_exit() -> None:
    record = run("printf 'partial out\\n'; printf 'partial err\\n' >&2; exit 23", 10)
    assert record["outcome"] == {"kind": "exited", "exitCode": 23}
    assert stream_bytes(record, "stdout") == b"partial out\n"
    assert record["exitCode"] == 23


def test_signal_death_names_the_signal() -> None:
    record = run("printf 'before signal\\n'; kill -KILL $$", 10)
    assert record["outcome"] == {"kind": "signalled", "signal": 9, "signalName": "SIGKILL"}
    assert record["exitCode"] == -9
    assert stream_bytes(record, "stdout") == b"before signal\n"


def test_spawn_errno_survives() -> None:
    record = with_one_process_limit("owner", {"command": "printf 'never runs\\n'", "timeout": 10})
    assert record_errors(record, exec_id="ctl-1") == []
    assert record["outcome"] == {
        "kind": "spawn_failed",
        "errno": 11,
        "errorName": "EAGAIN",
        "message": "[Errno 11] Resource temporarily unavailable",
    }
    assert (record["exitCode"], record["stdout"], record["stderr"]) == (
        -1,
        "",
        "[Errno 11] Resource temporarily unavailable",
    )
    assert record["lifecycle"]["leaderPid"] is None


# ---- Arbitration, identity and inspection ----------------------------------


class LostNotificationWaiter(PidfdLeaderWaiter):
    """The owner never hears the exit event, so it wakes only at the deadline."""

    def wait_exit(self, timeout: float | None) -> bool:
        time.sleep(max(0.0, timeout or 0.0))
        return False


def test_a_finished_command_is_never_reported_as_timed_out() -> None:
    record = run("exit 7", 0.3, OwnerSeams(leader_waiter=LostNotificationWaiter))
    assert record["outcome"] == {"kind": "exited", "exitCode": 7}
    assert record["termination"] is None
    assert record["lifecycle"]["ownerLatenessMs"] <= 500


def test_record_names_the_leader_and_its_group() -> None:
    record = run('read -r line < /proc/$$/stat; printf "%s" "$line"', 10)
    fields = stream_bytes(record, "stdout").decode()
    after = fields[fields.rindex(")") + 2 :].split()
    lifecycle = record["lifecycle"]
    assert int(fields.split()[0]) == lifecycle["leaderPid"]
    assert int(after[2]) == lifecycle["groupId"] == lifecycle["leaderPid"]
    assert after[19] == lifecycle["leaderStartTime"]


def test_normal_exit_reports_the_remaining_group_and_signals_nothing(tmp_path: Path) -> None:
    pid_file = tmp_path / "child.pid"
    record = run(f"sleep 30 & echo $! > {pid_file}; exit 0", 10)
    child_pid, child_ticks = read_child(pid_file)
    try:
        assert record["outcome"] == {"kind": "exited", "exitCode": 0}
        assert record["remainingGroup"] == "present"
        assert child_survives(child_pid, child_ticks)
    finally:
        os.kill(child_pid, signal.SIGKILL)


def test_a_failed_group_probe_never_proves_absence(monkeypatch: pytest.MonkeyPatch) -> None:
    real_killpg = os.killpg

    def killpg(group: int, signum: int) -> None:
        if signum == 0:
            raise PermissionError(1, "Operation not permitted")
        real_killpg(group, signum)

    monkeypatch.setattr(exec_owner.os, "killpg", killpg)
    exited = run("exit 0", 10)
    assert exited["remainingGroup"] == "unknown"
    timed_out = run("sleep 30", 0.5)
    assert timed_out["termination"]["groupEmpty"] == "unknown"
    assert timed_out["termination"]["withinBounds"] is False


def test_capture_directory_is_removed_after_each_command(capture_root: Path) -> None:
    run(ORACLE_COMMAND, 10)
    assert list(capture_root.iterdir()) == []


def test_agent_start_removes_old_capture_directories(capture_root: Path) -> None:
    stale = capture_root / "exec-stale"
    stale.mkdir(parents=True)
    (stale / "stdout").write_bytes(b"old")
    exec_owner.prepare_capture_root()
    assert list(capture_root.iterdir()) == []


def test_requested_exec_id_is_kept() -> None:
    from testanyware_agent import system_endpoints

    status, body = system_endpoints.handle_exec({"command": "true", "timeout": 5, "id": "my.id-1"})
    assert status == 200
    assert body["id"] == "my.id-1"
    status, body = system_endpoints.handle_exec({"command": "true", "timeout": 5})
    assert status == 200
    assert record_errors(body) == []
    assert base64.b64decode(body["evidence"]["stdout"]["base64"]) == b""
