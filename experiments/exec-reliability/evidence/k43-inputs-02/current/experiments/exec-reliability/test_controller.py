"""Failure controls operate on real local processes and fake VM markers only."""

import errno
import inspect
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


@pytest.mark.parametrize("failed_call", ["acquire", "workload", "cleanup", "dispose", "inspect"])
def test_review_inspection_failure_is_sticky_during_safety(tmp_path, monkeypatch, failed_call):
    import controller

    order = ["acquire", "workload", "cleanup", "dispose", "inspect"]
    failure_index = order.index(failed_call) + 1
    queries = []
    original_snapshot = controller._group_snapshot

    def snapshot(pid, deadline):
        observation = original_snapshot(pid, deadline)
        queries.append(pid)
        if len(queries) == failure_index:
            # Adapter damage after a real query. Every later real table would
            # succeed, but retrying cannot restore the run's inspection authority.
            observation.update(state="unknown", error="injected invalid table", stdout=b"bad row\n")
        return observation

    monkeypatch.setattr(controller, "_group_snapshot", snapshot)
    run_id = uuid.uuid4().hex
    owned = resource(tmp_path, run_id)
    workloads = [call("workload", "bytes")]
    if failed_call in order[:2]:
        workloads.append(call("unreached", "bytes"))
    path, result = run(
        tmp_path,
        workloads,
        [owned],
        run_id=run_id,
    )
    audit = {
        "adapter": "invalid table after real query",
        "failed_call": failed_call,
        "queries": queries,
        "leaders": [inspect_pid(receipt["pid"]) for receipt in result["calls"]],
        "marker_absent": not (tmp_path / owned.identity).exists(),
    }
    (path / "sticky-inspection-audit.json").write_text(json.dumps(audit, indent=2))
    assert all(item["state"] == "absent" for item in audit["leaders"])
    assert audit["marker_absent"] is True
    assert all(receipt["reaped"] for receipt in result["calls"])
    assert result["ok"] is False
    assert result["resources"][0]["state"] == "uncertain"
    assert len(queries) == failure_index
    first = next(item for item in result["calls"] if item["name"] == failed_call)
    assert result["inspection_failure"] == {
        "call": failed_call,
        "observation": first["group_observations"][0],
    }
    safety = [item for item in result["calls"] if item["name"] in order[2:]]
    assert [item["name"] for item in safety] == order[2:]
    for receipt in result["calls"][result["calls"].index(first) + 1 :]:
        observation = receipt["group_observations"][0]
        assert observation["state"] == "unknown"
        assert observation["probe_outcome"] == "not_attempted"
        assert observation["stdout"] == ""
        assert failed_call in observation["error"]
        assert receipt["termination_ok"] is False
    assert ("unreached" in result["unreached"]) == (failed_call in order[:2])

    # The failure belongs to this execute call, never to the module or next run.
    monkeypatch.setattr(controller, "_group_snapshot", original_snapshot)
    healthy_path, healthy = run(tmp_path, [call("fresh-run", "bytes")])
    assert healthy["ok"] is True
    assert healthy["inspection_failure"] is None
    fresh = inspect_pid(healthy["calls"][0]["pid"])
    (healthy_path / "fresh-run-audit.json").write_text(json.dumps(fresh, indent=2))
    assert fresh["state"] == "absent"


def test_review_sticky_failure_preserves_reverse_safety_and_signals(tmp_path, monkeypatch):
    import controller

    queries = []
    original_snapshot = controller._group_snapshot

    def snapshot(pid, deadline):
        observation = original_snapshot(pid, deadline)
        queries.append(pid)
        if len(queries) == 3:
            observation.update(state="unknown", error="injected inspector failure")
        return observation

    monkeypatch.setattr(controller, "_group_snapshot", snapshot)
    run_id = uuid.uuid4().hex
    resources = []
    for suffix in ("first", "second"):
        template = resource(tmp_path, run_id, cleanup="hang" if suffix == "second" else "bytes")
        identity = f"{template.identity}-{suffix}"
        actions = {
            field: replace(
                getattr(template, field),
                name=f"{field}-{suffix}",
                argv=tuple(
                    identity if token == template.identity else token
                    for token in getattr(template, field).argv
                ),
            )
            for field in ("acquire", "cleanup", "dispose", "inspect")
        }
        resources.append(Resource("vm", identity, **actions))
    path, result = run(
        tmp_path,
        [call("workload", "bytes"), call("unreached", "bytes")],
        resources,
        run_id=run_id,
    )
    audit = {
        "adapter": "third real table changed to unknown",
        "queries": queries,
        "leaders": [inspect_pid(receipt["pid"]) for receipt in result["calls"]],
        "markers_absent": [not (tmp_path / item.identity).exists() for item in resources],
    }
    (path / "reverse-safety-audit.json").write_text(json.dumps(audit, indent=2))
    assert all(item["state"] == "absent" for item in audit["leaders"])
    assert all(audit["markers_absent"])
    assert result["ok"] is False
    assert result["unreached"] == ["unreached"]
    assert [item["name"] for item in result["calls"][3:]] == [
        f"{action}-{suffix}"
        for suffix in ("second", "first")
        for action in ("cleanup", "dispose", "inspect")
    ]
    assert len(queries) == 3
    assert all(item["state"] == "uncertain" for item in result["resources"])
    stalled = result["calls"][3]
    assert stalled["reason"] == "deadline"
    assert [item["signal"] for item in stalled["signals"]] == ["TERM", "KILL"]
    assert stalled["reaped"] is True
    assert stalled["within_process_bounds"] is True
    assert stalled["group_observations"][0]["probe_outcome"] == "not_attempted"


def test_review_live_table_does_not_poison_later_inspection(tmp_path, monkeypatch):
    import controller

    original_snapshot = controller._group_snapshot
    queries = []

    def snapshot(pid, deadline):
        observation = original_snapshot(pid, deadline)
        queries.append(pid)
        if len(queries) == 1:
            # Classification adapter; real live-child behavior has separate controls.
            observation["state"] = "live"
        return observation

    monkeypatch.setattr(controller, "_group_snapshot", snapshot)
    run_id = uuid.uuid4().hex
    path, result = run(tmp_path, [], [resource(tmp_path, run_id)], run_id=run_id)
    audit = {
        "adapter": "first real table classified live",
        "queries": queries,
        "leaders": [inspect_pid(receipt["pid"]) for receipt in result["calls"]],
    }
    (path / "live-classification-audit.json").write_text(json.dumps(audit, indent=2))
    assert all(item["state"] == "absent" for item in audit["leaders"])
    assert result["ok"] is False
    assert result["calls"][0]["termination_ok"] is False
    assert result["inspection_failure"] is None
    assert result["resources"][0]["state"] == "disposed"
    assert len(queries) == 4


@pytest.mark.parametrize("pid_only", [False, True])
def test_review_final_query_requires_observed_exit(tmp_path, monkeypatch, pid_only):
    import controller

    queries = []
    original_snapshot = controller._group_snapshot

    def snapshot(pid, deadline):
        queries.append(time.monotonic())
        return original_snapshot(pid, deadline)

    # Simulate unavailable exit notification, while all signals and reaps act
    # on a real child. No process-table operation is authorized without exit.
    monkeypatch.setattr(controller, "_exited", lambda pid: False)
    monkeypatch.setattr(controller, "_group_snapshot", snapshot)
    path, result = run(
        tmp_path,
        [call("missing-exit", "hang", budget=0.15, interrupt_after=0.15 if pid_only else None)],
    )
    receipt = result["calls"][0]
    audit = {"simulation": "withheld exit notification", "queries": queries}
    audit["after_controller"] = inspect_pid(receipt["pid"])
    (path / "final-query-audit.json").write_text(json.dumps(audit, indent=2))
    assert audit["after_controller"]["state"] == "absent"
    assert receipt["reaped"] is True
    assert result["ok"] is False
    assert queries == []
    assert receipt["group_observations"][0]["state"] == "unknown"
    assert receipt["group_observations"][0]["probe_outcome"] == "not_attempted"


def test_review_final_live_table_cannot_be_retried(tmp_path, monkeypatch):
    import controller

    release, ready = tmp_path / "release", tmp_path / "ready"
    original_killpg, original_snapshot = os.killpg, controller._group_snapshot
    observations = []

    def killpg(pid, sig):
        if sig != signal.SIGKILL:
            original_killpg(pid, sig)

    def snapshot(pid, deadline):
        observation = original_snapshot(pid, deadline)
        observations.append(observation)
        # The first real table contains the live gated child. Releasing it
        # makes later tables favorable, but cannot repair that final evidence.
        release.write_bytes(b"release")
        return observation

    monkeypatch.setattr(os, "killpg", killpg)
    monkeypatch.setattr(controller, "_group_snapshot", snapshot)
    path, child = None, None
    try:
        path, result = run(
            tmp_path,
            [call("single-live", "scope-parent", release, ready, "stubborn-child", budget=0.3)],
        )
        child = int((path / "single-live.stdout").read_text())
        receipt = result["calls"][0]
        assert observations[0]["state"] == "live"
        assert receipt["termination_ok"] is False
        assert result["ok"] is False
        assert len(observations) == 1
        assert any(item["pid"] == child for item in receipt["residual_members"])
    finally:
        release.write_bytes(b"release")
        if child is not None:
            end = time.monotonic() + 1
            audit = [inspect_pid(child)]
            while audit[-1]["state"] != "absent" and time.monotonic() < end:
                time.sleep(0.005)
                audit.append(inspect_pid(child))
            (path / "known-child-audit.json").write_text(json.dumps(audit, indent=2))
            assert audit[-1]["state"] == "absent"


def test_review_kill_eperm_uses_only_final_postsignal_table(tmp_path, monkeypatch):
    original_killpg = os.killpg

    def killpg(pid, sig):
        original_killpg(pid, sig)
        if sig == signal.SIGKILL:
            raise PermissionError(errno.EPERM, "simulated KILL return for real zombie group")

    monkeypatch.setattr(os, "killpg", killpg)
    workload = replace(
        call("final-eperm", "hang", budget=0.2), expected_reason="deadline", expected_exit=-15
    )
    _, result = run(tmp_path, [workload])
    receipt = result["calls"][0]
    final_signal = receipt["signals"][-1]
    assert result["ok"] is True
    assert final_signal["outcome"] == "zombie_only_eperm"
    assert len(receipt["group_observations"]) == 1
    observation = receipt["group_observations"][0]
    assert final_signal["finished_monotonic"] <= observation["started_monotonic"]
    assert receipt["leader_exit_monotonic"] <= observation["started_monotonic"]


@pytest.mark.parametrize("reap_state", ["withheld", "missing", "expired"])
def test_review_nonblocking_reap_retains_failed_owner(tmp_path, monkeypatch, reap_state):
    import controller

    original_waitpid = os.waitpid
    attempts = []
    targets = []
    original_snapshot = controller._group_snapshot

    def snapshot(pid, deadline):
        targets.append(pid)
        result = original_snapshot(pid, deadline)
        if reap_state == "expired":
            time.sleep(max(0, deadline - time.monotonic()) + 0.11)
        return result

    def waitpid(pid, options):
        if pid in targets:
            attempts.append((pid, options, time.monotonic()))
            assert options == os.WNOHANG
            if reap_state == "missing":
                raise ChildProcessError(errno.ECHILD, "simulated ownership loss")
            return (0, 0)
        return original_waitpid(pid, options)

    monkeypatch.setattr(controller, "_group_snapshot", snapshot)
    monkeypatch.setattr(os, "waitpid", waitpid)
    path, result = run(tmp_path, [call("retained-leader", "bytes")])
    receipt = result["calls"][0]
    pid = receipt["pid"]
    audit = {"simulation": reap_state, "pid": pid, "attempts": attempts}
    try:
        assert result["ok"] is False
        assert receipt["reaped"] is False
        assert receipt["residual_leader_pid"] == pid
        assert receipt["within_process_bounds"] is False
        assert (not attempts) if reap_state == "expired" else bool(attempts)
        assert all(t < receipt["reap_deadline_monotonic"] for _, _, t in attempts)
        owner = controller._workload_owners[pid]
        owner.__del__()
        subprocess.run(("/usr/bin/true",), check=True)
        audit["before_independent_reap"] = inspect_pid(pid)
        assert audit["before_independent_reap"]["state"] == "zombie"
        assert owner.returncode is None
    finally:
        # Independent exact-child audit, outside the controller's closed authority.
        try:
            waited_pid, status = original_waitpid(pid, os.WNOHANG)
        except ChildProcessError as error:
            # Keep the baseline's already-reaped outcome without masking the
            # behavioral assertion above during the frozen RED replay.
            audit["independent_wait_error"] = str(error)
        else:
            audit["independent_wait"] = [waited_pid, status]
            assert waited_pid == pid
            assert os.waitstatus_to_exitcode(status) == 0
        audit["after_independent_reap"] = inspect_pid(pid)
        assert audit["after_independent_reap"]["state"] == "absent"
        getattr(controller, "_workload_owners", {}).pop(pid, None)
        (path / "leader-reap-audit.json").write_text(json.dumps(audit, indent=2))


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
    _, result, _, observation = owned_fork_run(tmp_path)
    assert result["calls"][0]["reaped"] is True
    assert result["calls"][0]["elapsed"] < 0.3 + 0.15 + 0.3
    assert observation["state"] == "absent"


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
    assert preflight.HERE / "inspection.py" in seen[0].subjects
    assert preflight.HERE / "test_inspection.py" in seen[0].subjects


def test_review_graceful_pid_exit_needs_no_kill(tmp_path):
    _, result = run(tmp_path, [call("term-exit", "hang", interrupt_after=0.2)])
    receipt = result["calls"][0]
    assert [event["signal"] for event in receipt["signals"]] == ["TERM"]
    assert receipt["leader_exit_monotonic"] >= receipt["signals"][0]["monotonic"]
    assert receipt["leader_exit_monotonic"] <= receipt["term_deadline_monotonic"]
    assert receipt["reaped_monotonic"] >= receipt["leader_exit_monotonic"]


def test_review_fork_handler_reaps_gracefully(tmp_path):
    _, result, _, check = owned_fork_run(tmp_path)
    receipt = result["calls"][0]
    assert receipt["returncode"] == 0
    assert receipt["termination_ok"] is True
    assert receipt["group_observations"][-1]["state"] == "zombie_only"
    assert check["state"] == "absent"


def owned_fork_run(tmp_path):
    release, ready = tmp_path / "release", tmp_path / "ready"
    path = None
    child = None
    try:
        path, result = run(tmp_path, [call("fork", "fork", release, ready, budget=0.3)])
        child = int(ready.read_text())
        return path, result, child, inspect_pid(child)
    finally:
        release.write_bytes(b"release")
        if child is None and ready.exists():
            child = int(ready.read_text())
        if child is not None:
            end = time.monotonic() + 1
            observations = [inspect_pid(child)]
            while observations[-1]["state"] != "absent" and time.monotonic() < end:
                time.sleep(0.005)
                observations.append(inspect_pid(child))
            target = path or tmp_path
            (target / "known-child-audit.json").write_text(json.dumps(observations, indent=2))
            assert observations[-1]["state"] == "absent"


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
    import controller

    reaped = set()
    original_waitpid = os.waitpid
    original_killpg = os.killpg

    def waitpid(pid, options):
        outcome = original_waitpid(pid, options)
        if outcome[0] > 0:
            reaped.add(outcome[0])
        return outcome

    def forbidden_wait(*args, **kwargs):
        raise AssertionError("blocking workload wait")

    def killpg(pid, sig):
        if pid in reaped:
            raise RuntimeError("group identity already released by reap")
        original_killpg(pid, sig)

    monkeypatch.setattr(os, "waitpid", waitpid)
    monkeypatch.setattr(controller._WorkloadProcess, "wait", forbidden_wait)
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
    assert receipt["authority_closed_monotonic"] <= receipt["reaped_monotonic"]
    assert all(
        item["finished_monotonic"] <= receipt["authority_closed_monotonic"]
        for item in receipt["group_observations"]
    )
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


@pytest.mark.parametrize("mode", ["hang", "ignore-term"])
def test_review_group_kill_precedes_inspection(tmp_path, monkeypatch, mode):
    # Moving ps into TERM grace or skipping KILL after leader exit must fail.
    original_run, original_killpg = subprocess.run, os.killpg
    query_times, kill_times = [], []

    def slow_ps(argv, *args, **kwargs):
        if argv[0] == "/bin/ps":
            query_times.append(time.monotonic())
            if not kill_times:
                # Deliberately slow the real table operation in the broken
                # ordering. No fake process rows or signal effects are used.
                time.sleep(0.2)
        return original_run(argv, *args, **kwargs)

    def signal_group(pid, sig):
        if sig == signal.SIGKILL:
            kill_times.append(time.monotonic())
        original_killpg(pid, sig)

    monkeypatch.setattr(subprocess, "run", slow_ps)
    monkeypatch.setattr(os, "killpg", signal_group)
    workload = replace(
        call("ordering", mode, budget=0.2),
        expected_reason="deadline",
        expected_exit=-15 if mode == "hang" else -9,
    )
    path, result = run(tmp_path, [workload])
    (path / "inspection-order.json").write_text(
        json.dumps({"queries": query_times, "kills": kill_times}, indent=2) + "\n"
    )
    receipt = result["calls"][0]
    assert [item["signal"] for item in receipt["signals"]] == ["TERM", "KILL"]
    assert kill_times[0] <= query_times[0]
    assert 0 <= kill_times[0] - receipt["term_deadline_monotonic"] < 0.06
    assert receipt["reaped"] is True
    assert result["ok"] is True


def test_review_live_signal_permission_uncertainty_is_retained(tmp_path, monkeypatch):
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
    assert receipt["signals"][0]["outcome"] == "permission_unknown"
    assert "permission_observation" not in receipt["signals"][0]
    assert receipt["returncode"] == -9
    assert receipt["termination_ok"] is False
    assert result["ok"] is False


def test_review_later_zombie_table_cannot_repair_earlier_eperm(tmp_path, monkeypatch):
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
    assert receipt["signals"][0]["outcome"] == "permission_unknown"
    # macOS may deny KILL to an already zombie-only group. That final
    # observation still cannot repair the earlier denied TERM receipt.
    assert receipt["signals"][1]["outcome"] in {"sent", "zombie_only_eperm"}
    assert receipt["group_observations"][-1]["state"] == "zombie_only"
    assert receipt["termination_ok"] is False
    assert result["ok"] is False


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


@pytest.mark.parametrize("interrupted", [False, True])
def test_review_live_group_is_failure_even_with_expected_leader_exit(tmp_path, interrupted):
    release, ready = tmp_path / "release", tmp_path / "ready"
    workload = call(
        "live-exit",
        "scope-parent",
        release,
        ready,
        budget=5,
        interrupt_after=0.3 if interrupted else None,
    )
    workload = replace(workload, expected_reason="interrupted" if interrupted else "exit")
    path = None
    child = None
    try:
        path, result = run(tmp_path, [workload, call("later", "bytes")])
        child = int((path / "live-exit.stdout").read_text())
        receipt = result["calls"][0]
        assert receipt["returncode"] == 0
        assert receipt["termination_ok"] is False
        assert result["ok"] is False
        assert result["unreached"] == ["later"]
        assert any(item["pid"] == child for item in receipt["residual_members"])
        assert receipt["residual_remedy"]
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


def test_review_checker_unreaped_timeout_still_retains_verdict(tmp_path, monkeypatch):
    import self_check

    class Unreapable:
        pid = 123456789

        def wait(self, timeout):
            raise subprocess.TimeoutExpired("forced checker", timeout)

    monkeypatch.setattr(subprocess, "Popen", lambda *args, **kwargs: Unreapable())
    monkeypatch.setattr(os, "killpg", lambda *args: None)
    output = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path)) / uuid.uuid4().hex
    monkeypatch.setattr(sys, "argv", ["self_check.py", str(output)])
    with pytest.raises(SystemExit) as status:
        self_check.main()
    assert status.value.code == 1
    verdict = json.loads((output / "verdict.json").read_text())
    assert verdict["ok"] is False
    assert len(verdict["checks"]) == 1
    assert verdict["checks"][0]["reaped"] is False
    assert verdict["checks"][0]["returncode"] is None
    assert verdict["checks"][0]["residual_remedy"]


def test_review_fork_publishes_child_before_stdout_and_releases_on_failure(tmp_path):
    release, ready = tmp_path / "release", tmp_path / "ready"
    peer = subprocess.Popen(
        (sys.executable, str(PEER), "fork", str(release), str(ready)),
        stdout=subprocess.DEVNULL,
        start_new_session=True,
    )
    child = None
    try:
        end = time.monotonic() + 1
        while not ready.exists() and time.monotonic() < end:
            time.sleep(0.005)
        assert ready.exists(), "child identity was not published independently of stdout"
        child = int(ready.read_text())
        assert str(release) in inspect_pid(child)["stdout"]
        # Model controller failure before the stdout PID can be read.
        peer.terminate()
    finally:
        release.write_bytes(b"release")
        if child is None:
            # Old fixture lacks the cooperative release protocol. Its leader
            # remains pinned here; cleanup only this new session's group.
            os.killpg(peer.pid, signal.SIGTERM)
        peer.wait(timeout=2)
        observations = []
        if child is not None:
            end = time.monotonic() + 1
            observations = [inspect_pid(child)]
            while observations[-1]["state"] != "absent" and time.monotonic() < end:
                time.sleep(0.005)
                observations.append(inspect_pid(child))
        audit = {"pid": peer.pid, "child": child, "observations": observations}
        target = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path)) / uuid.uuid4().hex
        target.mkdir()
        (target / "fork-failure-audit.json").write_text(json.dumps(audit, indent=2))
        if child is not None:
            assert observations[-1]["state"] == "absent"


@pytest.mark.parametrize("reaper", [None, "poll", "wait", "waitpid", "wait-any"])
def test_review_pinning_control_detects_real_early_reap(tmp_path, monkeypatch, reaper):
    import controller

    processes = {}
    original_init = subprocess.Popen.__init__
    original_wait = controller._wait
    original_snapshot = controller._group_snapshot
    observations = []

    def register(process, argv, *args, **kwargs):
        original_init(process, argv, *args, **kwargs)
        if str(PEER) in argv:
            processes[process.pid] = process

    def wait(pid, deadline):
        exited = original_wait(pid, deadline)
        if exited and reaper:
            process = processes[pid]
            if reaper == "poll":
                process.poll()
            elif reaper == "wait":
                process.wait(timeout=1)
            elif reaper == "waitpid":
                os.waitpid(pid, 0)
            else:
                assert os.wait()[0] == pid
        return exited

    def pinned_snapshot(pid, deadline):
        observation = inspect_pid(pid)
        observations.append(observation)
        if observation["state"] == "absent":
            raise RuntimeError("group identity released before inspection")
        return original_snapshot(pid, deadline)

    monkeypatch.setattr(subprocess.Popen, "__init__", register)
    monkeypatch.setattr(controller, "_wait", wait)
    monkeypatch.setattr(controller, "_group_snapshot", pinned_snapshot)
    run_id = uuid.uuid4().hex
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "evidence")) / run_id
    try:
        if reaper:
            with pytest.raises(RuntimeError, match="identity released"):
                run(tmp_path, [call("pin-exit", "bytes")], run_id=run_id)
        else:
            _, result = run(tmp_path, [call("pin-exit", "bytes")], run_id=run_id)
            assert result["ok"] is True
    finally:
        (root / "pinning-control.json").write_text(
            json.dumps(
                {
                    "reaper": reaper,
                    "observations": observations,
                },
                indent=2,
            )
        )


def mutate_function(monkeypatch, module, name, old, new):
    source = inspect.getsource(getattr(module, name))
    assert source.count(old) == 1
    namespace = {}
    exec(
        compile(source.replace(old, new), f"<controlled-{name}-mutation>", "exec"),
        module.__dict__,
        namespace,
    )
    monkeypatch.setattr(module, name, namespace[name])


@pytest.mark.parametrize("remove_gate", [False, True])
def test_review_isolated_reap_overrun_and_removed_gate(tmp_path, monkeypatch, remove_gate):
    import controller

    original_waitpid = os.waitpid

    def delayed_reap(pid, options):
        outcome = original_waitpid(pid, options)
        if pid in controller._workload_owners and outcome[0] == pid:
            assert options == os.WNOHANG
            # Simulate a delayed return from the nonblocking system call;
            # retain the late-result gate without pretending OS unreapability.
            time.sleep(0.35)
        return outcome

    monkeypatch.setattr(os, "waitpid", delayed_reap)
    if remove_gate:
        mutate_function(
            monkeypatch,
            controller,
            "_call",
            'and receipt["within_process_bounds"] is True',
            "and True",
        )
    _, result = run(tmp_path, [call("late-reap", "bytes")])
    receipt = result["calls"][0]
    assert receipt["termination_ok"] is True
    assert receipt["returncode"] == 0
    assert receipt["within_process_bounds"] is False
    assert result["ok"] is remove_gate


@pytest.mark.parametrize("remove_cap", [False, True])
def test_review_late_observation_cannot_extend_reap_deadline(tmp_path, monkeypatch, remove_cap):
    import controller

    original_exited = controller._exited
    original_killpg = os.killpg
    term_sent = False
    delayed = False

    def signal_group(pid, sig):
        nonlocal term_sent
        original_killpg(pid, sig)
        term_sent = term_sent or sig == signal.SIGTERM

    def late_observation(pid):
        nonlocal delayed
        if term_sent and not delayed:
            delayed = True
            time.sleep(0.2)
        return original_exited(pid)

    monkeypatch.setattr(os, "killpg", signal_group)
    monkeypatch.setattr(controller, "_exited", late_observation)
    if remove_cap:
        mutate_function(
            monkeypatch,
            controller,
            "_terminate",
            "term_deadline + plan.kill_wait",
            "time.monotonic() + plan.kill_wait",
        )
    _, result = run(tmp_path, [call("late-observe", "hang", budget=0.2)])
    receipt = result["calls"][0]
    capped = receipt["kill_reap_deadline_monotonic"] <= receipt["term_deadline_monotonic"] + 0.3
    assert capped is (not remove_cap)


def test_review_pid_late_observation_cannot_extend_reap_deadline(tmp_path, monkeypatch):
    import controller

    original_exited = controller._exited
    original_kill = os.kill
    term_sent = False
    delayed = False

    def signal_pid(pid, sig):
        nonlocal term_sent
        original_kill(pid, sig)
        term_sent = term_sent or sig == signal.SIGTERM

    def late_observation(pid):
        nonlocal delayed
        if term_sent and not delayed:
            delayed = True
            time.sleep(0.2)
        return original_exited(pid)

    monkeypatch.setattr(os, "kill", signal_pid)
    monkeypatch.setattr(controller, "_exited", late_observation)
    _, result = run(tmp_path, [call("pid-late-observe", "hang", interrupt_after=0.2)])
    receipt = result["calls"][0]
    assert delayed
    assert receipt["reason"] == "interrupted"
    assert receipt["leader_exit_monotonic"] > receipt["term_deadline_monotonic"]
    assert receipt["reaped"] is True
    assert all(event["scope"] == "pid" for event in receipt["signals"])
    assert receipt["kill_reap_deadline_monotonic"] <= receipt["term_deadline_monotonic"] + 0.3


@pytest.mark.parametrize("self_signal", [False, True])
def test_review_survival_checker_rejects_controller_self_signal(tmp_path, monkeypatch, self_signal):
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path / "nested-evidence"))
    root.mkdir(exist_ok=True)
    owned_file = root / f"self-signal-{uuid.uuid4().hex}.json"
    # Replace only the inner Python invocation. Its controller deliberately
    # misdirects the child's SIGTERM to itself. The independent parent stays live.
    script = f"""
import json, os, runpy, signal, sys
from pathlib import Path
original = os.kill
def misdirect(pid, sig):
    if sig == signal.SIGTERM:
        Path({str(owned_file)!r}).write_text(json.dumps({{"pid": pid}}))
        original(pid, signal.SIGKILL)
        original(os.getpid(), sig)
    else:
        original(pid, sig)
os.kill = misdirect
sys.argv = [{str(PEER)!r}, 'controller', {str(root)!r}]
runpy.run_path({str(PEER)!r}, run_name='__main__')
"""
    workload = call("self-signal", "controller", root)
    if self_signal:
        workload = replace(workload, argv=(sys.executable, "-c", script))
    _, result = run(tmp_path, [workload])
    assert result["ok"] is (not self_signal)
    if self_signal:
        assert result["calls"][0]["returncode"] == -signal.SIGTERM
        child = json.loads(owned_file.read_text())["pid"]
        end = time.monotonic() + 1
        observations = [inspect_pid(child)]
        while observations[-1]["state"] != "absent" and time.monotonic() < end:
            time.sleep(0.005)
            observations.append(inspect_pid(child))
        owned_file.with_suffix(".audit.json").write_text(json.dumps(observations, indent=2))
        assert observations[-1]["state"] == "absent"


@pytest.mark.parametrize(
    ("stdout", "stderr", "returncode", "error"),
    [
        (b"123 123 Z\n123 123 Z\n", b"", 0, "duplicate process ID"),
        (b"123 123 Z\n0 123 S\n", b"", 0, "invalid process ID"),
        (b"123 123 Z\n-2 123 S\n", b"", 0, "invalid process ID"),
        (b"+123 123 Z\n", b"", 0, "invalid process ID"),
        (b"123 123 Z\n124 999 S\n", b"", 0, "foreign process group"),
        (b"124 123 Z\n", b"", 0, "leader missing"),
        (b"123 123 Z", b"", 0, "incomplete process table"),
        (b"123 123 Z\n124 123", b"", 0, "incomplete process table"),
        (b"123 123 Z\n\n", b"", 0, "process table row"),
        (b"123 123 Z extra\n", b"", 0, "process table row"),
        (b"123 123 Z\n124 123 ?\n", b"", 0, "unsupported process state"),
        (b"123 123 Zgarbage\n", b"", 0, "unsupported process state"),
        (b"123 123 \xff\n", b"", 0, "unsupported process state"),
        (b"123 123 Z\n", b"warning", 0, "clean process table"),
        (b"123 123 Z\n", b"", 1, "clean process table"),
        (b"", b"usage", 64, "clean process table"),
        (b"123 123 Z\n" + b" " * (256 * 1024), b"", 0, "output limit"),
        (b"123 123 Z\n", b"x" * (256 * 1024), 0, "output limit"),
        (
            b"".join(f"{member} 123 Z\n".encode() for member in range(123, 4220)),
            b"",
            0,
            "row limit",
        ),
    ],
)
def test_group_snapshot_rejects_invalid_table_before_probe(
    monkeypatch, stdout, stderr, returncode, error
):
    import controller

    probes = []
    monkeypatch.setattr(
        controller.subprocess,
        "run",
        lambda *args, **kwargs: subprocess.CompletedProcess(args[0], returncode, stdout, stderr),
    )
    monkeypatch.setattr(controller.os, "killpg", lambda *args: probes.append(args))
    observation = controller._group_snapshot(123, time.monotonic() + 1)
    assert observation["state"] == "unknown"
    assert observation["members"] == []
    assert error in observation["error"]
    assert observation["stdout"] == stdout
    assert observation["stderr"] == stderr
    assert observation["returncode"] == returncode
    assert observation["probe_outcome"] == "not_attempted"
    assert probes == []


@pytest.mark.parametrize(
    ("stdout", "expected_state", "count"),
    [
        (b"123 123 Z\n", "zombie_only", 1),
        (b"  123 123 Zs\n124 123 R+\n", "live", 2),
        (b"123 123 Z\n" + b" " * (256 * 1024 - 20) + b"124 123 Z\n", "zombie_only", 2),
        (
            b"".join(f"{member} 123 Z\n".encode() for member in range(123, 4219)),
            "zombie_only",
            4096,
        ),
        (b"123 123 U<ANELNSsVWX+>\n", "live", 1),
    ],
)
def test_group_snapshot_accepts_valid_tables_and_limits(monkeypatch, stdout, expected_state, count):
    import controller

    probes = []
    monkeypatch.setattr(
        controller.subprocess,
        "run",
        lambda *args, **kwargs: subprocess.CompletedProcess(args[0], 0, stdout, b""),
    )
    monkeypatch.setattr(controller.os, "killpg", lambda *args: probes.append(args))
    observation = controller._group_snapshot(123, time.monotonic() + 1)
    assert observation["error"] is None
    assert observation["state"] == expected_state
    assert len(observation["members"]) == count
    assert observation["members"][0] == {
        "pid": 123,
        "pgid": 123,
        "stat": stdout.split()[2].decode(),
    }
    assert probes == [(123, 0)]


@pytest.mark.parametrize("late_during", ["helper", "probe"])
def test_group_snapshot_rejects_late_observation(monkeypatch, late_during):
    import controller

    now = [10.0]
    probes = []

    def helper(*args, **kwargs):
        if late_during == "helper":
            now[0] = 11.0
        return subprocess.CompletedProcess(args[0], 0, b"123 123 Z\n", b"")

    def probe(*args):
        probes.append(args)
        now[0] = 11.0

    monkeypatch.setattr(controller.time, "monotonic", lambda: now[0])
    monkeypatch.setattr(controller.subprocess, "run", helper)
    monkeypatch.setattr(controller.os, "killpg", probe)
    observation = controller._group_snapshot(123, 11.0)
    assert observation["state"] == "unknown"
    assert "deadline" in observation["error"]
    assert observation["stdout"] == b"123 123 Z\n"
    assert probes == ([] if late_during == "helper" else [(123, 0)])
