"""Controls for the r3-only probe and checker of leaf `macos-timeout-inspection-k193`."""

import json
from pathlib import Path
from typing import Any

import pytest

import controller
import timeout_k4
import timeout_k4_check
import timeout_k193
import timeout_k193_check as checker
from test_timeout_k4 import (
    DISPOSED,
    ORIGINAL_ONLINE,
    _run,  # pyright: ignore[reportPrivateUsage]
    _write_receipt,  # pyright: ignore[reportPrivateUsage]
)

TOOLS = {"python": "/usr/bin/python3", "cli": "/opt/homebrew/bin/testanyware", "tart": "/t"}


def _r3(tmp_path: Path, summary: dict[str, Any] | None = None) -> Path:
    return _run(tmp_path, summary, checker.SHAPE)


def test_plan_is_the_original_r3_sequence_alone(tmp_path: Path) -> None:
    plan = timeout_k193.plan(TOOLS, tmp_path)
    assert tuple(call.name for call in plan.calls) == checker.SHAPE == timeout_k4_check.R3
    assert plan.calls[0].name == "r3-health"
    (resource,) = plan.resources
    assert resource.identity == f"k193-r3-macos-{plan.run_id}"
    frozen = timeout_k4.r3_calls(TOOLS["cli"], resource.identity, timeout_k4.FIXTURE, tmp_path)
    assert plan.calls == tuple(frozen)


def test_plan_keeps_the_original_arguments_and_owned_cleanup(tmp_path: Path) -> None:
    plan = timeout_k193.plan(TOOLS, tmp_path)
    by_name = {call.name: call for call in plan.calls}
    assert by_name["r3-online"].argv[-3:] == ("120", "--json", ORIGINAL_ONLINE)
    assert by_name["r3-timeout"].argv[-3:-1] == ("2", "--json")
    assert by_name["r3-cancel"].interrupt_after == 2.0
    assert by_name["r3-cancel"].expected_exit == -15
    controller._validate(plan, tmp_path)  # pyright: ignore[reportPrivateUsage]
    (resource,) = plan.resources
    assert resource.acquire.argv[resource.acquire.argv.index("--base") + 1] == timeout_k4.GOLDEN
    assert resource.cleanup.budget == 26.0 and '"sleep 120"' in resource.cleanup.argv[-1]


def test_plan_digests_the_reused_k4_tooling(tmp_path: Path) -> None:
    names = {path.name for path in timeout_k193.plan(TOOLS, tmp_path).subjects}
    assert {"timeout_k4.py", "timeout_k4_check.py", "timeout_k193_check.py"} <= names
    assert {"timeout_k193_run.sh", "guest.sh", "lesson", "cases.json"} <= names


def test_clean_r3_run_is_complete_and_not_reproduced(tmp_path: Path) -> None:
    report = checker.check_r3(_r3(tmp_path))
    assert report["condition"] == "r3-only" and report["checker_revision"] == 3
    assert (report["evidence"], report["stall"]) == ("complete", "not-reproduced")


def test_issue_payloads_before_r3_break_the_condition(tmp_path: Path) -> None:
    report = checker.check_r3(_run(tmp_path))
    assert any("calls outside the shape" in failure for failure in report["run_failures"])
    assert report["evidence"] != "complete"


def test_guest_timeout_at_the_observation_after_the_timeout_reproduces(tmp_path: Path) -> None:
    run = _r3(tmp_path)
    _write_receipt(run, "r3-observe-timeout", exit_code=-1, timed_out=True, stdout="")
    report = checker.check_r3(run)
    assert report["stalled"] == ["r3-observe-timeout"] and report["stall"] == "reproduced"
    assert report["admitted"] == ["r3-observe-timeout"]
    child = report["samples"]["r3-observe-timeout"]
    assert child["local_stall"] is True


def test_timed_out_observation_never_proves_absence(tmp_path: Path) -> None:
    run = _r3(tmp_path)
    _write_receipt(
        run, "r3-observe-timeout", exit_code=-1, timed_out=True, stdout="timeout pid=701\n"
    )
    sample = checker.check_r3(run)["samples"]["r3-observe-timeout"]
    assert "not-listed" not in json.dumps(sample)


def test_stall_without_a_valid_earlier_timeout_is_not_admitted(tmp_path: Path) -> None:
    run = _r3(tmp_path)
    _write_receipt(run, "r3-timeout", exit_code=0, timed_out=False)
    _write_receipt(run, "r3-observe-timeout", exit_code=-1, timed_out=True, stdout="")
    report = checker.check_r3(run)
    assert report["admitted"] == [] and report["stall"] != "reproduced"


def test_missing_observation_leaves_the_evidence_incomplete(tmp_path: Path) -> None:
    run = _r3(tmp_path)
    path = run / "result.json"
    result = json.loads(path.read_bytes())
    result["calls"] = [c for c in result["calls"] if c["name"] != "r3-observe-after-cleanup"]
    result["unreached"] = ["r3-observe-after-cleanup"]
    path.write_text(json.dumps(result))
    report = checker.check_r3(run)
    assert report["evidence"] != "complete" and report["stall"] != "not-reproduced"


def test_failed_r3_call_is_kept_as_failed(tmp_path: Path) -> None:
    run = _r3(tmp_path)
    _write_receipt(run, "r3-nonzero", exit_code=0)
    report = checker.check_r3(run)
    assert "r3-nonzero" in report["failed"]


def test_residual_clone_fails_the_check(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    residual = {"ledger": [{"kind": "vm", "state": "residual"}], "residuals": ["k193-r3-x"]}
    run = _r3(tmp_path, residual)
    monkeypatch.setattr("sys.argv", ["timeout_k193_check.py", str(run)])
    assert checker.main() == 1
    assert json.loads(capsys.readouterr().out)["disposal_failures"]


def test_clean_run_passes_the_command(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, capsys: pytest.CaptureFixture[str]
) -> None:
    run = _r3(tmp_path, DISPOSED)
    monkeypatch.setattr("sys.argv", ["timeout_k193_check.py", str(run)])
    assert checker.main() == 0
    assert json.loads(capsys.readouterr().out)["stall"] == "not-reproduced"


def test_k4_check_defaults_are_unchanged(tmp_path: Path) -> None:
    report = timeout_k4_check.check(_run(tmp_path))
    assert (report["evidence"], report["stall"]) == ("complete", "not-reproduced")
