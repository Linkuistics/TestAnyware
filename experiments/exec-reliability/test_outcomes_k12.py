"""Controls for the runner, seams and checker of leaf `macos-exec-outcomes-k12`."""

import base64
import json
import subprocess
from pathlib import Path
from typing import Any

import outcomes_k12
import outcomes_k12_check as checker
import outcomes_k12_seam as seam_module
from outcomes_k12_seam import ORACLE_BINARY, ORACLE_STDERR, ORACLE_STDOUT, SEAMS, cases

TAG = "123456789012"


def test_the_case_matrix_is_finite_and_frozen() -> None:
    names = [case.name for case in cases("reference")]
    assert names == [
        "oracle-text",
        "binary",
        "exit-23-partial",
        "signal-kill",
        "exit-9",
        "deadline",
        "deadline-term-ignored",
        "spawn-failure",
    ]
    oracle = cases("reference")[0]
    assert (oracle.stdout, oracle.stderr, oracle.code) == (ORACLE_STDOUT, ORACLE_STDERR, 0)
    assert ORACLE_STDOUT.hex() == "4578706c61696e2074686973206368616e67652e"
    assert ORACLE_STDERR.hex() == "7365706172617465207374646572720a"
    assert ORACLE_BINARY.hex() == "00ff80410a"


def test_each_seam_gets_its_own_sleep_token_ending_in_the_run_tag() -> None:
    tokens = {
        case.command.split("sleep ")[1]
        for seam in SEAMS
        for case in cases(seam, TAG)
        if "sleep " in case.command
    }
    assert len(tokens) == 2 * len(SEAMS)
    assert all(token.endswith(TAG) for token in tokens)
    assert outcomes_k12.run_tag("f" * 32) == f"{(16**32 - 1) % 10**12:012d}"


def test_the_audit_pattern_matches_only_this_run() -> None:
    pattern = outcomes_k12.token_pattern(TAG)
    assert pattern.match(f"sleep 30.21{TAG}")
    assert not pattern.match("sleep 30.21")
    assert not pattern.match("sleep 30.21999999999999")
    assert not pattern.match(f"sleep 30.21{TAG}0")


def test_sweep_kills_only_the_token_of_this_run() -> None:
    own = f"sleep 30.11{TAG}"
    argvs = (["sleep", "30.11"], ["sleep", "30.11999999999999"], own.split())
    children = [subprocess.Popen(argv) for argv in argvs]  # noqa: S603 - fixed sleeps
    try:
        found = seam_module.sweep(own)
        assert [entry["pid"] for entry in found] == [children[2].pid]
        assert found[0]["killed"] is True
        assert children[2].wait(timeout=2) == -9
        assert children[0].poll() is None
        assert children[1].poll() is None
    finally:
        for child in children:
            child.kill()
            child.wait(timeout=2)


def test_the_plan_runs_four_seam_calls(tmp_path: Path) -> None:
    built = {name: tmp_path / name for name in ("probe-default", "probe-traced", "agent")}
    plan = outcomes_k12.plan("/usr/bin/python3", built)
    assert [call.name for call in plan.calls] == ["reference", "process", "traced", "http-cli"]


def test_stream_defects_name_each_known_loss() -> None:
    assert checker.stream_defects(b"a\n", b"a\n", "stdout") == []
    assert checker.stream_defects(b"a\n", b"a", "stdout") == ["trailing-lf-trimmed"]
    assert checker.stream_defects(ORACLE_BINARY, b"", "stdout") == ["non-utf8-dropped"]
    assert checker.stream_defects(b"a", b"b", "stdout") == ["stream-mismatch"]
    noted = b"Process timed out after 2s"
    assert checker.stream_defects(b"", noted, "stderr") == ["timeout-text-in-stderr"]
    assert checker.stream_defects(b"x\n", b"x\n" + noted, "stderr") == ["timeout-text-in-stderr"]


def test_status_defects_separate_signal_deadline_and_race() -> None:
    by_name = {case.name: case for case in cases("process")}
    signal, deadline = by_name["signal-kill"], by_name["deadline"]
    exit9, spawn = by_name["exit-9"], by_name["spawn-failure"]
    as_exit = {"end": "exit", "code": 9, "timed_out": False}
    assert checker.status_defects(signal, as_exit) == ["signal-reported-as-exit"]
    assert checker.status_defects(exit9, as_exit) == []
    synthetic = {"end": "exit", "code": -1, "timed_out": True}
    assert checker.status_defects(deadline, synthetic) == ["synthetic-exit-code"]
    raced = {"end": "exit", "code": -1, "timed_out": True}
    assert checker.status_defects(exit9, raced) == [checker.RACE, "synthetic-exit-code"]
    assert checker.status_defects(spawn, {"end": "http-500", "code": None}) == [
        "spawn-failure-unmapped"
    ]
    assert checker.status_defects(spawn, {"end": "spawn-error", "code": 35}) == []


def reference_term_ignored() -> dict[str, Any]:
    return {
        "case": "deadline-term-ignored",
        "status": {"end": "signal", "code": 9},
        "timed_out": True,
        "stdout_hex": b"ignoring term\n".hex(),
        "stderr_hex": "",
        "start_ns": 0,
        "deadline_ns": 2_000_000_000,
        "term_ns": 2_000_100_000,
        "grace_expired_ns": 4_000_100_000,
        "kill_ns": 4_000_200_000,
        "reap_ns": 4_000_300_000,
        "end_ns": 4_000_400_000,
        "residuals": [],
    }


def test_a_faithful_reference_deadline_needs_its_marks_and_signal() -> None:
    (verdict,) = checker.judge("reference", [reference_term_ignored()])
    assert verdict["defects"] == []
    for name in ("deadline_ns", "term_ns", "grace_expired_ns", "kill_ns", "reap_ns"):
        record = reference_term_ignored()
        del record[name]
        (verdict,) = checker.judge("reference", [record])
        assert verdict["defects"] == ["phase-marks-invalid"], name
    reordered = reference_term_ignored() | {"kill_ns": 1}
    (verdict,) = checker.judge("reference", [reordered])
    assert verdict["defects"] == ["phase-marks-invalid"]
    by_term = reference_term_ignored() | {"status": {"end": "signal", "code": 15}}
    (verdict,) = checker.judge("reference", [by_term])
    assert verdict["defects"] == ["status-mismatch"]
    summary: dict[str, Any] = {"verdict": True, "leftovers": [], "build_dir_removed": True}
    problems = checker.soundness(summary, {}, [verdict])
    assert any("reference/deadline-term-ignored is not faithful" in p for p in problems)


def test_the_cli_bound_is_the_agreed_ceiling() -> None:
    ordinary = cases("cli-json")[0]
    record = seam_module.cli_case("/usr/bin/true", {}, "normal", ordinary, json_mode=True)
    assert record["bound_seconds"] == 30
    verdict: dict[str, Any] = {
        "seam": "cli-json",
        "case": ordinary.name,
        "defects": [],
        "bound_exceeded": False,
        "seconds": 35.0,
        "ceiling_seconds": 30,
    }
    summary: dict[str, Any] = {"verdict": True, "leftovers": [], "build_dir_removed": True}
    problems = checker.soundness(summary, {}, [verdict])
    assert "cli-json/oracle-text exceeded the agreed 30 s ceiling" in problems


def test_the_cli_text_mode_never_infers_a_timeout() -> None:
    record = {
        "case": "deadline",
        "returncode": 255,
        "stdout_hex": b"before deadline".hex(),
        "stderr_hex": b"Process timed out after 2s\n".hex(),
        "start_ns": 0,
        "end_ns": 1,
    }
    (verdict,) = checker.judge("cli-text", [record])
    assert verdict["timed_out"] is None
    assert "deadline-not-flagged" in verdict["defects"]
    assert "synthetic-exit-code" in verdict["defects"]


def test_trace_phases_measure_grace_and_reap() -> None:
    rows = [
        {"event": "terminate-sent", "t_ns": 1_000_000_000},
        {"event": "wait-result", "phase": "terminate", "t_ns": 4_000_000_000},
        {"event": "kill-sent", "t_ns": 4_100_000_000},
        {"event": "reap-return", "t_ns": 4_200_000_000},
    ]
    phases = checker.trace_phases(rows)
    assert phases == {"grace": 3.0, "kill_reap": 0.1}
    assert checker.phase_defects(phases) == ["grace-exceeded"]


def test_the_detector_reports_each_missing_required_defect() -> None:
    missing = checker.missing_required([])
    assert len(missing) == len(checker.REQUIRED)
    merged: dict[tuple[str, str], set[str]] = {}
    for seam, case, alternatives in checker.REQUIRED:
        merged.setdefault((seam, case), set()).update(alternatives)
    present: list[dict[str, Any]] = [
        {"seam": seam, "case": case, "defects": sorted(found)}
        for (seam, case), found in merged.items()
    ]
    assert checker.missing_required(present) == []


def test_soundness_rejects_an_unfaithful_reference() -> None:
    verdict: dict[str, Any] = {
        "seam": "reference",
        "case": "binary",
        "defects": ["non-utf8-dropped"],
        "bound_exceeded": False,
        "seconds": 0.01,
        "ceiling_seconds": 30,
    }
    summary: dict[str, Any] = {"verdict": True, "leftovers": [], "build_dir_removed": True}
    problems = checker.soundness(summary, {"reference": {"work_removed": True}}, [verdict])
    assert any("reference/binary is not faithful" in problem for problem in problems)


def http_record(case: str, body: dict[str, Any]) -> dict[str, Any]:
    return {
        "case": case,
        "status": 200,
        "body_hex": json.dumps(body).encode().hex(),
        "start_ns": 0,
        "end_ns": 1_000_000,
    }


def stream(data: bytes, capture: str = "complete") -> dict[str, Any]:
    return {"base64": base64.b64encode(data).decode(), "capture": capture}


def exited(stdout: bytes, stderr: bytes, code: int) -> dict[str, Any]:
    return {
        "outcome": {"kind": "exited", "exitCode": code},
        "evidence": {"stdout": stream(stdout), "stderr": stream(stderr)},
    }


def record_defects(case: str, body: dict[str, Any]) -> list[str]:
    return checker.judge_record("http-record", [http_record(case, body)])[0]["defects"]


def test_the_record_judge_reads_outcome_and_exact_bytes() -> None:
    assert record_defects("exit-23-partial", exited(b"partial out\n", b"partial err\n", 23)) == []
    flipped = exited(b"partial out\n", b"partial err\n", 23)
    flipped["evidence"]["stdout"] = stream(b"partial oux\n")
    assert record_defects("exit-23-partial", flipped) == ["stream-mismatch"]
    partial = exited(b"partial out\n", b"partial err\n", 23)
    partial["evidence"]["stderr"] = stream(b"partial err\n", "incomplete")
    assert record_defects("exit-23-partial", partial) == ["capture-incomplete"]
    legacy = {"exitCode": 23, "stdout": "partial out", "stderr": "partial err", "timedOut": False}
    assert record_defects("exit-23-partial", legacy) == ["record-missing"]


def test_the_record_judge_reads_signal_and_spawn_errno() -> None:
    signal = exited(b"before signal\n", b"", 0)
    signal["outcome"] = {"kind": "signalled", "signal": 9, "signalName": "SIGKILL"}
    assert record_defects("signal-kill", signal) == []
    spawn = {"outcome": {"kind": "spawn_failed", "errno": 35}, "evidence": {}}
    assert record_defects("spawn-failure", spawn) == []
    spawn["outcome"]["errno"] = 12
    assert record_defects("spawn-failure", spawn) == ["spawn-failure-unmapped"]


def deadline(signal: int, kill_ms: int, final_ms: int, within: bool) -> dict[str, Any]:
    body = exited(b"before deadline\n", b"", 0)
    body["outcome"] = {"kind": "timed_out", "timeoutSeconds": 2}
    body["termination"] = {
        "termSentAfterMs": 0,
        "killSentAfterMs": kill_ms,
        "leaderStatus": {"kind": "signalled", "signal": signal},
        "withinBounds": within,
    }
    body["lifecycle"] = {"finalisedAfterMs": final_ms}
    return body


def test_the_record_judge_checks_the_termination_block() -> None:
    assert record_defects("deadline", deadline(15, 3, 4, True)) == []
    assert record_defects("deadline", deadline(9, 3, 4, True)) == ["status-mismatch"]
    assert record_defects("deadline", deadline(15, 3, 4, False)) == ["termination-out-of-bounds"]
    late = record_defects("deadline", deadline(15, 3000, 6000, True))
    assert late == ["grace-exceeded", "kill-reap-exceeded"]


def test_the_record_mode_needs_every_case_faithful() -> None:
    faithful: list[dict[str, Any]] = [
        {"seam": seam, "case": case.name, "defects": []}
        for seam, base in checker.RECORD_SEAMS.items()
        for case in cases(base)
    ]
    assert checker.record_failures(faithful) == []
    faithful[0]["defects"] = ["record-missing"]
    assert checker.record_failures(faithful[1:]) == ["http-record/oracle-text: no verdict"]
    assert checker.record_failures(faithful) == ["http-record/oracle-text: ['record-missing']"]
