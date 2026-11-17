"""Controls for the k4 post-timeout inspection tooling (leaf macos-timeout-inspection-k4).

Each control builds a run directory without a VM. The run-level host evidence is the
retained k186 result record. The calls are copies of one healthy k186 call record,
renamed to the k4 shape. The receipts and downloads are the expected outcome of each
role. Each control then breaks one property and expects the checker to report it.
"""

import copy
import json
import shutil
from collections.abc import Callable
from pathlib import Path
from typing import Any

import pytest

import controller
import timeout_k4
import timeout_k4_check as checker

HERE = Path(__file__).resolve().parent
RETAINED = HERE / "evidence/k186-completion-macos-01/b1256cae4068418dac994e03f0392f6b"
ORIGINAL = Path(
    "/Users/antony/Development/AgenticOrchestration/Ravel-Strategies.gh-issue-1"
    "/experiments/tutorial-vm-fit/evidence/macos-03/clone-1/observe-timeout.json"
)
ORIGINAL_ONLINE = (
    "bash /tmp/vmfit-k15/guest.sh online"
    " https://github.com/mikefarah/yq/releases/download/v4.47.2/yq_darwin_arm64"
    " 4ccc7f2f5f6f37804d70ad211a287b1b589f67024ecb77586c77106030424b9f"
)
DISPOSED = {"ledger": [{"kind": "vm", "state": "disposed"}], "residuals": []}


def _guest(name: str, exit_code: int, timed_out: bool, stdout: str) -> dict[str, Any]:
    return {
        "schema_version": "1.0",
        "ok": True,
        "command": checker.COMMANDS[name],
        "exit_code": exit_code,
        "timed_out": timed_out,
        "stdout": stdout,
        "stderr": "",
    }


def _transfer(name: str) -> dict[str, Any]:
    if name == checker.MISSING:
        return {"schema_version": "1.0", "ok": False, "code": "DOWNLOAD_FAILED"}
    remote = checker.TRANSFERS[name]
    local = remote.rsplit("/", 1)[1]
    size = (
        len(checker.DOWNLOADS[local])
        if name.startswith("r3-download-")
        else len((timeout_k4.FIXTURE / local).read_bytes())
    )
    return {"schema_version": "1.0", "ok": True, "remote": remote, "bytes": size}


def _receipt(name: str) -> dict[str, Any]:
    """Return the expected receipt of one call of the frozen shape."""
    kind = checker.role(name)
    if kind == "health":
        return {"schema_version": "1.0", "ok": True, "reachable": True, "platform": "macos"}
    if kind == "transfer":
        return _transfer(name)
    if kind == "cancel":
        return {}
    if kind == "timeout":
        return _guest(name, -1, True, "")
    if name in checker.OWN_OBSERVATIONS:
        return _guest(name, 1, False, "child pid=700")
    if kind == "observe":
        return _guest(name, 0, False, "timeout pid=701\ncancel pid=702")
    return _guest(name, checker.R3_ORIGINAL.get(name, 0), False, "ok")


def _call(template: dict[str, Any], name: str) -> dict[str, Any]:
    call = copy.deepcopy(template) | {"name": name, "elapsed": 0.2}
    if checker.role(name) == "timeout":
        call["elapsed"] = 2.3
    if name == "r3-cancel":
        call |= {"reason": "interrupted", "returncode": -15}
    return call


def _run(
    tmp_path: Path, summary: dict[str, Any] | None = None, names: tuple[str, ...] = checker.SHAPE
) -> Path:
    """Build one complete clean run in the frozen k4 shape, or in the shape `names`."""
    run = tmp_path / "out" / "run"
    run.mkdir(parents=True)
    result = json.loads((RETAINED / "result.json").read_bytes())
    calls = {call["name"]: call for call in result["calls"]}
    template = calls["serial-01"]
    shape = [_call(template, name) for name in names]
    resource = [calls[name] for name in checker.RESOURCE]
    result["calls"] = [resource[0], *shape, *resource[1:]]
    result["unreached"] = []
    (run / "result.json").write_text(json.dumps(result))
    shutil.copy(RETAINED / "storage-close.json", run / "storage-close.json")
    for name in names:
        (run / f"{name}.stdout").write_text(json.dumps(_receipt(name)))
    downloads = run.parent / "r3-downloads"
    downloads.mkdir()
    for local, data in checker.DOWNLOADS.items():
        (downloads / local).write_bytes(data)
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
    assert len(checker.SHAPE) == 37 and len(checker.AFTER) == 10
    # The clone-1 order of the original r3 run between its start and its stop.
    original = (
        "health bootstrap-dir upload-guest-sh upload-lesson inventory install intact"
        " download-stdout-bin download-stderr-bin download-artifact-bin download-status-txt"
        " online nonzero bad-install timeout observe-timeout cancel observe-cancel"
        " download-timeout-partial download-cancel-partial missing-artifact guest-cleanup"
        " observe-after-cleanup"
    )
    assert checker.R3 == tuple(f"r3-{name}" for name in original.split())
    assert checker.SHAPE[1:4] == ("issue-sleep", "issue-child", "issue-observe")


def test_clean_run_is_complete_and_not_reproduced(tmp_path: Path) -> None:
    report = checker.check(_run(tmp_path))
    assert (report["evidence"], report["stall"]) == ("complete", "not-reproduced")
    assert report["violations"] == {} and report["failed"] == []
    assert report["checker_revision"] == 3
    child = report["samples"]["after-02-observe"]["children"][0]
    assert child == {"label": "child", "pid": 700, "state": "absent"}
    r3 = report["samples"]["r3-observe-cancel"]["children"]
    assert [c["state"] for c in r3] == ["not-listed", "not-listed"]


def _set(**fields: object) -> Callable[[dict[str, Any]], None]:
    return lambda receipt: receipt.update(fields)


def _drop(field: str) -> Callable[[dict[str, Any]], None]:
    return lambda receipt: receipt.pop(field)


# Each wrong outcome is the expected receipt of one call with one property changed.
WRONG_OUTCOMES: dict[str, tuple[str, Callable[[dict[str, Any]], None]]] = {
    "r3-online-original-exit-1": ("r3-online", _set(exit_code=1)),
    "r3-online-without-url": ("r3-online", _set(command="bash /tmp/vmfit-k15/guest.sh online")),
    "nonzero-exit-0": ("r3-nonzero", _set(exit_code=0)),
    "bad-install-exit-0": ("r3-bad-install", _set(exit_code=0)),
    "install-exit-1": ("r3-install", _set(exit_code=1)),
    "r3-case-timed-out": ("r3-intact", _set(timed_out=True)),
    "ordinary-exit-2": ("after-05-ordinary", _set(exit_code=2)),
    "observation-exit-2": ("after-02-observe", _set(exit_code=2)),
    "r3-observation-exit-1": ("r3-observe-cancel", _set(exit_code=1)),
    "receipt-not-ok": ("after-04-threads", _set(ok=False)),
    "exit-code-string": ("after-06-ordinary", _set(exit_code="0")),
    "exit-code-boolean": ("after-06-ordinary", _set(exit_code=False)),
    "timed-out-absent": ("after-07-identity", _drop("timed_out")),
    "schema-2": ("after-03-table", _set(schema_version="2.0")),
    "health-unreachable": ("after-01-health", _set(reachable=False)),
    "health-not-ok": ("r3-health", _set(ok=False)),
    "upload-wrong-path": ("r3-upload-lesson", _set(remote="/tmp/elsewhere")),
    "upload-wrong-size": ("r3-upload-guest-sh", _set(bytes=1)),
    "download-not-ok": ("r3-download-stdout-bin", _set(ok=False)),
    "missing-artifact-found": ("r3-missing-artifact", _set(ok=True)),
}


@pytest.mark.parametrize("case", sorted(WRONG_OUTCOMES))
def test_wrong_role_outcome_fails_the_call(tmp_path: Path, case: str) -> None:
    name, change = WRONG_OUTCOMES[case]
    run = _run(tmp_path)
    receipt = _receipt(name)
    change(receipt)
    (run / f"{name}.stdout").write_text(json.dumps(receipt))
    report = checker.check(run)
    assert report["failed"] == [name] and report["evidence"] == "incomplete"
    assert report["stall"] == "unknown"


@pytest.mark.parametrize("local", ["artifact.bin", "timeout.partial"])
def test_download_without_the_pinned_bytes_fails(tmp_path: Path, local: str) -> None:
    run = _run(tmp_path)
    path = run.parent / "r3-downloads" / local
    path.write_bytes(path.read_bytes()[:-1] + b"X")
    report = checker.check(run)
    assert len(report["failed"]) == 1 and report["stall"] == "unknown"


def test_failed_observation_leaves_a_printed_child_unknown(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _edit(
        run, "after-08-observe", lambda call: call["streams"]["stdout"].update(completeness="cut")
    )
    _write_receipt(run, "after-02-observe", stderr="ps: illegal option")
    samples = checker.check(run)["samples"]
    for name in ("after-02-observe", "after-08-observe"):
        assert samples[name]["children"][0]["state"] == "unknown"


def test_timed_out_observation_reproduces_even_with_incomplete_evidence(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write_receipt(run, "issue-observe", exit_code=-1, timed_out=True, stdout="child pid=700")
    (run / "r3-online.stdout").unlink()
    _edit(run, "r3-guest-cleanup", lambda call: call.update(ok=False))
    report = checker.check(run)
    assert (report["evidence"], report["stall"]) == ("incomplete", "reproduced")
    assert report["stalled"] == ["issue-observe"]
    child = report["samples"]["issue-observe"]["children"][0]
    assert child == {"label": "child", "pid": 700, "state": "unknown"}


def test_slow_observation_without_a_guest_timeout_is_not_admitted(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _edit(run, "after-08-observe", lambda call: call.update(elapsed=33.1))
    report = checker.check(run)
    assert report["stalled"] == ["after-08-observe"] and report["admitted"] == []
    assert report["stall"] == "unknown"


def _stall_not_ok(run: Path) -> None:
    _write_receipt(run, "after-09-table", ok=False, exit_code=-1, timed_out=True)


def _stall_cut(run: Path) -> None:
    _write_receipt(run, "after-09-table", exit_code=-1, timed_out=True)
    _edit(run, "after-09-table", lambda call: call["streams"]["stderr"].update(completeness="cut"))


def _stall_host_deadline(run: Path) -> None:
    _write_receipt(run, "after-09-table", exit_code=-1, timed_out=True)
    _edit(run, "after-09-table", lambda call: call.update(ok=False, reason="deadline"))


def _stall_without_prior_timeout(run: Path) -> None:
    for name in ("issue-sleep", "issue-child"):
        _write_receipt(run, name, exit_code=0, timed_out=False)
    _write_receipt(run, "after-09-table", exit_code=-1, timed_out=True)


def _stall_with_damaged_provenance(run: Path) -> None:
    _write_receipt(run, "after-09-table", exit_code=-1, timed_out=True)
    (run / "storage-close.json").unlink()


@pytest.mark.parametrize(
    "damage",
    [
        _stall_not_ok,
        _stall_cut,
        _stall_host_deadline,
        _stall_without_prior_timeout,
        _stall_with_damaged_provenance,
    ],
)
def test_stall_without_valid_local_evidence_is_not_admitted(
    tmp_path: Path, damage: Callable[[Path], None]
) -> None:
    run = _run(tmp_path)
    damage(run)
    report = checker.check(run)
    assert report["stalled"] == ["after-09-table"] and report["admitted"] == []
    assert report["stall"] == "unknown"


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
    call["streams"] = {stream: {"completeness": "eof"} for stream in checker.STREAMS}
    sample = checker.judge("r3-observe-timeout", call, original["receipt"])
    assert sample["stalled"] is True and sample["local_stall"] is True
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
    assert by_name["r3-online"].argv[-3:] == ("120", "--json", ORIGINAL_ONLINE)
    controller._validate(plan, tmp_path)  # pyright: ignore[reportPrivateUsage]
    (resource,) = plan.resources
    assert resource.cleanup.budget == 26.0 and '"sleep 120"' in resource.cleanup.argv[-1]


def test_ordinary_guest_timeout_after_the_timeouts_reproduces(tmp_path: Path) -> None:
    run = _run(tmp_path)
    _write_receipt(run, "after-09-table", exit_code=-1, timed_out=True)
    report = checker.check(run)
    assert report["stalled"] == ["after-09-table"] and report["stall"] == "reproduced"
    assert report["failed"] == ["after-09-table"] and report["admitted"] == ["after-09-table"]


def _cleanup(stdout: str, exit_code: int) -> dict[str, Any]:
    return {
        "schema_version": "1.0",
        "ok": True,
        "command": timeout_k4.OWNED_CLEANUP,
        "exit_code": exit_code,
        "timed_out": False,
        "stdout": stdout,
        "stderr": "",
    }


@pytest.mark.parametrize(
    ("receipt", "state"),
    [
        (_cleanup("x pid=1 command=\ngone pid=1\ncleanup confirmed", 0), "confirmed"),
        (
            _cleanup("x pid=1 command=\nunknown pid=1 state=unknown\ncleanup uncertain", 1),
            "uncertain",
        ),
        (_cleanup("x pid=1 command=", 0), "unknown"),
        (_cleanup("cleanup confirmed", 0) | {"command": "printf"}, "unknown"),
        (None, "unknown"),
    ],
)
def test_cleanup_reports_only_a_confirmed_state_as_confirmed(
    receipt: dict[str, Any] | None, state: str
) -> None:
    assert checker.cleanup_state({"ok": True}, receipt)["state"] == state


def test_interrupted_cancel_expects_the_term_exit() -> None:
    (cancel,) = [
        c for c in timeout_k4.r3_calls("/c", "vm", Path("/"), Path("/")) if c.interrupt_after
    ]
    assert (cancel.name, cancel.expected_exit, cancel.expected_reason) == (
        "r3-cancel",
        -15,
        "interrupted",
    )
