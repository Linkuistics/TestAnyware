"""Controls for the k4 post-timeout inspection tooling (leaf macos-timeout-inspection-k4).

Each control builds a run directory without a VM. The run-level host evidence is the
retained k186 result record. The calls are copies of one healthy k186 call record,
renamed to the k4 shape. Each control then breaks one property and expects the
checker to report it.
"""

import copy
import json
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

import timeout_k4
import timeout_k4_check as checker

HERE = Path(__file__).resolve().parent
RETAINED = HERE / "evidence/k186-completion-macos-01/b1256cae4068418dac994e03f0392f6b"
ORIGINAL = Path(
    "/Users/antony/Development/AgenticOrchestration/Ravel-Strategies.gh-issue-1"
    "/experiments/tutorial-vm-fit/evidence/macos-03/clone-1/observe-timeout.json"
)
DISPOSED = {"ledger": [{"kind": "vm", "state": "disposed"}], "residuals": []}


def _receipt(name: str) -> dict[str, Any]:
    kind = checker.role(name)
    if kind == "timeout":
        return {"ok": True, "exit_code": -1, "timed_out": True, "stdout": "", "stderr": ""}
    if kind == "observe":
        out = "child pid=700\ntimeout pid=701\ncancel pid=702"
        return {"ok": True, "exit_code": 0, "timed_out": False, "stdout": out, "stderr": ""}
    return {"ok": True, "exit_code": 0, "timed_out": False, "stdout": "ok", "stderr": ""}


def _call(template: dict[str, Any], name: str) -> dict[str, Any]:
    call = copy.deepcopy(template) | {"name": name, "elapsed": 0.2}
    if checker.role(name) == "timeout":
        call["elapsed"] = 2.3
    if name == "r3-cancel":
        call |= {"reason": "interrupted", "returncode": -15}
    return call


def _run(tmp_path: Path, summary: dict[str, Any] | None = None) -> Path:
    """Build one complete clean run in the frozen k4 shape."""
    run = tmp_path / "out" / "run"
    run.mkdir(parents=True)
    result = json.loads((RETAINED / "result.json").read_bytes())
    calls = {call["name"]: call for call in result["calls"]}
    template = calls["serial-01"]
    shape = [_call(template, name) for name in checker.SHAPE]
    resource = [calls[name] for name in checker.RESOURCE]
    result["calls"] = [resource[0], *shape, *resource[1:]]
    result["unreached"] = []
    (run / "result.json").write_text(json.dumps(result))
    shutil.copy(RETAINED / "storage-close.json", run / "storage-close.json")
    for name in checker.SHAPE:
        (run / f"{name}.stdout").write_text(json.dumps(_receipt(name)))
    (run.parent / "summary.json").write_text(json.dumps(summary or DISPOSED))
    return run


def _edit(run: Path, name: str, edit: Callable[[dict[str, Any]], None]) -> None:
    path = run / "result.json"
    result = json.loads(path.read_bytes())
    edit(next(call for call in result["calls"] if call["name"] == name))
    path.write_text(json.dumps(result))


def _write_receipt(run: Path, name: str, **fields: object) -> None:
    (run / f"{name}.stdout").write_text(json.dumps(_receipt(name) | fields))


def test_shape_is_frozen() -> None:
    assert len(checker.SHAPE) == 36 and len(checker.AFTER) == 10
    assert checker.SHAPE[1:4] == ("issue-sleep", "issue-child", "issue-observe")


def test_clean_run_is_complete_and_not_reproduced(tmp_path: Path) -> None:
    report = checker.check(_run(tmp_path))
    assert (report["evidence"], report["stall"]) == ("complete", "not-reproduced")
    assert report["violations"] == {} and report["failed"] == []


def test_timed_out_observation_reproduces_even_with_incomplete_evidence(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write_receipt(run, "issue-observe", exit_code=-1, timed_out=True, stdout="child pid=700")
    (run / "r3-online.stdout").unlink()
    report = checker.check(run)
    assert (report["evidence"], report["stall"]) == ("incomplete", "reproduced")
    assert report["stalled"] == ["issue-observe"]
    child = report["samples"]["issue-observe"]["children"][0]
    assert child == {"label": "child", "pid": 700, "state": "unknown"}


def test_slow_observation_reproduces_without_a_guest_timeout(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _edit(run, "after-08-observe", lambda call: call.update(elapsed=33.1))
    assert checker.check(run)["stalled"] == ["after-08-observe"]


def _drop_receipt(run: Path) -> None:
    (run / "r3-observe-cancel.stdout").unlink()


def _fail_observation(run: Path) -> None:
    _edit(run, "after-02-observe", lambda call: call.update(ok=False))


def _uninterrupted_cancel(run: Path) -> None:
    _edit(run, "r3-cancel", lambda call: call.update(reason="exit"))


def _cut_stream(run: Path) -> None:
    _edit(
        run, "after-05-ordinary", lambda call: call["streams"]["stdout"].update(completeness="cut")
    )


@pytest.mark.parametrize(
    "damage", [_drop_receipt, _fail_observation, _uninterrupted_cancel, _cut_stream]
)
def test_missing_or_failed_evidence_leaves_the_stall_unknown(
    tmp_path: Path, damage: Callable[[Path], None]
) -> None:
    run = _run(tmp_path)
    damage(run)
    report = checker.check(run)
    assert (report["evidence"], report["stall"]) == ("incomplete", "unknown")


def test_unreached_observation_leaves_the_stall_unknown(tmp_path: Path) -> None:
    run = _run(tmp_path)
    path = run / "result.json"
    result = json.loads(path.read_bytes())
    result["calls"] = [c for c in result["calls"] if c["name"] != "r3-observe-after-cleanup"]
    result["unreached"] = ["r3-observe-after-cleanup"]
    path.write_text(json.dumps(result))
    report = checker.check(run)
    assert report["stall"] == "unknown" and report["missing"] == ["r3-observe-after-cleanup"]


def test_timeout_overhead_above_four_seconds_is_a_violation(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _edit(run, "issue-child", lambda call: call.update(elapsed=6.5))
    report = checker.check(run)
    assert report["violations"] == {"issue-child": ["overhead 4.500 s above 4 s"]}
    assert report["stall"] == "not-reproduced"


def test_timeout_without_a_timed_out_receipt_fails(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write_receipt(run, "issue-sleep", exit_code=0, timed_out=False)
    assert checker.check(run)["failed"] == ["issue-sleep"]


@pytest.mark.parametrize(
    "summary",
    [
        {"ledger": [{"kind": "vm", "state": "retained"}], "residuals": []},
        {"ledger": [], "residuals": []},
        {"ledger": [{"kind": "vm", "state": "disposed"}], "residuals": [{"kind": "leader"}]},
    ],
)
def test_undisposed_clone_or_residual_fails_disposal(
    tmp_path: Path, summary: dict[str, Any]
) -> None:
    report = checker.check(_run(tmp_path, summary))
    assert report["disposal_failures"] and report["evidence"] == "incomplete"


@pytest.mark.skipif(not ORIGINAL.is_file(), reason="retained VM-fit evidence is not on this host")
def test_original_r3_failed_inspection_is_judged_stalled() -> None:
    original = json.loads(ORIGINAL.read_bytes())
    call = {"ok": True, "reason": "exit", "returncode": 0, "elapsed": original["seconds"]}
    sample = checker.judge("r3-observe-timeout", call, original["receipt"])
    assert sample["stalled"] is True
    assert sample["children"] == [{"label": "timeout", "pid": 666, "state": "unknown"}]


def test_retained_fixture_matches_and_one_changed_byte_is_refused(tmp_path: Path) -> None:
    assert timeout_k4.fixture_mismatches(timeout_k4.FIXTURE) == []
    copied = tmp_path / "fixture"
    shutil.copytree(timeout_k4.FIXTURE, copied)
    with (copied / "guest.sh").open("ab") as handle:
        handle.write(b"\n")
    assert [m.split(":")[0] for m in timeout_k4.fixture_mismatches(copied)] == ["guest.sh"]


def test_plan_freezes_the_issue_sequence_and_owned_cleanup(tmp_path: Path) -> None:
    tools = {"python": "/usr/bin/python3", "cli": "/opt/homebrew/bin/testanyware", "tart": "/t"}
    plan = timeout_k4.plan(tools, tmp_path)
    names = [call.name for call in plan.calls]
    assert tuple(names) == checker.SHAPE
    by_name = {call.name: call for call in plan.calls}
    assert by_name["issue-sleep"].argv[-3:] == ("2", "--json", "sleep 120")
    assert by_name["issue-observe"].budget == 50.0
    assert by_name["r3-cancel"].interrupt_after == 2.0
    (resource,) = plan.resources
    assert resource.cleanup.budget == 26.0 and '"sleep 120"' in resource.cleanup.argv[-1]
