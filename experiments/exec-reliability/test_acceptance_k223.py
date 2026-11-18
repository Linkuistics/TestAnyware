"""Controls for the acceptance trial plan and checker (leaf acceptance-plan-k223).

No test boots a guest. The plan tests build the frozen plans with placeholder
tools and validate them with the controller's own validation. The checker tests
build one clean clone directory for each OS, in the shape that the controller
and the Host CLI write. Each defect control then breaks one property and expects
the checker to reject the clone. The helper tests run the cancellation helper
against a fake Host CLI.
"""

from __future__ import annotations

import base64
import hashlib
import json
import os
import subprocess
import sys
import time
from collections.abc import Callable
from dataclasses import asdict
from pathlib import Path
from typing import Any, cast

import pytest

import acceptance_k223 as trial
import acceptance_k223_check as checker
import controller

Json = dict[str, Any]
OSES = ("macos", "linux")
PID = 4242


def _manifest(tmp_path: Path, **bounds: object) -> Path:
    path = tmp_path / "manifest.json"
    data = {
        "cli": "/opt/private/testanyware",
        "tart": "/opt/private/tart",
        "goldens": {"macos": "k224-golden-macos", "linux": "k224-golden-linux"},
        "bounds": bounds or {"boot": 420, "slot": 60},
        "sha256": {},
    }
    path.write_text(json.dumps(data))
    return path


def _plan(tmp_path: Path, os_name: str) -> controller.Plan:
    manifest = trial.read_manifest(_manifest(tmp_path))
    return trial.plan(manifest, os_name, 1, sys.executable, tmp_path / "downloads")


# --- the plan ---------------------------------------------------------------


@pytest.mark.parametrize("os_name", OSES)
def test_plan_passes_controller_validation_in_root_order(tmp_path: Path, os_name: str) -> None:
    plan = _plan(tmp_path, os_name)
    controller._validate(plan, tmp_path / "evidence")  # pyright: ignore[reportPrivateUsage]
    names = [call.name for call in plan.calls]
    assert ["acquire", *names, "cleanup", "dispose", "inspect"] == checker.expected_shape(os_name)
    order = [
        "baseline",
        "install",
        "intact",
        "ordinary-01",
        "exit-23",
        "timeout",
        "after-timeout-01",
        "cancel",
        "after-cancel-01",
        "interrupt",
        "capture",
        "download-missing",
    ]
    assert [names.index(name) for name in order] == sorted(names.index(name) for name in order)
    assert sum(name.startswith("ordinary-") for name in names) == 10
    assert sum(name.startswith("after-timeout-") for name in names) == 10
    assert sum(name.startswith("after-cancel-") for name in names) == 10


@pytest.mark.parametrize("os_name", OSES)
def test_every_host_cli_call_uses_json_mode_and_its_own_exec_id(
    tmp_path: Path, os_name: str
) -> None:
    plan = _plan(tmp_path, os_name)
    resource = plan.resources[0]
    calls = [*plan.calls, resource.cleanup]
    ids: list[str] = []
    for call in calls:
        if call.argv[0] == "/opt/private/testanyware":
            assert "--json" in call.argv, call.name
        if call.argv[1:3] == ("file", "exec"):
            exec_id = call.argv[call.argv.index("--exec-id") + 1]
            assert checker.EXEC_ID.fullmatch(exec_id)
            assert exec_id == trial.exec_id(plan.run_id, call.name)
            ids.append(exec_id)
    assert len(ids) == len(set(ids))


def test_interruption_and_cancellation_stay_separate(tmp_path: Path) -> None:
    plan = _plan(tmp_path, "linux")
    calls = {call.name: call for call in plan.calls}
    interrupt, cancel = calls["interrupt"], calls["cancel"]
    assert interrupt.interrupt_after == trial.CANCEL_AFTER == 2.0
    assert (interrupt.expected_exit, interrupt.expected_reason) == (-15, "interrupted")
    assert cancel.interrupt_after is None and cancel.argv[2].endswith("acceptance_k223_cancel.py")
    interrupt_id = interrupt.argv[interrupt.argv.index("--exec-id") + 1]
    cancel_id = cancel.argv[cancel.argv.index("--exec-id") + 1]
    fallback = calls["interrupt-fallback"].argv
    assert interrupt_id != cancel_id
    assert fallback[1:3] == ("file", "cancel")
    assert fallback[fallback.index("--exec-id") + 1] == interrupt_id
    assert fallback[fallback.index("--wait") + 1] == "5"
    assert trial.child_payload("interrupt") in interrupt.argv
    assert trial.child_payload("cancel") in cancel.argv


def test_bounds_are_the_root_and_spec_ceilings(tmp_path: Path) -> None:
    plan = _plan(tmp_path, "macos")
    resource = plan.resources[0]
    reserve = plan.term_grace + plan.kill_wait
    assert (plan.term_grace, plan.kill_wait) == (2.0, 2.0)
    assert resource.dispose.budget + reserve <= trial.DISPOSAL == 45.0
    assert resource.cleanup.budget + reserve <= trial.GUEST_CLEANUP == 30.0
    assert resource.acquire.budget == 480.0
    for call in plan.calls:
        if call.argv[1:3] == ("file", "exec"):
            guest = int(call.argv[call.argv.index("--timeout") + 1])
            assert call.budget <= guest + trial.HOST_MARGIN, call.name
        if call.argv[1:3] in (("file", "upload"), ("file", "download")):
            assert call.budget == trial.TRANSFER
        if call.argv[1:3] == ("agent", "health"):
            assert call.budget == trial.OBSERVATION
    calls = {call.name: call for call in plan.calls}
    assert calls["baseline"].budget == trial.INVENTORY == 60.0
    assert calls["timeout"].budget == 22.0
    assert calls["ordinary-02"].budget == trial.OBSERVATION == 30.0
    bounds = trial.bounds(trial.read_manifest(_manifest(tmp_path)))
    assert bounds["boot"] == 420 and bounds["slot"] == 60 and bounds["acquire"] == 480


@pytest.mark.parametrize(
    "bounds",
    [{"boot": 420}, {"boot": 0, "slot": 60}, {"boot": True, "slot": 60}, {"boot": "4", "slot": 6}],
)
def test_manifest_without_valid_boot_and_slot_bounds_is_refused(
    tmp_path: Path, bounds: Json
) -> None:
    with pytest.raises(ValueError, match="manifest bound"):
        trial.read_manifest(_manifest(tmp_path, **bounds))


def test_changed_frozen_digest_is_refused(tmp_path: Path) -> None:
    subject = tmp_path / "subject"
    subject.write_bytes(b"frozen")
    digest = hashlib.sha256(b"frozen").hexdigest()
    assert trial.digest_mismatches({str(subject): digest}) == []
    subject.write_bytes(b"frozen!")
    assert trial.digest_mismatches({str(subject): digest}) != []
    assert trial.fixture_mismatches("macos") == trial.fixture_mismatches("linux") == []


def test_reset_is_demonstrated_only_by_the_next_baseline() -> None:
    clones = [
        {"clone": "macos-01", "passed": True, "baseline_ok": True},
        {"clone": "macos-02", "passed": False, "baseline_ok": True},
    ]
    assert trial.reset_record(clones) == {
        "macos-01": "demonstrated by macos-02",
        "macos-02": "undemonstrated: the next clone was not reached",
    }
    five = [{"clone": f"linux-0{n}", "passed": True, "baseline_ok": True} for n in range(1, 6)]
    assert trial.reset_record(five)["linux-05"] == "undemonstrated: no later clone of this OS"


# --- record validity --------------------------------------------------------


def _stream(data: bytes) -> Json:
    return {
        "base64": base64.b64encode(data).decode(),
        "bytes": len(data),
        "file_bytes": len(data),
        "sha256": hashlib.sha256(data).hexdigest(),
        "capture": "complete",
        "reason": None,
    }


def _termination(leader_first: bool = True) -> Json:
    if leader_first:
        return {
            "term_sent_after_ms": 0,
            "kill_sent_after_ms": 3,
            "leader_status": {"kind": "signalled", "signal": 15},
            "leader_exit_after_ms": 2,
            "group_empty": "yes",
            "group_empty_after_ms": 4,
            "within_bounds": True,
        }
    return {
        "term_sent_after_ms": 0,
        "kill_sent_after_ms": 1900,
        "leader_status": {"kind": "signalled", "signal": 9},
        "leader_exit_after_ms": 1904,
        "group_empty": "yes",
        "group_empty_after_ms": 1906,
        "within_bounds": True,
    }


def _record(
    platform: str,
    exec_id: str,
    outcome: Json,
    out: bytes = b"",
    err: bytes = b"",
    *,
    termination: Json | None = None,
    duration: int = 41,
) -> Json:
    """One valid typed record in the snake_case shape of the Host CLI."""
    terminated = outcome["kind"] in ("timed_out", "cancelled")
    legacy = checker.projection(platform, outcome, out, err)
    return {
        "exit_code": legacy[0],
        "stdout": legacy[1],
        "stderr": legacy[2],
        "timed_out": legacy[3],
        "exec_id": exec_id,
        "legacy_projection": platform,
        "outcome": outcome,
        "evidence": {"stdout": _stream(out), "stderr": _stream(err)},
        "lifecycle": {
            "leader_pid": 812,
            "group_id": 812,
            "leader_start_time": "Wed Nov 18 03:40:00 2026",
            "spawn_ms": 2,
            "duration_ms": duration,
            "owner_lateness_ms": 3 if terminated else 0,
            "finalised_after_ms": 1910 if terminated else 3,
        },
        "termination": (termination or _termination()) if terminated else None,
        "remaining_group": None if terminated else "none",
    }


def _envelope(argv: list[str], record: Json) -> Json:
    return {"schema_version": "1.0", "ok": True, "command": argv[-1], "record": "agent", **record}


TIMED_OUT = {"kind": "timed_out", "timeout_seconds": 2}
CANCELLED = {"kind": "cancelled", "before_start": False}
PARTIAL = (trial.PARTIAL_STDOUT, trial.PARTIAL_STDERR)


def test_legacy_projection_of_each_platform() -> None:
    assert checker.projection("macos", TIMED_OUT, *PARTIAL) == (
        -1,
        "partial-out",
        "partial-err\n[Process timed out after 2s]",
        True,
    )
    assert checker.projection("macos", CANCELLED, b"", b"") == (-1, "", "Process cancelled", False)
    assert checker.projection("linux", TIMED_OUT, *PARTIAL) == (-1, "", "Process timed out", False)
    exited = {"kind": "exited", "exit_code": 0}
    assert checker.projection("linux", exited, b"a\r\nb \n", b"") == (0, "a\nb", "", False)
    assert checker.projection("linux", exited, b"\xff", b"") == (-1, "", None, False)
    assert checker.projection("macos", exited, b"\xff", b"x\x1c") == (0, "", "x\x1c", False)
    signalled = {"kind": "signalled", "signal": 9, "signal_name": "SIGKILL"}
    assert checker.projection("linux", signalled, b"", b"")[0] == -9
    assert checker.projection("macos", signalled, b"", b"")[0] == 9


@pytest.mark.parametrize("platform", OSES)
@pytest.mark.parametrize("leader_first", [True, False])
def test_both_leader_orders_are_valid_and_within_bounds(platform: str, leader_first: bool) -> None:
    record = _record(platform, "id-1", TIMED_OUT, *PARTIAL, termination=_termination(leader_first))
    assert checker.record_failures(record, "id-1", envelope=False) == []
    assert checker.bound_failures(record) == []


def test_unavailable_leader_status_is_valid_but_fails_acceptance() -> None:
    termination = {
        "term_sent_after_ms": 0,
        "kill_sent_after_ms": 1900,
        "leader_status": {"kind": "unavailable"},
        "leader_exit_after_ms": None,
        "group_empty": "no",
        "group_empty_after_ms": None,
        "within_bounds": False,
    }
    record = _record("macos", "id-1", TIMED_OUT, *PARTIAL, termination=termination)
    record["lifecycle"]["finalised_after_ms"] = 3310
    assert checker.record_failures(record, "id-1", envelope=False) == []
    assert any("within_bounds" in failure for failure in checker.bound_failures(record))


@pytest.mark.parametrize(
    "edit",
    [
        lambda r: r.update(exec_id="another"),
        lambda r: r.update(legacy_projection="windows"),
        lambda r: r.update(stdout="partial-out\n"),
        lambda r: r.update(timed_out=False),
        lambda r: r["evidence"]["stdout"].update(sha256="0" * 64),
        lambda r: r["evidence"]["stdout"].update(bytes=3),
        lambda r: r["evidence"]["stdout"].update(capture="incomplete", reason="deadline"),
        lambda r: r.update(remaining_group="none"),
        lambda r: r.update(termination=None),
        lambda r: r["lifecycle"].update(spawn_ms=-1),
        lambda r: r["termination"].update(term_sent_after_ms=4),
        lambda r: r["termination"].update(leader_exit_after_ms=None),
        lambda r: r["termination"].update(leader_exit_after_ms=1911),
        lambda r: r["termination"].update(group_empty_after_ms=None),
        lambda r: r["termination"].update(within_bounds=False),
        lambda r: r.pop("outcome"),
        lambda r: r.update(outcome={"kind": "unknown", "reason": "legacy_agent"}),
    ],
)
def test_each_broken_validity_rule_is_rejected(edit: Callable[[Json], object]) -> None:
    record = _record("macos", "id-1", TIMED_OUT, *PARTIAL)
    assert checker.record_failures(record, "id-1", envelope=False) == []
    edit(record)
    assert checker.record_failures(record, "id-1", envelope=False) != []


def test_legacy_envelope_is_not_an_agent_record() -> None:
    record = _record("linux", "id-1", {"kind": "exited", "exit_code": 0})
    assert checker.record_failures(record | {"record": "agent"}, "id-1", envelope=True) == []
    assert checker.record_failures(record | {"record": "legacy"}, "id-1", envelope=True) != []


@pytest.mark.parametrize(
    ("stdout", "state"),
    [
        (b"child pid=4242\nchild ps-exit=1\n", "absent"),
        (
            b"child pid=4242\n 4242 1 4240 S Wed Nov 18 03:40:00 2026 sleep 120\nchild ps-exit=0\n",
            "present",
        ),
        (
            b"child pid=4242\n 4242 1 4240 Z Wed Nov 18 03:40:00 2026 [sleep]\nchild ps-exit=0\n",
            "zombie",
        ),
        (
            b"child pid=4242\n 4242 1 4240 S Wed Nov 18 03:40:00 2026 /usr/bin/vim\n"
            b"child ps-exit=0\n",
            "other",
        ),
        (b"child pid-file=unreadable\n", "unknown"),
        (b"child pid=4242\nchild ps-exit=2\n", "unknown"),
        (None, "unknown"),
    ],
)
def test_child_states(stdout: bytes | None, state: str) -> None:
    assert checker.child_state(stdout) == state


# --- the clean clone ----------------------------------------------------------


class Builder:
    """Write one clean clone directory in the shape of the controller and the Host CLI."""

    def __init__(self, tmp_path: Path, os_name: str) -> None:
        self.os_name = os_name
        self.directory = tmp_path / f"{os_name}-01"
        self.downloads = self.directory / "downloads"
        self.downloads.mkdir(parents=True)
        manifest = trial.read_manifest(_manifest(tmp_path))
        self.plan = trial.plan(manifest, os_name, 1, sys.executable, self.downloads)
        self.run = self.directory / self.plan.run_id
        self.run.mkdir()
        resource = self.plan.resources[0]
        self.calls = {
            call.name: call
            for call in (
                *self.plan.calls,
                resource.acquire,
                resource.cleanup,
                resource.dispose,
                resource.inspect,
            )
        }
        self.receipts: dict[str, Json] = {}
        self.stdout: dict[str, bytes] = {}
        self.inventory: Json = {
            "leaf": trial.LEAF,
            "os": os_name,
            "index": 1,
            "bounds": trial.bounds(manifest),
            "plan": json.loads(controller._json_bytes(asdict(self.plan))),  # pyright: ignore[reportPrivateUsage]
        }
        for name in checker.expected_shape(os_name):
            self.default(name)

    def id(self, name: str) -> str:
        return trial.exec_id(self.plan.run_id, name)

    def argv(self, name: str) -> list[str]:
        return list(self.calls[name].argv)

    def put(self, name: str, receipt: Json | bytes, elapsed: float = 0.2, **fields: Any) -> None:
        data = receipt if isinstance(receipt, bytes) else json.dumps(receipt).encode()
        self.stdout[name] = data
        self.receipts[name] = {
            "name": name,
            "argv": self.argv(name),
            "ok": True,
            "reason": "exit",
            "returncode": 0,
            "reaped": True,
            "elapsed": elapsed,
            "started_monotonic": 1000.0,
            "streams": {
                "stdout": {"completeness": "eof", "sha256": hashlib.sha256(data).hexdigest()},
                "stderr": {"completeness": "eof"},
            },
            **fields,
        }

    def exec(self, name: str, outcome: Json, out: bytes = b"", err: bytes = b"", **kw: Any) -> None:
        record = _record(self.os_name, self.id(name), outcome, out, err, **kw)
        self.put(name, _envelope(self.argv(name), record), elapsed=0.3)

    def cancel_reply(self, target: str) -> Json:
        record = _record(self.os_name, self.id(target), CANCELLED, *PARTIAL, duration=1900)
        return {
            "schema_version": "1.0",
            "ok": True,
            "exec_id": self.id(target),
            "state": "terminal",
            "record": record,
        }

    def default(self, name: str) -> None:
        argv = self.argv(name)
        exited0 = {"kind": "exited", "exit_code": 0}
        if name == "acquire":
            self.put(name, b'{"ok": true}\n', elapsed=120.0)
        elif name == "dispose":
            self.put(name, b"disposed vm stopped\n", elapsed=12.0)
        elif name == "inspect":
            self.put(name, b"absent vm\n", elapsed=4.0, stdout_matches=True)
        elif argv[1:3] == ["agent", "health"]:
            receipt = {"schema_version": "1.0", "ok": True, "reachable": True}
            self.put(name, receipt | {"platform": self.os_name})
        elif name == "intact":
            self.exec(name, exited0, trial.ORACLE["stdout.bin"], trial.ORACLE["stderr.bin"])
        elif name == "baseline":
            self.exec(name, exited0, b"AGENT_AND_BASELINE_DETAILS\nBASELINE_ABSENT\n")
        elif name == "exit-23":
            self.exec(name, {"kind": "exited", "exit_code": 23})
        elif name == "timeout":
            self.exec(name, TIMED_OUT, *PARTIAL, duration=2003)
            self.receipts[name]["elapsed"] = 4.1
        elif name == "cleanup":
            self.exec(name, exited0, b"gone pid=1\ncleanup confirmed\n")
        elif argv[-1] == trial.ORDINARY:
            self.exec(name, exited0, b"ordinary-ok")
        elif any(argv[-1] == trial.child_check(case) for case in trial.CHILD_CASES):
            self.exec(name, exited0, f"child pid={PID}\nchild ps-exit=1\n".encode())
        elif name == "interrupt":
            self.put(
                name,
                b"",
                elapsed=2.4,
                reason="interrupted",
                returncode=-15,
                signals=[
                    {"signal": "TERM", "scope": "pid", "outcome": "sent", "monotonic": 1002.01}
                ],
                leader_exit_monotonic=1002.3,
            )
        elif name == "interrupt-fallback":
            self.put(name, self.cancel_reply("interrupt"), elapsed=0.4)
        elif name == "cancel":
            self.put(name, self.helper(), elapsed=3.0)
        elif argv[1:3] == ["file", "upload"]:
            local, remote = Path(argv[5]), argv[6]
            self.put(
                name,
                {
                    "schema_version": "1.0",
                    "ok": True,
                    "remote": remote,
                    "local": str(local),
                    "bytes": len(local.read_bytes()),
                },
            )
        elif name == "download-missing":
            self.put(
                name,
                {"schema_version": "1.0", "ok": False, "code": "DOWNLOAD_FAILED"},
                returncode=1,
            )
        elif argv[1:3] == ["file", "download"]:
            remote, local = argv[5], Path(argv[6])
            data = trial.ORACLE.get(local.name, trial.PARTIAL)
            local.write_bytes(data)
            self.put(
                name,
                {
                    "schema_version": "1.0",
                    "ok": True,
                    "remote": remote,
                    "local": str(local),
                    "bytes": len(data),
                },
                elapsed=0.5,
            )
        else:
            self.exec(name, exited0, b"ok\n")

    def helper(self, **changes: Any) -> Json:
        exec_argv = trial._exec(  # pyright: ignore[reportPrivateUsage]
            trial.Clone(
                self.os_name,
                1,
                self.plan.run_id,
                self.argv("cancel")[6],
                self.argv("cancel")[4],
                sys.executable,
                self.downloads,
            ),
            "cancel",
            trial.CHILD_GUEST,
            trial.child_payload("cancel"),
        )
        record = _record(self.os_name, self.id("cancel"), CANCELLED, *PARTIAL, duration=1900)
        envelope = _envelope(list(exec_argv), record)
        reply = self.cancel_reply("cancel")
        health = {"schema_version": "1.0", "ok": True, "reachable": True}

        def child(data: Json, started: float, ended: float) -> Json:
            encoded = base64.b64encode(json.dumps(data).encode()).decode()
            return {
                "returncode": 0,
                "reaped": True,
                "signals": [],
                "stdout": encoded,
                "stderr": "",
                "started_after": started,
                "ended_after": ended,
            }

        helper = {
            "schema": "acceptance-k223-cancel/1",
            "exec_id": self.id("cancel"),
            "health": child(health, 1.0, 1.1),
            "exec_running_at_cancel": True,
            "cancel": child(reply, 2.0, 2.2),
            "exec": child(envelope, 0.0, 2.3) | {"argv": list(exec_argv)},
            "ok": True,
        }
        for key, value in changes.items():
            helper[key] = value
        return helper

    def write(self) -> Path:
        names = checker.expected_shape(self.os_name)
        result = {
            "run_id": self.plan.run_id,
            "ok": True,
            "inspection_failure": None,
            "calls": [self.receipts[name] for name in names if name in self.receipts],
            "unreached": [name for name in names if name not in self.receipts],
            "subjects_before": {"/x": "1"},
            "subjects_after": {"/x": "1"},
            "storage": {
                "frames": {"failed": 0, "dropped": 0, "abandoned": 0},
                "first_failure": None,
            },
        }
        (self.run / "result.json").write_text(json.dumps(result))
        (self.run / "plan.json").write_text(json.dumps(self.inventory["plan"]))
        close = {"state": "orderly", "refused": 0, "failed": 0}
        (self.run / "storage-close.json").write_text(json.dumps(close))
        for name, data in self.stdout.items():
            (self.run / f"{name}.stdout").write_bytes(data)
        (self.directory / "inventory.json").write_text(json.dumps(self.inventory))
        summary = {
            "verdict": True,
            "run": str(self.run),
            "ledger": [{"kind": "vm", "state": "disposed"}],
            "residuals": [],
        }
        (self.directory / "summary.json").write_text(json.dumps(summary))
        return self.directory


def _failing_cases(verdict: Json) -> dict[str, list[str]]:
    cases = cast(dict[str, Json], verdict["cases"])
    return {name: case["failures"] for name, case in cases.items() if not case["ok"]}


@pytest.mark.parametrize("os_name", OSES)
def test_clean_clone_passes_and_each_corruption_control_is_rejected(
    tmp_path: Path, os_name: str
) -> None:
    directory = Builder(tmp_path, os_name).write()
    verdict = checker.check_clone(directory)
    assert _failing_cases(verdict) == {} and verdict["failures"] == []
    controls = verdict["corruption_controls"]
    assert controls["ok"] is True and controls["originals_unchanged"] is True
    assert set(controls["controls"]) == set(checker.CONTROLS)
    assert all(control["rejected"] is True for control in controls["controls"].values())
    assert verdict["ok"] is True and verdict["baseline_ok"] is True


def test_appended_lf_is_caught_by_the_byte_oracle_not_by_validity(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "macos")
    damaged = json.loads(checker._appended_lf(builder.stdout["intact"]))  # pyright: ignore[reportPrivateUsage]
    assert checker.record_failures(damaged, builder.id("intact"), envelope=True) == []
    failures = checker.exec_failures(
        damaged,
        builder.argv("intact"),
        outcome={"kind": "exited", "exit_code": 0},
        stdout=trial.ORACLE["stdout.bin"],
    )
    assert failures and all("stdout bytes" in failure for failure in failures)


def test_contradictory_order_has_a_form_when_kill_is_at_zero(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "linux")
    envelope = json.loads(builder.stdout["timeout"])
    envelope["termination"].update(
        kill_sent_after_ms=0, leader_exit_after_ms=0, group_empty_after_ms=1
    )
    assert checker.record_failures(envelope, builder.id("timeout"), envelope=True) == []
    damaged = json.loads(checker._contradictory_order(json.dumps(envelope).encode()))  # pyright: ignore[reportPrivateUsage]
    assert checker.record_failures(damaged, builder.id("timeout"), envelope=True) != []


def _exec_edit(name: str, edit: Callable[[Json], object]) -> Callable[[Builder], None]:
    def apply(builder: Builder) -> None:
        envelope = json.loads(builder.stdout[name])
        edit(envelope)
        builder.put(name, envelope, elapsed=builder.receipts[name]["elapsed"])

    return apply


def _out(data: bytes) -> Callable[[Json], None]:
    return lambda envelope: envelope["evidence"].update(stdout=_stream(data))


def _child(row: str) -> Callable[[Builder], None]:
    data = f"child pid={PID}\n {PID} 1 {PID} {row}\nchild ps-exit=0\n".encode()
    return _exec_edit("after-timeout-04", _out(data))


def _receipt(name: str, **fields: Any) -> Callable[[Builder], None]:
    return lambda builder: builder.receipts[name].update(fields)


def _helper(**changes: Any) -> Callable[[Builder], None]:
    def apply(builder: Builder) -> None:
        helper = builder.helper()
        for key, value in changes.items():
            if isinstance(value, dict):
                helper[key] = helper[key] | value
            else:
                helper[key] = value
        builder.put("cancel", helper, elapsed=3.0)

    return apply


def _drop(name: str) -> Callable[[Builder], None]:
    def apply(builder: Builder) -> None:
        del builder.receipts[name]
        del builder.stdout[name]

    return apply


def _download(local: str, data: bytes) -> Callable[[Builder], None]:
    return lambda builder: (builder.downloads / local).write_bytes(data)


DEFECTS: dict[str, tuple[Callable[[Builder], None], str]] = {
    "intact stdout with LF": (
        _exec_edit("intact", _out(trial.ORACLE["stdout.bin"] + b"\n")),
        "intact",
    ),
    "flipped artifact byte": (
        _download("artifact.bin", bytes.fromhex("00ff81410a")),
        "download-artifact-bin",
    ),
    "status not 0": (_download("status.txt", b"1\n"), "download-status-txt"),
    "exit 23 reported as -1": (
        _exec_edit("exit-23", lambda e: e["outcome"].update(exit_code=0)),
        "exit-23",
    ),
    "timeout without termination": (
        _exec_edit("timeout", lambda e: e.pop("termination")),
        "timeout",
    ),
    "timeout duration too long": (
        _exec_edit("timeout", lambda e: e["lifecycle"].update(duration_ms=2600)),
        "timeout",
    ),
    "timeout group not empty": (
        _exec_edit(
            "timeout",
            lambda e: e["termination"].update(
                group_empty="unknown", group_empty_after_ms=None, within_bounds=False
            ),
        ),
        "timeout",
    ),
    "timeout capture incomplete": (
        _exec_edit(
            "timeout",
            lambda e: e["evidence"]["stdout"].update(
                file_bytes=99, capture="incomplete", reason="deadline"
            ),
        ),
        "timeout",
    ),
    "child present after timeout": (
        _child("S Wed Nov 18 03:40:00 2026 sleep 120"),
        "after-timeout-04",
    ),
    "child zombie after timeout": (
        _child("Z Wed Nov 18 03:40:00 2026 [sleep]"),
        "after-timeout-04",
    ),
    "ordinary bytes wrong": (_exec_edit("ordinary-02", _out(b"ordinary-ok\n")), "ordinary-02"),
    "slow observation": (_receipt("ordinary-03", elapsed=30.5), "ordinary-03"),
    "slow inventory": (_receipt("baseline", elapsed=60.5), "baseline"),
    "dirty baseline": (
        _exec_edit("baseline", _out(b"BASELINE_DIRTY\n")),
        "baseline",
    ),
    "spawn too slow": (
        _exec_edit("install", lambda e: e["lifecycle"].update(spawn_ms=1001)),
        "install",
    ),
    "unreachable health": (
        lambda b: b.put("ordinary-04", {"schema_version": "1.0", "ok": True, "reachable": False}),
        "ordinary-04",
    ),
    "interrupt needed SIGKILL": (
        _receipt(
            "interrupt",
            signals=[
                {"signal": "TERM", "scope": "pid", "outcome": "sent", "monotonic": 1002.01},
                {"signal": "KILL", "scope": "pid", "outcome": "sent", "monotonic": 1004.01},
            ],
        ),
        "interrupt",
    ),
    "interrupt death too slow": (_receipt("interrupt", leader_exit_monotonic=1004.2), "interrupt"),
    "interrupt before 2 s": (
        _receipt(
            "interrupt",
            signals=[{"signal": "TERM", "scope": "pid", "outcome": "sent", "monotonic": 1001.5}],
        ),
        "interrupt",
    ),
    "fallback not terminal": (
        lambda b: b.put(
            "interrupt-fallback",
            b.cancel_reply("interrupt") | {"state": "terminating", "record": None},
        ),
        "interrupt-fallback",
    ),
    "fallback reply too slow": (_receipt("interrupt-fallback", elapsed=5.2), "interrupt-fallback"),
    "cancellation too early": (_helper(cancel={"started_after": 1.9}), "cancel"),
    "health during command too slow": (
        _helper(health={"ended_after": 1.95, "started_after": 0.9}),
        "cancel",
    ),
    "health before the spawn": (_helper(health={"started_after": 0.1}), "cancel"),
    "exec ended before the cancellation": (_helper(exec_running_at_cancel=False), "cancel"),
    "cancellation reply too slow": (_helper(cancel={"ended_after": 7.1}), "cancel"),
    "missing download succeeded": (
        lambda b: b.put("download-missing", {"schema_version": "1.0", "ok": True}, returncode=0),
        "download-missing",
    ),
    "upload byte count wrong": (
        lambda b: b.put("upload-lesson", json.loads(b.stdout["upload-lesson"]) | {"bytes": 1}),
        "upload-lesson",
    ),
    "slow transfer": (
        _receipt("download-timeout-partial", elapsed=150.5),
        "download-timeout-partial",
    ),
    "cleanup unconfirmed": (_exec_edit("cleanup", _out(b"cleanup uncertain\n")), "cleanup"),
    "slow disposal": (_receipt("dispose", elapsed=45.5), "dispose"),
    "slow acquisition": (_receipt("acquire", elapsed=481.0), "acquire"),
    "stored stdout differs from capture": (
        lambda b: b.receipts["identity"]["streams"]["stdout"].update(sha256="0" * 64),
        "identity",
    ),
    "unreached case": (_drop("capture"), "capture"),
}


@pytest.mark.parametrize("defect", sorted(DEFECTS))
def test_each_defect_fails_its_case_and_the_clone(tmp_path: Path, defect: str) -> None:
    builder = Builder(tmp_path, "macos")
    edit, case = DEFECTS[defect]
    edit(builder)
    verdict = checker.check_clone(builder.write())
    assert verdict["ok"] is False
    assert case in _failing_cases(verdict), _failing_cases(verdict)


def test_failed_original_disables_the_corruption_controls(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "linux")
    _exec_edit("timeout", lambda e: e.pop("outcome"))(builder)
    verdict = checker.check_clone(builder.write())
    assert verdict["corruption_controls"]["ok"] is False


@pytest.mark.parametrize(
    "edit",
    [
        lambda b: b.inventory["plan"]["calls"].pop(),
        lambda b: b.inventory["plan"]["calls"][3]["argv"].append("--extra"),
    ],
)
def test_changed_frozen_plan_fails_the_clone(
    tmp_path: Path, edit: Callable[[Builder], object]
) -> None:
    builder = Builder(tmp_path, "macos")
    edit(builder)
    directory = builder.write()
    plan = json.loads((builder.run / "plan.json").read_bytes())
    original = json.loads(controller._json_bytes(asdict(builder.plan)))  # pyright: ignore[reportPrivateUsage]
    assert plan != original
    assert checker.check_clone(directory)["ok"] is False


def test_undisposed_clone_or_residual_fails(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "linux")
    directory = builder.write()
    summary = json.loads((directory / "summary.json").read_bytes())
    summary["ledger"][0]["state"] = "uncertain"
    (directory / "summary.json").write_text(json.dumps(summary))
    assert checker.check_clone(directory)["ok"] is False
    summary["ledger"][0]["state"] = "disposed"
    summary["residuals"] = [{"kind": "group", "pgid": 1}]
    (directory / "summary.json").write_text(json.dumps(summary))
    assert checker.check_clone(directory)["ok"] is False


def test_checker_main_exits_by_the_verdict(tmp_path: Path) -> None:
    directory = Builder(tmp_path, "macos").write()
    script = Path(checker.__file__)
    clean = subprocess.run(
        [sys.executable, str(script), str(directory)], capture_output=True, timeout=60
    )
    assert clean.returncode == 0 and json.loads(clean.stdout)["ok"] is True
    (directory / "downloads" / "artifact.bin").write_bytes(b"\x00")
    damaged = subprocess.run(
        [sys.executable, str(script), str(directory)], capture_output=True, timeout=60
    )
    assert damaged.returncode == 1


# --- the cancellation helper -----------------------------------------------

FAKE_CLI = """#!{python}
import json, os, sys, time
args = sys.argv[1:]
marker = os.environ["FAKE_MARKER"]
mode = os.environ.get("FAKE_MODE", "ok")
if args[:2] == ["agent", "health"]:
    print(json.dumps({{"schema_version": "1.0", "ok": True, "reachable": True}}))
elif args[:2] == ["file", "cancel"]:
    open(marker, "w").close()
    print(json.dumps({{"schema_version": "1.0", "ok": True, "state": "terminal"}}))
elif args[:2] == ["file", "exec"]:
    while not os.path.exists(marker) or mode == "stall":
        time.sleep(0.01)
    print(json.dumps({{"schema_version": "1.0", "ok": True, "outcome": {{"kind": "cancelled"}}}}))
"""


def _fake_cli(tmp_path: Path) -> Path:
    path = tmp_path / "fake-cli"
    path.write_text(FAKE_CLI.format(python=sys.executable))
    path.chmod(0o755)
    return path


def _helper_run(tmp_path: Path, mode: str) -> tuple[subprocess.CompletedProcess[bytes], float]:
    argv = [
        sys.executable,
        "-I",
        str(Path(trial.__file__).with_name("acceptance_k223_cancel.py")),
        "--cli",
        str(_fake_cli(tmp_path)),
        "--vm",
        "vm",
        "--exec-id",
        "k223-cancel-test",
        "--guest-timeout",
        "1",
        "--host-margin",
        "0.5",
        "--health-after",
        "0.2",
        "--cancel-after",
        "0.4",
        "--cancel-wait",
        "5",
        "--observation",
        "5",
        "--command",
        "sleep 120",
    ]
    environment = {**os.environ, "FAKE_MARKER": str(tmp_path / "cancelled"), "FAKE_MODE": mode}
    started = time.monotonic()
    completed = subprocess.run(argv, capture_output=True, env=environment, timeout=30)
    return completed, time.monotonic() - started


def test_helper_runs_health_then_cancel_while_the_command_runs(tmp_path: Path) -> None:
    completed, _ = _helper_run(tmp_path, "ok")
    assert completed.returncode == 0, completed.stderr
    record = json.loads(completed.stdout)
    assert record["ok"] is True and record["exec_running_at_cancel"] is True
    health, cancel, run = record["health"], record["cancel"], record["exec"]
    assert 0.2 <= health["started_after"] <= health["ended_after"] <= cancel["started_after"]
    assert 0.4 <= cancel["started_after"] <= cancel["ended_after"] <= run["ended_after"]
    reply = json.loads(base64.b64decode(cancel["stdout"]))
    assert reply["state"] == "terminal"
    assert run["argv"][run["argv"].index("--exec-id") + 1] == "k223-cancel-test"
    assert json.loads(base64.b64decode(run["stdout"]))["outcome"] == {"kind": "cancelled"}


def test_helper_ends_a_stalled_command_within_its_bounds(tmp_path: Path) -> None:
    completed, elapsed = _helper_run(tmp_path, "stall")
    record = json.loads(completed.stdout)
    run = record["exec"]
    assert run["deadline_passed"] is True and run["reaped"] is True
    assert [signal["signal"] for signal in run["signals"]] == ["SIGTERM"]
    assert run["returncode"] == -15
    # Guest 1 s plus host margin 0.5 s, then the 2 s grace at most.
    assert elapsed < 1.5 + trial.TERM_GRACE + 1.0


# --- the controls must be seen to fail ---------------------------------------


@pytest.mark.parametrize("control", sorted(checker.CONTROLS))
def test_a_control_whose_damage_changes_nothing_is_reported(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, control: str
) -> None:
    item = checker.CONTROLS[control][0]
    monkeypatch.setitem(checker.CONTROLS, control, (item, lambda data: data))
    verdict = checker.check_clone(Builder(tmp_path, "linux").write())
    controls = verdict["corruption_controls"]
    assert controls["controls"][control]["rejected"] is False
    assert controls["ok"] is False and verdict["ok"] is False


def test_a_checker_without_termination_rules_misses_the_order_control(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    monkeypatch.setattr(checker, "termination_failures", lambda termination, finalised: [])
    verdict = checker.check_clone(Builder(tmp_path, "macos").write())
    controls = verdict["corruption_controls"]["controls"]
    assert controls["contradictory-order"]["rejected"] is False
    assert verdict["ok"] is False


def test_a_checker_without_the_byte_oracle_misses_the_appended_lf(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    original = checker.exec_failures

    def without_bytes(envelope: Json | None, argv: list[str], **kw: Any) -> list[str]:
        return original(envelope, argv, outcome=kw["outcome"])

    monkeypatch.setattr(checker, "exec_failures", without_bytes)
    verdict = checker.check_clone(Builder(tmp_path, "macos").write())
    assert verdict["corruption_controls"]["controls"]["appended-lf"]["rejected"] is False
    assert verdict["ok"] is False


def test_an_original_changed_by_a_control_is_reported(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> None:
    builder = Builder(tmp_path, "linux")
    artifact = builder.downloads / "artifact.bin"

    def in_place(data: bytes) -> bytes:
        damaged = checker._flipped(data)  # pyright: ignore[reportPrivateUsage]
        artifact.write_bytes(damaged)
        return damaged

    monkeypatch.setitem(checker.CONTROLS, "flipped-byte", ("artifact", in_place))
    verdict = checker.check_clone(builder.write())
    controls = verdict["corruption_controls"]
    assert controls["originals_unchanged"] is False and controls["ok"] is False
