"""Controls for the runner, seams and checker of leaf `linux-timeout-lifecycle-k5`."""

from typing import Any

import pytest

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


def test_the_kill_helper_spares_a_reused_pid(monkeypatch: pytest.MonkeyPatch) -> None:
    # The host has no /proc, so the control supplies the rows. No signal is sent.
    monkeypatch.setattr(seam_module, "proc_rows", lambda: [row(7, 99)])
    results = seam_module.kill_here([(7, 20), (9, 30)])
    assert [(r["pid"], r["killed"], r["why"]) for r in results] == [
        (7, False, "gone"),
        (9, False, "gone"),
    ]


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
