"""Failure controls operate on real local processes and fake VM markers only."""

import errno
import json
import os
import signal
import subprocess
import sys
import time
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
        (
            PEER,
            Path(__file__),
            HERE / "pyproject.toml",
            HERE / "uv.lock",
            Path("/bin/sleep"),
            *subjects,
        ),
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
    _, result = run(tmp_path, [call("interrupt", "ignore-term", interrupt_after=0.3)])
    receipt = result["calls"][0]
    assert receipt["reason"] == "interrupted"
    assert receipt["returncode"] == -signal.SIGKILL
    assert [event["signal"] for event in receipt["signals"]] == ["TERM", "KILL"]
    assert receipt["reaped"] is True


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


def test_review_graceful_pid_exit_needs_no_kill(tmp_path):
    _, result = run(tmp_path, [call("term-exit", "hang", interrupt_after=0.2)])
    receipt = result["calls"][0]
    assert [event["signal"] for event in receipt["signals"]] == ["TERM"]
    assert receipt["leader_exit_monotonic"] >= receipt["signals"][0]["monotonic"]
    assert receipt["leader_exit_monotonic"] <= receipt["term_deadline_monotonic"]
    assert receipt["reaped_monotonic"] >= receipt["leader_exit_monotonic"]


def test_review_fork_handler_reaps_gracefully(tmp_path):
    path, result = run(tmp_path, [call("fork", "fork", budget=0.3)])
    receipt = result["calls"][0]
    assert receipt["returncode"] == 0
    assert receipt["termination_ok"] is True
    assert receipt["group_observations"][-1]["state"] == "zombie_only"
    child = int((path / "fork.stdout").read_text())
    check = inspect_pid(child)
    (path / "known-child-audit.json").write_text(json.dumps(check, indent=2))
    assert check["state"] == "absent"


def inspect_pid(pid):
    check = subprocess.run(
        ["/bin/ps", "-p", str(pid), "-o", "pid=,pgid=,stat=,lstart=,command="],
        capture_output=True,
        timeout=1,
    )
    state = "unknown"
    if check.returncode == 1 and check.stdout == b"" and check.stderr == b"":
        state = "absent"
    elif check.returncode == 0 and check.stderr == b"":
        fields = check.stdout.split(maxsplit=3)
        if len(fields) == 4 and int(fields[0]) == pid:
            state = "zombie" if fields[2].startswith(b"Z") else "live"
    return {
        "pid": pid,
        "state": state,
        "returncode": check.returncode,
        "stdout": check.stdout.decode(),
        "stderr": check.stderr.decode(),
        "monotonic": time.monotonic(),
    }


def scope_control(tmp_path, *, pid_only):
    release, ready = tmp_path / "release", tmp_path / "ready"
    workload = call(
        "scope",
        "scope-parent",
        release,
        ready,
        budget=0.3,
        interrupt_after=0.3 if pid_only else None,
    )
    path = None
    child = None
    try:
        path, result = run(tmp_path, [workload])
        child = int((path / "scope.stdout").read_text())
        observation = inspect_pid(child)
        # The expected live child must have this run's unique argv token and
        # still be in its recorded leader's group, not just reuse its PID.
        matches = observation["state"] == "live" and str(release) in observation["stdout"]
        if matches:
            matches = int(observation["stdout"].split()[1]) == result["calls"][0]["pgid"]
        verdict = matches if pid_only else observation["state"] == "absent"
        (path / "scope-control.json").write_text(
            json.dumps(
                {
                    "pid_only": pid_only,
                    "ok": verdict,
                    "observation": observation,
                },
                indent=2,
            )
        )
        return verdict
    finally:
        # Cooperative cleanup cannot signal an identity which has been reused.
        release.write_bytes(b"release")
        if child is not None:
            end = time.monotonic() + 1
            observations = [inspect_pid(child)]
            while observations[-1]["state"] == "live" and time.monotonic() < end:
                time.sleep(0.005)
                observations.append(inspect_pid(child))
            (path / "known-child-audit.json").write_text(json.dumps(observations, indent=2))
            assert observations[-1]["state"] == "absent"


@pytest.mark.parametrize("pid_only", [True, False])
def test_review_signal_scope_has_opposed_child_outcomes(tmp_path, pid_only):
    assert scope_control(tmp_path, pid_only=pid_only) is True


@pytest.mark.parametrize("pid_only", [True, False])
def test_review_wrong_signal_scope_is_detected(tmp_path, monkeypatch, pid_only):
    if pid_only:
        monkeypatch.setattr(os, "kill", os.killpg)
    else:
        monkeypatch.setattr(os, "killpg", os.kill)
    assert scope_control(tmp_path, pid_only=pid_only) is False


def test_review_controller_survival_is_observed_by_another_process(tmp_path):
    nested_root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "nested-evidence"))
    # Outer controller is a separate parent and sees the inner controller's
    # actual exit and post-interruption output. A signal to it loses this proof.
    path, result = run(tmp_path, [call("nested", "controller", nested_root)])
    assert result["ok"] is True
    output = (path / "nested.stdout").read_text().splitlines()
    assert output[-1] == "controller continued"
    nested = json.loads((Path(output[0]) / "result.json").read_text())
    assert nested["ok"] is True
    assert [item["name"] for item in nested["calls"]] == ["interrupt", "after"]


def test_review_group_signals_and_inspection_finish_before_reaping(tmp_path, monkeypatch):
    reaped = set()
    original_wait = subprocess.Popen.wait
    original_killpg = os.killpg

    def wait(process, *args, **kwargs):
        code = original_wait(process, *args, **kwargs)
        reaped.add(process.pid)
        return code

    def killpg(pid, sig):
        if pid in reaped:
            raise RuntimeError("group identity already released by reap")
        original_killpg(pid, sig)

    monkeypatch.setattr(subprocess.Popen, "wait", wait)
    monkeypatch.setattr(os, "killpg", killpg)
    _, result = run(tmp_path, [call("pin", "ignore-term", budget=0.2)])
    receipt = result["calls"][0]
    assert receipt["returncode"] == -9
    assert receipt["termination_ok"] is True
    assert all(
        item["finished_monotonic"] <= receipt["reaped_monotonic"]
        for item in receipt["group_observations"]
    )
    assert receipt["reaped_monotonic"] <= receipt["kill_reap_deadline_monotonic"]
    assert receipt["within_process_bounds"] is True
    assert receipt["kill_reap_deadline_monotonic"] <= receipt["term_deadline_monotonic"] + 0.3
    # Prove this guard rejects a released identity without sending any signal.
    with pytest.raises(RuntimeError, match="identity already released"):
        killpg(receipt["pid"], 0)


@pytest.mark.parametrize("failure", ["usage", "timeout", "foreign"])
def test_review_failed_group_inspection_is_never_absence(tmp_path, monkeypatch, failure):
    original_run = subprocess.run

    def broken_ps(argv, *args, **kwargs):
        if argv[0] != "/bin/ps":
            return original_run(argv, *args, **kwargs)
        if failure == "timeout":
            raise subprocess.TimeoutExpired(argv, kwargs["timeout"], b"partial")
        if failure == "foreign":
            return subprocess.CompletedProcess(argv, 0, b"1 1 S\n", b"")
        return subprocess.CompletedProcess(argv, 1, b"", b"ps: usage error\n")

    monkeypatch.setattr(subprocess, "run", broken_ps)
    workload = replace(
        call("unknown", "hang", budget=0.2), expected_reason="deadline", expected_exit=-15
    )
    _, result = run(tmp_path, [workload])
    receipt = result["calls"][0]
    assert receipt["reaped"] is True
    assert receipt["returncode"] == -15
    assert receipt["termination_ok"] is False
    assert receipt["group_observations"][-1]["state"] == "unknown"
    assert result["ok"] is False


def test_review_live_signal_permission_failure_is_retained(tmp_path, monkeypatch):
    original = os.killpg

    def deny_term(pid, sig):
        if sig == signal.SIGTERM:
            raise PermissionError(errno.EPERM, "forced live permission failure")
        original(pid, sig)

    monkeypatch.setattr(os, "killpg", deny_term)
    workload = replace(
        call("denied", "hang", budget=0.2), expected_reason="deadline", expected_exit=-9
    )
    _, result = run(tmp_path, [workload])
    receipt = result["calls"][0]
    assert receipt["signals"][0]["outcome"] == "permission_failure"
    assert receipt["signals"][0]["permission_observation"]["state"] == "live"
    assert receipt["returncode"] == -9
    assert receipt["termination_ok"] is False
    assert result["ok"] is False


def test_review_zombie_only_eperm_is_separate_from_permission_failure(tmp_path, monkeypatch):
    import controller

    original = os.killpg

    def zombie_race(pid, sig):
        original(pid, sig)
        if sig == signal.SIGTERM:
            assert controller._wait(pid, time.monotonic() + 0.1)
            raise PermissionError(errno.EPERM, "forced zombie-only return race")

    monkeypatch.setattr(os, "killpg", zombie_race)
    workload = replace(
        call("zombie", "hang", budget=0.2), expected_reason="deadline", expected_exit=-15
    )
    _, result = run(tmp_path, [workload])
    receipt = result["calls"][0]
    assert [item["outcome"] for item in receipt["signals"]] == ["zombie_only_eperm"]
    assert receipt["termination_ok"] is True
    assert result["ok"] is True


@pytest.mark.parametrize("drop_kill", [False, True])
def test_review_exited_leader_does_not_hide_a_surviving_group_member(
    tmp_path, monkeypatch, drop_kill
):
    release, ready = tmp_path / "release", tmp_path / "ready"
    original = os.killpg

    def signal_group(pid, sig):
        if not (drop_kill and sig == signal.SIGKILL):
            original(pid, sig)

    monkeypatch.setattr(os, "killpg", signal_group)
    path = None
    child = None
    try:
        path, result = run(
            tmp_path,
            [call("stubborn", "scope-parent", release, ready, "stubborn-child", budget=0.3)],
        )
        child = int((path / "stubborn.stdout").read_text())
        receipt = result["calls"][0]
        assert receipt["returncode"] == 0
        assert [item["signal"] for item in receipt["signals"]] == ["TERM", "KILL"]
        assert receipt["leader_exit_monotonic"] < receipt["signals"][1]["monotonic"]
        assert receipt["termination_ok"] is (not drop_kill)
        if drop_kill:
            group = receipt["group_observations"][-1]
            assert group["state"] == "live"
            assert any(
                item["pid"] == child and not item["stat"].startswith("Z")
                for item in group["members"]
            )
            assert inspect_pid(child)["state"] == "live"
    finally:
        release.write_bytes(b"release")
        if child is not None:
            end = time.monotonic() + 1
            observations = [inspect_pid(child)]
            while observations[-1]["state"] != "absent" and time.monotonic() < end:
                time.sleep(0.005)
                observations.append(inspect_pid(child))
            (path / "known-child-audit.json").write_text(json.dumps(observations, indent=2))
            assert observations[-1]["state"] == "absent"
