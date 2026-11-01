"""Failure controls operate on real local processes and fake VM markers only."""

import json
import os
import signal
import sys
import uuid
from dataclasses import fields, replace
from pathlib import Path

import pytest

from controller import Call, Plan, Resource, execute

HERE = Path(__file__).resolve().parent
PEER = HERE / "peer.py"


def call(name, mode, *args, budget=3, interrupt_after=None):
    return Call(
        name,
        (sys.executable, str(PEER), mode, *map(str, args)),
        budget,
        interrupt_after=interrupt_after,
    )


def run(tmp_path, calls, resources=(), subjects=(), run_id=None):
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "evidence"))
    plan = Plan(
        run_id or uuid.uuid4().hex,
        tuple(calls),
        tuple(resources),
        (PEER, Path(__file__), HERE / "pyproject.toml", HERE / "uv.lock", *subjects),
        term_grace=0.15,
        kill_wait=0.3,
    )
    path = execute(plan, root)
    return path, json.loads((path / "result.json").read_text())


def resource(tmp_path, run_id, *, inspection="inspect", cleanup="bytes", dispose="dispose"):
    identity = f"diag-{run_id}-vm"
    return Resource(
        "vm",
        identity,
        call("acquire", "acquire", identity, tmp_path),
        call("cleanup", cleanup, identity, tmp_path, budget=0.2),
        call("dispose", dispose, identity, tmp_path, budget=0.2),
        Call(
            "inspect",
            (sys.executable, str(PEER), inspection, identity, str(tmp_path)),
            0.2,
            expected_stdout=b"absent\n",
        ),
    )


def test_exact_raw_capture_and_subject_freeze(tmp_path):
    path, result = run(tmp_path, [call("bytes", "bytes")])
    assert result["ok"] is True
    assert (path / "bytes.stdout").read_bytes() == bytes.fromhex("00ff80410a")
    assert (path / "bytes.stderr").read_bytes() == b"separate stderr\n"
    assert result["subjects_before"] == result["subjects_after"]
    assert result["calls"][0]["elapsed"] >= 0
    assert json.loads((path / "plan.json").read_text())["calls"][0]["name"] == "bytes"


def test_overlong_call_fails_and_retains_partial_bytes(tmp_path):
    path, result = run(tmp_path, [call("overlong", "hang", budget=0.2), call("unreached", "bytes")])
    assert result["ok"] is False
    assert result["calls"][0]["reason"] == "deadline"
    assert (path / "overlong.stdout").read_bytes() == b"partial\n"
    assert result["unreached"] == ["unreached"]


def test_cli_interruption_kills_term_ignorer_and_controller_survives(tmp_path):
    controller_pid = os.getpid()
    _, result = run(tmp_path, [call("interrupt", "ignore-term", interrupt_after=0.3)])
    receipt = result["calls"][0]
    assert receipt["reason"] == "interrupted"
    assert receipt["returncode"] == -signal.SIGKILL
    assert [event["signal"] for event in receipt["signals"]] == ["TERM", "KILL"]
    assert receipt["reaped"] is True
    assert os.getpid() == controller_pid


def test_finally_disposal_independent_of_blocked_channel_and_failed_cleanup(tmp_path):
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id, cleanup="fail")
    unrelated = tmp_path / "unrelated"
    unrelated.write_bytes(b"keep")
    _, result = run(
        tmp_path, [call("blocked-transfer", "hang", budget=0.2)], [owned], run_id=run_id
    )
    assert result["ok"] is False
    assert result["resources"][0]["state"] == "disposed"
    assert result["resources"][0]["cleanup_ok"] is False
    assert not (tmp_path / owned.identity).exists()
    assert unrelated.read_bytes() == b"keep"


def test_failed_inspection_is_uncertain_even_after_successful_disposal(tmp_path):
    run_id = uuid.uuid4().hex
    _, result = run(tmp_path, [], [resource(tmp_path, run_id, inspection="fail")], run_id=run_id)
    assert result["ok"] is False
    assert result["resources"][0]["state"] == "uncertain"


def test_blocked_disposal_is_bounded_and_retains_exact_residual(tmp_path):
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id, dispose="ignore-term")
    _, result = run(tmp_path, [], [owned], run_id=run_id)
    assert result["ok"] is False
    assert result["resources"][0]["state"] == "uncertain"
    assert result["resources"][0]["identity"] == owned.identity
    assert (tmp_path / owned.identity).read_bytes() == b"owned"


def test_failed_transfer_preserves_raw_failure(tmp_path):
    path, result = run(tmp_path, [call("transfer", "fail")])
    assert result["ok"] is False
    assert result["calls"][0]["returncode"] == 23
    assert b"forced public interface failure" in (path / "transfer.stderr").read_bytes()


def test_hashing_drift_fails_instead_of_reinterpreting_measurement(tmp_path):
    subject = tmp_path / "input"
    subject.write_bytes(b"frozen")
    _, result = run(tmp_path, [call("drift", "drift", subject)], subjects=[subject])
    assert result["ok"] is False
    assert result["subjects_before"] != result["subjects_after"]


def test_foreign_resource_and_reused_output_are_rejected_before_calls(tmp_path):
    import pytest

    with pytest.raises(ValueError, match="identity"):
        run(tmp_path, [], [resource(tmp_path, "foreign")])
    run_id = uuid.uuid4().hex
    run(tmp_path, [], run_id=run_id)
    with pytest.raises(FileExistsError):
        run(tmp_path, [], run_id=run_id)


def test_owned_fork_is_terminated_and_known_child_absent(tmp_path):
    import pytest

    path, result = run(tmp_path, [call("fork", "fork", budget=0.3)])
    child = int((path / "fork.stdout").read_text())
    assert result["calls"][0]["reaped"] is True
    assert result["calls"][0]["elapsed"] < 0.3 + 0.15 + 0.3
    with pytest.raises(ProcessLookupError):
        os.kill(child, 0)


def test_blocked_cleanup_and_inspection_do_not_skip_disposal(tmp_path):
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id, cleanup="hang", inspection="hang")
    path, result = run(tmp_path, [], [owned], run_id=run_id)
    assert result["resources"][0]["state"] == "uncertain"
    assert json.loads((path / "dispose.json").read_text())["ok"] is True
    assert not (tmp_path / owned.identity).exists()


def test_spawn_error_is_not_numeric_exit(tmp_path):
    executable = tmp_path / "not-executable"
    executable.write_bytes(b"no executable permission")
    _, result = run(tmp_path, [Call("missing", (str(executable),), 1)])
    assert result["ok"] is False
    receipt = result["calls"][0]
    assert receipt["reason"] == "spawn_error"
    assert receipt["returncode"] is None


def test_exit_observation_error_fails_and_still_reaps_owned_process(tmp_path, monkeypatch):
    import controller

    def failed_observation(_pid):
        raise OSError("forced exit observer failure")

    monkeypatch.setattr(controller, "_exited", failed_observation)
    path, result = run(tmp_path, [call("observer-error", "hang", budget=0.2)])
    receipt = result["calls"][0]
    assert result["ok"] is False
    assert receipt["reason"] == "observation_error"
    assert receipt["reaped"] is True
    assert "forced exit observer failure" in receipt["error"]
    assert (path / "observer-error.json").is_file()


def test_review_expected_interruption_continues_same_resource(tmp_path):
    assert "expected_reason" in {field.name for field in fields(Call)}
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id)
    interrupted = replace(
        call("interrupt", "ignore-term", interrupt_after=0.2),
        expected_reason="interrupted",
        expected_exit=-signal.SIGKILL,
    )
    path, result = run(
        tmp_path, [interrupted, call("observe-after", "bytes")], [owned], run_id=run_id
    )
    assert result["ok"] is True
    assert result["unreached"] == []
    assert result["calls"][1]["reason"] == "interrupted"
    assert (path / "observe-after.stdout").read_bytes() == bytes.fromhex("00ff80410a")
    assert result["resources"][0]["state"] == "disposed"


def test_review_unexpected_interruption_still_stops(tmp_path):
    _, result = run(
        tmp_path,
        [call("interrupt", "ignore-term", interrupt_after=0.2), call("later", "bytes")],
    )
    assert result["ok"] is False
    assert result["unreached"] == ["later"]


def test_review_resource_deadline_cannot_be_declared_success(tmp_path):
    assert "expected_reason" in {field.name for field in fields(Call)}
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id)
    owned = replace(owned, dispose=replace(owned.dispose, expected_reason="deadline"))
    with pytest.raises(ValueError, match="resource.*exit"):
        run(tmp_path, [], [owned], run_id=run_id)
    assert not (tmp_path / owned.identity).exists()


@pytest.mark.parametrize("name", ["plan", "result", "subjects-before", "intent-1"])
def test_review_reserved_names_rejected_before_acquisition(tmp_path, name):
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id)
    owned = replace(owned, cleanup=replace(owned.cleanup, name=name))
    with pytest.raises(ValueError, match="reserved"):
        run(tmp_path, [], [owned], run_id=run_id)
    assert not (tmp_path / owned.identity).exists()


def test_review_receipt_io_exception_cannot_skip_other_disposal(tmp_path, monkeypatch):
    import controller

    run_id = uuid.uuid4().hex
    first = resource(tmp_path, run_id)
    second = resource(tmp_path, run_id)
    identity = f"diag-{run_id}-second"
    second = replace(
        second,
        identity=identity,
        **{
            action: replace(
                getattr(second, action),
                name=f"second-{action}",
                argv=tuple(
                    identity if arg == first.identity else arg
                    for arg in getattr(second, action).argv
                ),
            )
            for action in ("acquire", "cleanup", "dispose", "inspect")
        },
    )
    original = controller.write_json

    def fail_one_receipt(path, value):
        if path.name == "second-cleanup.json":
            raise OSError("forced receipt storage failure")
        original(path, value)

    monkeypatch.setattr(controller, "write_json", fail_one_receipt)
    _, result = run(tmp_path, [], [first, second], run_id=run_id)
    assert result["ok"] is False
    assert not (tmp_path / first.identity).exists()
    assert not (tmp_path / second.identity).exists()
    assert all(item["state"] == "disposed" for item in result["resources"])
    assert any(item.get("reason") == "controller_io_error" for item in result["calls"])


def test_review_skipped_acquisitions_are_unreached(tmp_path):
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id)
    _, result = run(tmp_path, [], [owned], subjects=[tmp_path / "missing"], run_id=run_id)
    assert result["ok"] is False
    assert result["unreached"] == ["acquire"]
    assert result["resources"] == []


def test_review_zero_exit_wrong_stdout_is_failure(tmp_path):
    bad_oracle = replace(call("wrong-absence", "bytes"), expected_stdout=b"absent\n")
    _, result = run(tmp_path, [bad_oracle, call("later", "bytes")])
    assert result["calls"][0]["returncode"] == 0
    assert result["calls"][0]["stdout_matches"] is False
    assert result["ok"] is False
    assert result["unreached"] == ["later"]


def test_review_preflight_keeps_missing_declared_subjects(tmp_path, monkeypatch):
    import preflight

    seen = []
    monkeypatch.setattr(preflight, "REPO", tmp_path / "missing-repo")
    monkeypatch.setattr(sys, "argv", ["preflight.py", str(tmp_path / "preflight")])
    monkeypatch.setattr(preflight.shutil, "which", lambda _name: "/usr/bin/true")
    monkeypatch.setattr(preflight, "execute", lambda plan, _output: seen.append(plan) or tmp_path)
    preflight.main()
    assert seen
    assert tmp_path / "missing-repo/Taskfile.yml" in seen[0].subjects
