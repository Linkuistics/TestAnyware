"""Controls for the k204 Linux timeout lifecycle tooling (leaf linux-timeout-live-k204).

Each control builds a run directory without a VM. The run-level host evidence is the
retained k186 result record. The calls are copies of one healthy k186 call record,
renamed to the k204 shape. The receipts, raw records and downloads are the expected
outcome of each role on the unchanged Linux agent. Each control then breaks one
property and expects the checker to report it.
"""

import copy
import json
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

import controller
import linux_k204
import linux_k204_check as checker

HERE = Path(__file__).resolve().parent
RETAINED = HERE / "evidence/k186-completion-macos-01/b1256cae4068418dac994e03f0392f6b"
DISPOSED = {"ledger": [{"kind": "vm", "state": "disposed"}], "residuals": []}
PIDS = {"cli-fork-shell": 900, "cli-fork-child": 901, "raw-fork-shell": 910, "raw-fork-child": 911}
LSTART = "Wed Nov 18 03:40:00 2026"


def _row(key: str, args: str, stat: str = "S") -> str:
    pid = PIDS[key]
    return f"    {pid}       1   {pid}   {pid} {stat}    {LSTART} {args}"


def _observation(labels: tuple[str, ...], rows: dict[str, str] | None = None) -> str:
    """Return one own observation. A key in `rows` is listed with that row."""
    rows = rows or {}
    lines: list[str] = []
    for label in labels:
        for role in ("shell", "child"):
            key = f"{label}-{role}"
            lines.append(f"{key} pid={PIDS[key]}")
            if key in rows:
                lines.extend([rows[key], f"{key} ps-exit=0"])
            else:
                lines.append(f"{key} ps-exit=1")
    return "\n".join(lines)


def _observation_stdout(name: str) -> str:
    if name == checker.LATE or name == checker.AFTER_CLEANUP:
        return _observation(linux_k204.FORK_LABELS)
    return _observation((name.removeprefix("observe-"),))


def _guest(name: str, exit_code: int, stdout: str, stderr: str = "") -> dict[str, Any]:
    return {
        "schema_version": "1.0",
        "ok": True,
        "command": checker.COMMANDS[name],
        "exit_code": exit_code,
        "timed_out": False,
        "stdout": stdout,
        "stderr": stderr,
    }


def _raw(name: str, exit_code: int = -1, stderr: str = checker.TIMEOUT_STDERR) -> dict[str, Any]:
    body = {"exitCode": exit_code, "stdout": "", "stderr": stderr}
    return {
        "schema": "k204-raw/2",
        "request": {"command": checker.COMMANDS[name], "timeout": checker.GUEST_BUDGET[name]},
        "http_status": 200,
        "body": body,
        "error": None,
        "seconds": 2.1,
    }


def _transfer(name: str) -> dict[str, Any]:
    if name == checker.MISSING:
        return {"schema_version": "1.0", "ok": False, "code": "DOWNLOAD_FAILED"}
    remote = checker.TRANSFERS[name]
    remote_name = remote.rsplit("/", 1)[1]
    if name.startswith("r5-upload-"):
        size = len((linux_k204.FIXTURE / remote_name).read_bytes())
    elif name.startswith("r5-download-"):
        size = len(checker.DOWNLOADS[remote_name])
    else:
        size = len(b"partial-file\n")
    return {"schema_version": "1.0", "ok": True, "remote": remote, "bytes": size}


def _receipt(name: str) -> dict[str, Any]:
    """Return the expected receipt of one call of the frozen shape."""
    kind = checker.role(name)
    if kind == "health":
        return {"schema_version": "1.0", "ok": True, "reachable": True, "platform": "linux"}
    if kind == "transfer":
        return _transfer(name)
    if kind == "cancel":
        return {}
    if name in checker.RAW:
        return _raw(name)
    if kind == "timeout":
        return _guest(name, -1, "", checker.TIMEOUT_STDERR)
    if kind == "observe":
        return _guest(name, 0, _observation_stdout(name))
    if kind == "r5-observe":
        return _guest(name, 0, "timeout pid=701\ncancel pid=702")
    if kind == "cleanup":
        return _guest(name, 0, "cli-fork-child gone\nraw-fork-child gone\ncleanup confirmed")
    return _guest(name, checker.R5_ORIGINAL.get(name, 0), "ok")


def _call(template: dict[str, Any], name: str) -> dict[str, Any]:
    call = copy.deepcopy(template) | {"name": name, "elapsed": 0.2}
    if checker.role(name) == "timeout":
        call["elapsed"] = 2.3
    if name == "r5-cancel":
        call |= {"reason": "interrupted", "returncode": -15}
    return call


def _run(tmp_path: Path, summary: dict[str, Any] | None = None) -> Path:
    """Build one complete clean run in the frozen k204 shape."""
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
    for folder in ("k204-downloads", "r5-downloads"):
        (run.parent / folder).mkdir()
    for local, data in checker.DOWNLOADS.items():
        folder = (
            "k204-downloads"
            if local.removesuffix(".partial") in linux_k204.FORK_LABELS
            else "r5-downloads"
        )
        (run.parent / folder / local).write_bytes(data)
    (run.parent / "summary.json").write_text(json.dumps(summary or DISPOSED))
    return run


def _edit(run: Path, name: str, edit: Callable[[dict[str, Any]], None]) -> None:
    path = run / "result.json"
    result = json.loads(path.read_bytes())
    edit(next(call for call in result["calls"] if call["name"] == name))
    path.write_text(json.dumps(result))


def _write(run: Path, name: str, receipt: dict[str, Any]) -> None:
    (run / f"{name}.stdout").write_text(json.dumps(receipt))


def _observe(run: Path, name: str, stdout: str, exit_code: int = 0, stderr: str = "") -> None:
    _write(run, name, _guest(name, exit_code, stdout, stderr))


def test_shape_is_frozen() -> None:
    # The clone-1 order of the original r5 run, linux-03, between its start and its stop.
    original = (
        "health bootstrap-dir upload-guest-sh upload-lesson inventory install intact"
        " download-stdout-bin download-stderr-bin download-artifact-bin download-status-txt"
        " online nonzero bad-install timeout observe-timeout cancel observe-cancel"
        " download-timeout-partial download-cancel-partial missing-artifact guest-cleanup"
        " observe-after-cleanup"
    )
    assert checker.R5 == tuple(f"r5-{name}" for name in original.split())
    assert checker.CONTROLS[3:8] == (
        "cli-sleep",
        "table-after-cli-sleep",
        "raw-sleep",
        "table-after-raw-sleep",
        "cli-fork",
    )
    assert checker.COMMANDS["cli-sleep"] == checker.COMMANDS["raw-sleep"] == "sleep 120"
    assert all(checker.GUEST_BUDGET[name] == 2 for name in checker.TIMEOUTS)


def test_clean_run_is_complete_and_not_reproduced(tmp_path: Path) -> None:
    report = checker.check(_run(tmp_path))
    assert report["evidence"] == "complete", report["failed"] or report["run_failures"]
    assert (report["survival"], report["stall"]) == ("not-reproduced", "not-reproduced")
    assert report["fixture_survival"]["result"] == "not-listed"
    assert report["control_cleanup"]["state"] == "confirmed"
    assert report["violations"] == {}


def test_child_listed_in_the_late_observation_reproduces_failure_b(tmp_path: Path) -> None:
    run = _run(tmp_path)
    rows = {"cli-fork-child": _row("cli-fork-child", "sleep 120")}
    _observe(run, checker.LATE, _observation(linux_k204.FORK_LABELS, rows))
    report = checker.check(run)
    assert report["survival"] == "reproduced"
    control = report["fork_controls"]["cli-fork"]
    assert control["states"]["child"]["late"] == "present"
    assert report["violations"][checker.LATE] == [
        "cli-fork child present 5 s after the control returned"
    ]
    assert report["evidence"] == "complete"


def test_zombie_child_reproduces_and_another_command_does_not(tmp_path: Path) -> None:
    run = _run(tmp_path)
    rows = {
        "cli-fork-child": _row("cli-fork-child", "[sleep] <defunct>", "Z"),
        "raw-fork-child": _row("raw-fork-child", "sshd: other"),
    }
    _observe(run, checker.LATE, _observation(linux_k204.FORK_LABELS, rows))
    report = checker.check(run)
    controls = report["fork_controls"]
    assert controls["cli-fork"]["result"] == "reproduced"
    assert controls["raw-fork"]["states"]["child"]["late"] == "other"
    assert controls["raw-fork"]["result"] == "unknown"
    assert report["survival"] == "reproduced"


@pytest.mark.parametrize(
    "case",
    ["nonzero exit", "guest timeout", "ps status 2", "pid file unreadable", "absent receipt"],
)
def test_failed_late_observation_leaves_each_child_unknown(tmp_path: Path, case: str) -> None:
    run = _run(tmp_path)
    stdout = _observation(linux_k204.FORK_LABELS)
    if case == "nonzero exit":
        _observe(run, checker.LATE, stdout, exit_code=1)
    elif case == "guest timeout":
        _observe(run, checker.LATE, "", exit_code=-1, stderr=checker.TIMEOUT_STDERR)
    elif case == "ps status 2":
        _observe(run, checker.LATE, stdout.replace("child ps-exit=1", "child ps-exit=2"))
    elif case == "pid file unreadable":
        unreadable = stdout.replace("cli-fork-child pid=901\ncli-fork-child ps-exit=1", "")
        _observe(run, checker.LATE, unreadable + "\ncli-fork-child pid-file=unreadable")
    else:
        (run / f"{checker.LATE}.stdout").write_text("not json")
    report = checker.check(run)
    assert report["fork_controls"]["cli-fork"]["states"]["child"]["late"] == "unknown"
    assert report["survival"] == "unknown"


def test_guest_timeout_in_an_observation_is_an_admitted_stall(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _observe(run, "observe-raw-fork", "", exit_code=-1, stderr=checker.TIMEOUT_STDERR)
    report = checker.check(run)
    assert report["stall"] == "reproduced"
    assert report["admitted"] == ["observe-raw-fork"]
    assert report["evidence"] == "incomplete"


def test_slow_observation_without_a_guest_timeout_is_not_admitted(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _edit(run, "observe-cli-fork", lambda call: call.update(elapsed=12.0))
    report = checker.check(run)
    assert report["stalled"] == ["observe-cli-fork"]
    assert (report["stall"], report["admitted"]) == ("unknown", [])


@pytest.mark.parametrize(
    "edit",
    [
        {"http_status": 500},
        {"error": "timed out"},
        {"body": "not an object"},
        {"request": {"command": "sleep 120", "timeout": 3}},
        {"schema": "k204-raw/1"},
    ],
)
def test_raw_record_failure_fails_the_raw_timeout(tmp_path: Path, edit: dict[str, Any]) -> None:
    run = _run(tmp_path)
    _write(run, "raw-sleep", _raw("raw-sleep") | edit)
    report = checker.check(run)
    assert report["failed"] == ["raw-sleep"]
    assert report["evidence"] == "incomplete"


def test_raw_control_with_an_ordinary_exit_is_not_a_timeout(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write(run, "raw-fork", _raw("raw-fork", exit_code=0, stderr=""))
    report = checker.check(run)
    assert report["fork_controls"]["raw-fork"]["timeout_valid"] is False
    assert report["fork_controls"]["raw-fork"]["result"] == "unknown"
    assert report["survival"] == "unknown"


def test_timeout_without_the_agent_timeout_shape_fails(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write(run, "cli-sleep", _guest("cli-sleep", -1, "", "Killed"))
    assert checker.check(run)["failed"] == ["cli-sleep"]


def test_timed_out_field_is_recorded_and_not_required(tmp_path: Path) -> None:
    report = checker.check(_run(tmp_path))
    assert report["samples"]["cli-sleep"]["timed_out"] is False
    assert report["samples"]["cli-sleep"]["failures"] == []


def test_overhead_above_four_seconds_is_a_violation(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _edit(run, "cli-fork", lambda call: call.update(elapsed=6.5))
    raw = _raw("raw-sleep") | {"seconds": 6.1}
    _write(run, "raw-sleep", raw)
    report = checker.check(run)
    assert report["violations"]["cli-fork"] == ["overhead 4.500 s above 4 s"]
    assert report["violations"]["raw-sleep"] == ["raw_overhead 4.100 s above 4 s"]
    assert report["evidence"] == "complete"


@pytest.mark.parametrize(
    ("name", "receipt"),
    [
        ("r5-nonzero", {"exit_code": 0}),
        ("r5-bad-install", {"exit_code": 0}),
        ("r5-inventory", {"exit_code": 1}),
        ("table-before", {"exit_code": 1}),
        ("identity", {"command": "id"}),
        ("table-after-cli-sleep", {"exit_code": "0"}),
    ],
)
def test_wrong_outcome_fails_the_call(tmp_path: Path, name: str, receipt: dict[str, Any]) -> None:
    run = _run(tmp_path)
    _write(run, name, _receipt(name) | receipt)
    report = checker.check(run)
    assert report["failed"] == [name]
    assert report["evidence"] == "incomplete"


def test_download_without_the_pinned_bytes_fails(tmp_path: Path) -> None:
    run = _run(tmp_path)
    (run.parent / "k204-downloads" / "cli-fork.partial").write_bytes(b"other\n")
    (run.parent / "r5-downloads" / "timeout.partial").unlink()
    report = checker.check(run)
    assert report["failed"] == ["download-cli-fork-partial", "r5-download-timeout-partial"]


def test_missing_artifact_must_fail_with_a_typed_error(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write(run, checker.MISSING, {"schema_version": "1.0", "ok": True})
    assert checker.check(run)["failed"] == [checker.MISSING]


def test_r5_observation_listing_the_timeout_child_reports_it(tmp_path: Path) -> None:
    run = _run(tmp_path)
    stdout = "timeout pid=701\n    701     600 S    sleep"
    _observe(run, "r5-observe-timeout", stdout)
    report = checker.check(run)
    assert report["fixture_survival"]["result"] == "listed"
    assert report["evidence"] == "complete"


def test_cancel_must_be_interrupted(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _edit(run, "r5-cancel", lambda call: call.update(reason="exit", returncode=0))
    assert checker.check(run)["failed"] == ["r5-cancel"]


def test_undisposed_clone_or_residual_fails_disposal(tmp_path: Path) -> None:
    summary = {"ledger": [{"kind": "vm", "state": "acquired"}], "residuals": ["vm"]}
    report = checker.check(_run(tmp_path, summary))
    assert len(report["disposal_failures"]) == 2
    assert report["evidence"] == "incomplete"


def test_missing_call_leaves_the_evidence_incomplete(tmp_path: Path) -> None:
    run = _run(tmp_path)
    path = run / "result.json"
    result = json.loads(path.read_bytes())
    result["calls"] = [call for call in result["calls"] if call["name"] != checker.LATE]
    result["unreached"] = [checker.LATE]
    path.write_text(json.dumps(result))
    report = checker.check(run)
    assert checker.LATE in report["missing"]
    assert (report["survival"], report["stall"]) == ("unknown", "unknown")


def test_retained_fixture_matches_and_one_changed_byte_is_refused(tmp_path: Path) -> None:
    assert linux_k204.fixture_mismatches(linux_k204.FIXTURE) == []
    copy_dir = tmp_path / "fixture"
    shutil.copytree(linux_k204.FIXTURE, copy_dir)
    data = bytearray((copy_dir / "guest.sh").read_bytes())
    data[-2] ^= 1
    (copy_dir / "guest.sh").write_bytes(bytes(data))
    assert linux_k204.fixture_mismatches(copy_dir) != []


def test_plan_freezes_the_shape_bounds_and_owned_cleanup(tmp_path: Path) -> None:
    tools = {"cli": "/bin/echo", "tart": "/bin/echo", "python": "/usr/bin/python3"}
    plan = linux_k204.plan(tools, tmp_path)
    assert isinstance(plan, controller.Plan)
    assert tuple(call.name for call in plan.calls) == checker.SHAPE
    by_name = {call.name: call for call in plan.calls}
    for name in checker.TIMEOUTS:
        assert by_name[name].budget == 2 + linux_k204.HOST_MARGIN
    for name in checker.OWN_OBSERVATIONS:
        assert by_name[name].budget == linux_k204.OBSERVE_BOUND
    assert by_name["r5-cancel"].interrupt_after == 2
    assert by_name["r5-cancel"].expected_exit == -15
    (resource,) = plan.resources
    assert resource.cleanup.budget == linux_k204.CLEANUP_BOUND
    assert resource.cleanup.argv[-1] == linux_k204.FINAL_CLEANUP
    assert resource.cleanup.budget + plan.term_grace + plan.kill_wait <= 30
    assert resource.dispose.budget + plan.term_grace + plan.kill_wait <= 45


def test_owned_cleanup_acts_only_on_the_exact_issue_sleep() -> None:
    script = linux_k204.CONTROL_CLEANUP
    assert '= "sleep 120"' in script
    assert "kill -TERM" in script and "kill -KILL" in script
    assert "pkill" not in script and "killall" not in script


def test_cleanup_reports_only_a_confirmed_state_as_confirmed(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write(run, "control-cleanup", _guest("control-cleanup", 0, "x unknown\ncleanup uncertain"))
    assert checker.check(run)["control_cleanup"]["state"] == "uncertain"
    _write(run, "control-cleanup", _guest("control-cleanup", 1, "cleanup confirmed"))
    assert checker.check(run)["control_cleanup"]["state"] == "unknown"
