"""Controls for the runner, seams and checker of leaf `linux-timeout-lifecycle-k5`."""

import json
import os
import subprocess
import sys
from pathlib import Path
from typing import Any

import pytest

import controller
import linux_k5
import linux_k5_check as checker
import linux_k5_seam as seam_module
from linux_k5_seam import ISSUE_FORK_WAIT, ISSUE_SLEEP, SEAMS, cases
from outcomes_k12_seam import ORACLE_BINARY, ORACLE_STDERR, ORACLE_STDOUT

TAG = "123456789012"


def by_name(seam: str = "reference") -> dict[str, Any]:
    return {case.name: case for case in cases(seam, TAG)}


def test_the_case_matrix_is_finite_and_frozen() -> None:
    assert list(by_name()) == [
        "oracle-text",
        "binary",
        "exit-23-partial",
        "exit-255",
        "signal-kill",
        "signal-hup",
        "deadline",
        "deadline-term-ignored",
        "issue-sleep",
        "issue-fork-wait",
        "spawn-failure",
    ]
    oracle = by_name()["oracle-text"]
    assert (oracle.stdout, oracle.stderr, oracle.code) == (ORACLE_STDOUT, ORACLE_STDERR, 0)
    assert by_name()["binary"].stdout == ORACLE_BINARY


def test_the_issue_controls_are_the_exact_retained_commands() -> None:
    assert ISSUE_SLEEP == "sleep 120"
    assert ISSUE_FORK_WAIT == "sleep 120 & p=$!; echo $p > /tmp/vmrel-k16/child.pid; wait"
    assert by_name()["issue-sleep"].command == ISSUE_SLEEP
    assert by_name()["issue-fork-wait"].command == ISSUE_FORK_WAIT
    assert {by_name()[name].timeout for name in ("issue-sleep", "issue-fork-wait")} == {2}


def test_each_seam_gets_its_own_deadline_token_ending_in_the_run_tag() -> None:
    tokens = [
        case.command.rsplit("sleep ", 1)[1]
        for seam in SEAMS
        for case in cases(seam, TAG)
        if case.name.startswith("deadline")
    ]
    assert len(tokens) == len(set(tokens)) == 2 * len(SEAMS)
    assert all(token.endswith(TAG) for token in tokens)


def test_the_run_tag_has_twelve_digits() -> None:
    assert linux_k5.run_tag("f" * 32) == f"{int('f' * 32, 16) % 10**12:012d}"
    assert len(linux_k5.run_tag("0" * 31 + "1")) == 12


def test_the_late_observation_waits_for_the_faithful_bound() -> None:
    deadline, ordinary = by_name()["deadline"], by_name()["oracle-text"]
    assert seam_module.late_ns(deadline, 0, 2_000_000_000) == 6_000_000_000
    assert seam_module.late_ns(ordinary, 0, 1_000) == 2_000_001_000


def row(pid: int, ticks: int, state: str = "S") -> dict[str, Any]:
    return {"pid": pid, "start_ticks": ticks, "state": state, "argv": ["sleep", "120"]}


def test_ownership_needs_a_new_pid_or_a_new_start_time() -> None:
    baseline = {"rows": [row(1, 10), row(7, 20)]}
    later = {"rows": [row(1, 10), row(7, 99), row(9, 30), row(11, 31, "Z")]}
    assert [r["pid"] for r in seam_module.owned(baseline, later)] == [7, 9, 11]
    assert [r["pid"] for r in checker.owned_live(baseline, later) or []] == [7, 9]
    assert seam_module.owned(baseline, {"unknown": {}}) == []
    assert checker.owned_live(baseline, {"unknown": {}}) is None


class FakeKernel:
    """The pidfd and /proc seams of `kill_here`, for a host without either.

    `holders` maps a PID to the processes that hold it in turn, each as an
    identity and its start ticks. A pidfd fixes the identity at open time.
    `step` passes the PID to the next holder between the open and the re-read,
    which is the reuse window.
    """

    def __init__(self, holders: dict[int, list[tuple[str, int]]], step: bool = False) -> None:
        self.holders = holders
        self.step = step
        self.fds: dict[int, tuple[int, str]] = {}
        self.signalled: list[str] = []

    def open(self, pid: int) -> int:
        if not self.holders.get(pid):
            raise ProcessLookupError(pid)
        fd = os.open(os.devnull, os.O_RDONLY)
        self.fds[fd] = (pid, self.holders[pid][0][0])
        if self.step:
            self.holders[pid].pop(0)
        return fd

    def ticks(self, pid: int) -> int | None:
        holders = self.holders.get(pid)
        return holders[0][1] if holders else None

    def kill(self, fd: int) -> None:
        pid, identity = self.fds[fd]
        holders = self.holders.get(pid)
        if not holders or holders[0][0] != identity:
            raise ProcessLookupError(pid)
        self.signalled.append(identity)

    def install(self, monkeypatch: pytest.MonkeyPatch) -> None:
        monkeypatch.setattr(seam_module, "pidfd_open", self.open)
        monkeypatch.setattr(seam_module, "start_ticks_of", self.ticks)
        monkeypatch.setattr(seam_module, "pidfd_kill", self.kill)

    def all_closed(self) -> bool:
        return all(not is_open(fd) for fd in self.fds)


def is_open(fd: int) -> bool:
    try:
        os.fstat(fd)
    except OSError:
        return False
    return True


def outcomes(results: list[dict[str, Any]]) -> list[tuple[int, bool, str | None]]:
    return [(r["pid"], r["killed"], r.get("why")) for r in results]


def test_the_kill_helper_kills_a_matching_target(monkeypatch: pytest.MonkeyPatch) -> None:
    kernel = FakeKernel({7: [("target", 20)]})
    kernel.install(monkeypatch)
    assert outcomes(seam_module.kill_here([(7, 20)])) == [(7, True, None)]
    assert kernel.signalled == ["target"]
    assert kernel.all_closed()


def test_the_kill_helper_spares_a_pid_reused_before_the_open(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    kernel = FakeKernel({7: [("other", 99)]})
    kernel.install(monkeypatch)
    results = seam_module.kill_here([(7, 20), (9, 30)])
    assert outcomes(results) == [(7, False, "gone"), (9, False, "gone")]
    assert kernel.signalled == []
    assert kernel.all_closed()


def test_the_kill_helper_spares_a_pid_reused_after_the_open(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # The pidfd holds the target, which dies. A new process then takes the PID.
    kernel = FakeKernel({7: [("target", 20), ("other", 99)]}, step=True)
    kernel.install(monkeypatch)
    assert outcomes(seam_module.kill_here([(7, 20)])) == [(7, False, "gone")]
    assert kernel.signalled == []
    assert kernel.all_closed()


def test_the_kill_helper_refuses_to_signal_a_successor_with_the_same_ticks(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    # The re-read matches a successor, but the pidfd process is dead. The kernel refuses.
    kernel = FakeKernel({7: [("target", 20), ("other", 20)]}, step=True)
    kernel.install(monkeypatch)
    assert outcomes(seam_module.kill_here([(7, 20)])) == [(7, False, "raced")]
    assert kernel.signalled == []
    assert kernel.all_closed()


def test_the_kill_helper_records_a_refused_pidfd(monkeypatch: pytest.MonkeyPatch) -> None:
    def refuse(pid: int) -> int:
        raise PermissionError(pid)

    monkeypatch.setattr(seam_module, "pidfd_open", refuse)
    [result] = seam_module.kill_here([(7, 20)])
    assert result["killed"] is False
    assert str(result["why"]).startswith("refused:")


def test_the_plan_hashes_every_source_before_and_after() -> None:
    subjects = set(linux_k5.plan("/usr/bin/python3").subjects)
    recorded = {linux_k5.REPO / name for name in linux_k5.sources()}
    assert recorded <= subjects
    here = linux_k5.HERE
    for needed in (
        here / "outcomes_k12_check.py",
        here / "controller.py",
        here / "writer.py",
        linux_k5.AGENT_PACKAGE / "tree_walker.py",
        linux_k5.STUB / "pyatspi/__init__.py",
        linux_k5.STUB / "gi/repository/__init__.py",
    ):
        assert needed in subjects, needed


def test_a_copied_bundle_replays_from_its_own_bytes(tmp_path: Path) -> None:
    run = tmp_path / "abc123"
    run.mkdir()
    (run / "reference.stdout").write_text('{"reference": []}')
    elsewhere = "/no/such/worktree/run-03/abc123"
    (tmp_path / "summary.json").write_text(json.dumps({"run": elsewhere}))
    summary, seams = checker.load(tmp_path)
    assert summary["run"] == elsewhere
    assert seams["reference"] == {"reference": []}
    assert seams["host"] is None


def test_a_summary_without_a_run_loads_nothing(tmp_path: Path) -> None:
    (tmp_path / "summary.json").write_text("{}")
    _, seams = checker.load(tmp_path)
    assert set(seams.values()) == {None}


def test_the_stream_classes_are_distinct() -> None:
    assert checker.stream_defects(b"a\n", b"a", "stdout") == ["trailing-lf-trimmed"]
    assert checker.stream_defects(ORACLE_BINARY, b"", "stdout") == ["non-utf8-dropped"]
    assert checker.stream_defects(b"text\n", b"", "stdout") == ["partial-output-dropped"]
    assert checker.stream_defects(b"", b"Process timed out", "stderr") == ["timeout-text-in-stderr"]
    assert checker.stream_defects(b"x\n", b"y\n", "stdout") == ["stream-mismatch"]


def test_the_status_classes() -> None:
    names = by_name()
    exit_obs = {"end": "exit", "timed_out": None}
    assert checker.status_defects(names["signal-kill"], exit_obs | {"code": -9}) == [
        "signal-reported-as-exit"
    ]
    assert checker.status_defects(names["deadline"], exit_obs | {"code": -1}) == [
        "deadline-not-flagged",
        "synthetic-exit-code",
    ]
    assert checker.status_defects(names["spawn-failure"], exit_obs | {"code": -1}) == [
        "spawn-failure-unmapped"
    ]
    faithful = {"end": "deadline-signal", "code": 15, "timed_out": True}
    assert checker.status_defects(names["deadline"], faithful) == []
    errno = {"end": "spawn-error", "code": 11}
    assert checker.status_defects(names["spawn-failure"], errno) == []


def verdict(case: str, end: str, code: int) -> dict[str, Any]:
    return {
        "seam": "http",
        "case": case,
        "end": end,
        "code": code,
        "timed_out": None,
        "defects": [],
    }


def test_ambiguity_marks_only_different_faithful_ends() -> None:
    verdicts = [
        verdict("deadline", "exit", -1),
        verdict("issue-sleep", "exit", -1),
        verdict("oracle-text", "exit", 0),
        verdict("binary", "exit", 0),
    ]
    checker.mark_ambiguous(verdicts)
    assert [v["defects"] for v in verdicts] == [[], [], [], []]
    verdicts.append(verdict("signal-hup", "exit", -1))
    checker.mark_ambiguous(verdicts)
    assert [bool(v["defects"]) for v in verdicts] == [True, True, False, False, True]


def test_a_traced_kill_first_skips_term() -> None:
    trace = [
        {"event": "communicate-timeout", "ns": 1},
        {"event": "signal", "ns": 2, "signal": 9},
        {"event": "wait-return", "ns": 3},
    ]
    marks = checker.trace_marks(trace)
    assert marks["first_signal"] == 9
    assert checker.trace_phases(marks) == {"kill_reap": 1e-9}


def test_every_required_defect_names_a_known_seam_and_case() -> None:
    names = set(by_name())
    assert all(seam in SEAMS and case in names for seam, case, _ in checker.REQUIRED)
    assert not any(seam == "reference" for seam, _, _ in checker.REQUIRED)


def test_the_controller_accepts_the_frozen_plan() -> None:
    frozen = linux_k5.plan("/usr/bin/python3")
    assert [call.name for call in frozen.calls] == ["reference", "process", "traced", "host"]
    assert all(Path(call.argv[0]).is_absolute() for call in frozen.calls)
    validate = getattr(controller, "_validate")  # noqa: B009 - the controller's own refusal rules
    validate(frozen)


def test_the_host_call_imports_the_seam_with_its_frozen_flags() -> None:
    host = linux_k5.plan("/usr/bin/python3").calls[-1]
    flags = list(host.argv[1 : 1 + len(linux_k5.HOST_FLAGS)])
    assert tuple(flags) == linux_k5.HOST_FLAGS
    done = subprocess.run(
        [sys.executable, *flags, str(linux_k5.SEAM)], capture_output=True, timeout=30, check=False
    )
    assert done.returncode == 64, done.stderr
