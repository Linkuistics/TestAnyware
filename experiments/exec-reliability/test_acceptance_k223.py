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


CLI = "/opt/private/testanyware"
TART = "/opt/private/tart"
APP_EXECUTABLE = "/opt/private/TestAnyware.app/Contents/MacOS/TestAnyware"
APP_BUILD = "0.0.0-k224"
FROZEN = {CLI: "a" * 64, TART: "b" * 64, APP_EXECUTABLE: "c" * 64}
MACOS_AGENT_SHA256 = "d" * 64
LINUX_LISTING = (
    f"{'e' * 64}  ./testanyware_agent/__main__.py\n{'f' * 64}  ./testanyware_agent/exec.py\n"
)
AGENTS = {
    "macos": MACOS_AGENT_SHA256,
    "linux": hashlib.sha256(LINUX_LISTING.encode()).hexdigest(),
}


def _manifest_data(**bounds: object) -> Json:
    return {
        "cli": CLI,
        "tart": TART,
        "goldens": {"macos": "k224-golden-macos", "linux": "k224-golden-linux"},
        "bounds": bounds or {"boot": 420, "slot": 60},
        "agents": dict(AGENTS),
        "app": {"build": APP_BUILD, "executable": APP_EXECUTABLE},
        "sha256": dict(FROZEN),
    }


def _manifest(tmp_path: Path, data: Json | None = None, **bounds: object) -> Path:
    path = tmp_path / "manifest.json"
    path.write_text(json.dumps(data or _manifest_data(**bounds)))
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
        "app-identity",
        "identity",
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
    calls = {call.name: call for call in plan.calls}
    for case in ("timeout", "cancel"):
        block = [calls[name] for name in names if name.startswith(f"after-{case}-")]
        checks = [call for call in block if call.argv[-1] == trial.child_check(case)]
        # The stricter reading of the root: ten operations, and child checks besides them.
        assert (len(block), len(checks)) == (14, 4)
        assert block[0] in checks and block[-1] in checks
        operations = [call for call in block if call not in checks]
        kinds = [call.argv[1:3] == ("agent", "health") for call in operations]
        assert kinds == [True, False] * 5


@pytest.mark.parametrize("os_name", OSES)
def test_every_host_cli_call_uses_json_mode_and_its_own_exec_id(
    tmp_path: Path, os_name: str
) -> None:
    plan = _plan(tmp_path, os_name)
    resource = plan.resources[0]
    calls = [*plan.calls, resource.cleanup]
    ids: list[str] = []
    for call in calls:
        if call.argv[0] == CLI:
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
    # The boot bound alone: `vm start` runs without `--wait` and never waits for a slot.
    assert resource.acquire.budget == 420.0
    assert "--wait" not in resource.acquire.argv
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
    assert bounds["boot"] == 420 and bounds["slot"] == 60 and bounds["acquire"] == 420


@pytest.mark.parametrize(
    "bounds",
    [{"boot": 420}, {"boot": 0, "slot": 60}, {"boot": True, "slot": 60}, {"boot": "4", "slot": 6}],
)
def test_manifest_without_valid_boot_and_slot_bounds_is_refused(
    tmp_path: Path, bounds: Json
) -> None:
    with pytest.raises(ValueError, match="manifest bound"):
        trial.read_manifest(_manifest(tmp_path, **bounds))


@pytest.mark.parametrize(
    ("edit", "message"),
    [
        (lambda d: d.pop("agents"), "agents"),
        (lambda d: d["agents"].pop("linux"), "agents"),
        (lambda d: d["agents"].update(macos="sha256:abc"), "agents"),
        (lambda d: d.pop("app"), "app build"),
        (lambda d: d["app"].update(build=""), "app build"),
        (lambda d: d["app"].update(executable="TestAnyware"), "app executable"),
        (lambda d: d["sha256"].pop(CLI), "does not pin"),
        (lambda d: d["sha256"].pop(TART), "does not pin"),
        (lambda d: d["sha256"].pop(APP_EXECUTABLE), "does not pin"),
        (lambda d: d["sha256"].update({CLI: "A" * 64}), "malformed"),
        (lambda d: d.update(sha256={}), "does not pin"),
    ],
)
def test_manifest_without_frozen_identities_is_refused(
    tmp_path: Path, edit: Callable[[Json], object], message: str
) -> None:
    data = _manifest_data()
    edit(data)
    with pytest.raises(ValueError, match=message):
        trial.read_manifest(_manifest(tmp_path, data))


def test_changed_frozen_digest_is_refused(tmp_path: Path) -> None:
    subject = tmp_path / "subject"
    subject.write_bytes(b"frozen")
    digest = hashlib.sha256(b"frozen").hexdigest()
    assert trial.digest_mismatches({str(subject): digest}) == []
    subject.write_bytes(b"frozen!")
    assert trial.digest_mismatches({str(subject): digest}) != []
    assert trial.fixture_mismatches("macos") == trial.fixture_mismatches("linux") == []


def test_a_subject_changed_between_clones_refuses_the_next_clone(tmp_path: Path) -> None:
    subject = tmp_path / "cli"
    subject.write_bytes(b"build 1")
    data = _manifest_data()
    data["sha256"] = {str(subject): hashlib.sha256(b"build 1").hexdigest()}
    data["cli"] = data["tart"] = data["app"]["executable"] = str(subject)
    path = _manifest(tmp_path, data)
    manifest = trial.read_manifest(path)
    frozen = hashlib.sha256(path.read_bytes()).hexdigest()
    assert trial.freeze_refusals(manifest, frozen) == []
    # A compatible build at the same path after clone 1 is a new candidate.
    subject.write_bytes(b"build 2")
    assert trial.freeze_refusals(manifest, frozen) != []
    subject.write_bytes(b"build 1")
    path.write_text(path.read_text() + " ")
    assert trial.freeze_refusals(manifest, frozen) != []


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
    refused = [
        {"clone": "macos-01", "passed": True, "baseline_ok": True},
        {"clone": "macos-02", "passed": False, "baseline_ok": False, "refused": ["cli"]},
    ]
    assert trial.reset_record(refused) == {
        "macos-01": "undemonstrated: macos-02 was refused before it ran",
        "macos-02": "undemonstrated: this clone was refused before it ran",
    }


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
    finalised: int | None = None,
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
            "finalised_after_ms": finalised if finalised is not None else 1910 if terminated else 3,
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
        # Finding R6 of k241: the payload of each leader status kind.
        lambda r: r["termination"].update(leader_status={"kind": "signalled"}),
        lambda r: r["termination"].update(leader_status={"kind": "signalled", "signal": "15"}),
        lambda r: r["termination"].update(leader_status={"kind": "signalled", "signal": True}),
        lambda r: r["termination"].update(leader_status={"kind": "exited"}),
        lambda r: r["termination"].update(leader_status={"kind": "exited", "signal": 15}),
        lambda r: r["termination"].update(
            leader_status={"kind": "signalled", "signal": 15, "exit_code": 0}
        ),
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


def test_exited_leader_status_is_valid() -> None:
    termination = _termination() | {"leader_status": {"kind": "exited", "exit_code": 0}}
    record = _record("linux", "id-1", TIMED_OUT, *PARTIAL, termination=termination)
    assert checker.record_failures(record, "id-1", envelope=False) == []


ZOMBIE_ROW = f" {PID} 1 4240 Z Wed Nov 18 03:40:00 2026 [sleep]\n"
SLEEP_ROW = f" {PID} 1 4240 S Wed Nov 18 03:40:00 2026 sleep 120\n"


def _polls(*rows: str | None) -> bytes:
    """A child poll output: each row is a listed snapshot, and None is an absent poll."""
    lines = [f"child pid={PID}\n"]
    for row in rows:
        lines.append("child ps-exit=1\n" if row is None else f"{row}child ps-exit=0\n")
    return "".join(lines).encode()


@pytest.mark.parametrize(
    ("stdout", "passes"),
    [
        # Finding R1 of k241: a zombie inside the termination interval, then absence.
        (_polls(ZOMBIE_ROW, None), True),
        (_polls(SLEEP_ROW, ZOMBIE_ROW, None), True),
        (_polls(None), True),
        (_polls(*([SLEEP_ROW] * trial.CHILD_POLLS)), False),
        (_polls(*([ZOMBIE_ROW] * trial.CHILD_POLLS)), False),
        (_polls(*([SLEEP_ROW] * trial.CHILD_POLLS), None), False),
        (_polls(f" {PID} 1 4240 S Wed Nov 18 03:40:00 2026 /usr/bin/vim\n", None), False),
        (_polls(None, None), False),
        (b"child pid-file=unreadable\n", False),
        (_polls(SLEEP_ROW)[:-1], False),
        (None, False),
    ],
)
def test_child_poll_requires_absence_by_its_last_poll(stdout: bytes | None, passes: bool) -> None:
    assert (checker.child_poll_failures(stdout) == []) is passes


def test_child_poll_covers_the_termination_interval() -> None:
    poll = trial.child_poll("interrupt")
    waits = (trial.CHILD_POLLS - 1) * trial.CHILD_POLL_WAIT
    assert waits >= trial.TERM_GRACE + trial.KILL_WAIT
    assert f'[ "$n" -ge {trial.CHILD_POLLS} ]' in poll and "sleep 0.1" in poll
    # The poll and its waits fit inside the guest budget of the observation.
    assert waits < trial.OBSERVE_GUEST


def test_child_poll_runs_in_a_posix_shell(tmp_path: Path) -> None:
    pid_file = tmp_path / "child.pid"
    child = subprocess.Popen(["sleep", "120"])
    pid_file.write_text(f"{child.pid}\n")
    poll = trial.child_poll("interrupt").replace(
        f"{trial.CHILD_ROOT}/interrupt/child.pid", str(pid_file)
    )
    poller = subprocess.Popen(["sh", "-c", poll], stdout=subprocess.PIPE)
    try:
        time.sleep(0.4)
        child.kill()  # a zombie until this test reaps it
        time.sleep(0.4)
    finally:
        child.kill()
        child.wait()
    stdout, _ = poller.communicate(timeout=10)
    states = checker.child_states(stdout)
    assert states[0] == "present" and "zombie" in states, stdout
    assert states[-1] == "absent" and checker.child_poll_failures(stdout) == [], stdout


# --- the clean clone ----------------------------------------------------------


class Builder:
    """Write one clean clone directory in the shape of the controller and the Host CLI."""

    def __init__(self, tmp_path: Path, os_name: str) -> None:
        self.os_name = os_name
        self.directory = tmp_path / f"{os_name}-01"
        self.downloads = self.directory / "downloads"
        self.downloads.mkdir(parents=True)
        self.manifest_path = _manifest(tmp_path)
        manifest = trial.read_manifest(self.manifest_path)
        self.manifest = manifest
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
            "manifest": str(manifest.path),
            "manifest_sha256": hashlib.sha256(self.manifest_path.read_bytes()).hexdigest(),
            "frozen_sha256": manifest.sha256,
            "identities": trial.identities(manifest, os_name),
            "bounds": trial.bounds(manifest),
            "slot": trial.SLOT_NOTE,
            "plan": json.loads(controller._json_bytes(asdict(self.plan))),  # pyright: ignore[reportPrivateUsage]
        }
        self.subjects = self.frozen_subjects()
        for name in checker.expected_shape(os_name):
            self.default(name)

    def frozen_subjects(self) -> dict[str, str]:
        """The subject digests of a run that measured the frozen candidate."""
        digests = dict(self.manifest.sha256)
        digests[str(self.manifest.path)] = self.inventory["manifest_sha256"]
        fixture = trial.FIXTURES[self.os_name]
        for name, digest in trial.FIXTURE_SHA256[self.os_name].items():
            digests[str((fixture / name).absolute())] = digest
        for module in trial.checker_modules():
            digests[str(module.absolute())] = hashlib.sha256(module.read_bytes()).hexdigest()
        return digests

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

    def cancel_record(self, target: str) -> Json:
        # Spawn at 0.10 s, outcome at 2.05 s, record published at 2.15 s: the
        # helper's cancellation reply then ends at 2.20 s.
        return _record(
            self.os_name, self.id(target), CANCELLED, *PARTIAL, duration=1950, finalised=100
        )

    def cancel_reply(self, target: str, record: Json | None = None) -> Json:
        record = record or self.cancel_record(target)
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
        elif name == "app-identity":
            self.put(name, self.app_record(), elapsed=1.5)
        elif name == "identity":
            self.exec(name, exited0, self.identity_output())
        elif argv[-1] == trial.child_poll("interrupt"):
            # Finding R1 of k241: a zombie while the agent still reaps, then absence.
            self.exec(name, exited0, _polls(ZOMBIE_ROW, None))
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

    def identity_output(self, agent: str | None = None) -> bytes:
        if self.os_name == "macos":
            digest = agent or MACOS_AGENT_SHA256
            return (
                f"{digest}  {trial.MACOS_AGENT}\nProductName:\tmacOS\nDarwin guest 25.0.0\n"
            ).encode()
        return (agent or LINUX_LISTING).encode() + b'NAME="Ubuntu"\nLinux guest 6.8.0\n'

    def app_record(self, **running: Any) -> Json:
        report = {
            "schema_version": "1.0",
            "ok": False,
            "checks": {
                "install_path": {"status": "fail"},
                "app": {
                    "status": "pass",
                    "socket": "/h/testanyware.sock",
                    "running": {
                        "status": "pass",
                        "build": APP_BUILD,
                        "executable": APP_EXECUTABLE,
                        **running,
                    },
                },
            },
        }
        doctor = {
            "argv": [CLI, "doctor", "--json"],
            # `doctor` exits 1 for the install path of a private Host CLI.
            "returncode": 1,
            "reaped": True,
            "signals": [],
            "stdout": base64.b64encode(json.dumps(report).encode()).decode(),
            "stderr": "",
            "started_after": 0.0,
            "ended_after": 1.2,
            "elapsed": 1.2,
        }
        return {"schema": "acceptance-k223-app/1", "doctor": doctor, "ok": True}

    def helper(self, record: Json | None = None, **changes: Any) -> Json:
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
        record = record or self.cancel_record("cancel")
        envelope = _envelope(list(exec_argv), record)
        reply = self.cancel_reply("cancel", record)
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

    def write(self, order: list[str] | None = None) -> Path:
        names = order or checker.expected_shape(self.os_name)
        result = {
            "run_id": self.plan.run_id,
            "ok": True,
            "inspection_failure": None,
            "calls": [self.receipts[name] for name in names if name in self.receipts],
            "unreached": [name for name in names if name not in self.receipts],
            "subjects_before": dict(self.subjects),
            "subjects_after": dict(self.subjects),
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
    return lambda builder: builder.exec(
        "after-timeout-05", {"kind": "exited", "exit_code": 0}, data
    )


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
        "after-timeout-05",
    ),
    "child zombie after timeout": (
        _child("Z Wed Nov 18 03:40:00 2026 [sleep]"),
        "after-timeout-05",
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
    # Finding R5 of k241: a 450 s boot with no slot wait overran the 420 s boot bound.
    "boot overran into the slot allowance": (_receipt("acquire", elapsed=450.0), "acquire"),
    # Finding R1 of k241: the poll still fails a child that outlives it.
    "child survives the interruption poll": (
        lambda b: b.exec(
            "interrupt-child",
            {"kind": "exited", "exit_code": 0},
            _polls(*([SLEEP_ROW] * trial.CHILD_POLLS)),
        ),
        "interrupt-child",
    ),
    "another command at the child PID": (
        lambda b: b.exec(
            "interrupt-child",
            {"kind": "exited", "exit_code": 0},
            _polls(f" {PID} 1 4240 S Wed Nov 18 03:40:00 2026 /usr/bin/vim\n", None),
        ),
        "interrupt-child",
    ),
    # Finding R3 of k241: the answering agent and app must be the frozen ones.
    "answering agent differs": (
        lambda b: b.exec(
            "identity", {"kind": "exited", "exit_code": 0}, b.identity_output("0" * 64)
        ),
        "identity",
    ),
    "agent digest command failed": (
        lambda b: b.exec(
            "identity",
            {"kind": "exited", "exit_code": 0},
            b"ProductName:\tmacOS\nDarwin guest 25.0.0\n",
        ),
        "identity",
    ),
    "answering app build differs": (
        lambda b: b.put("app-identity", b.app_record(build="1.0.0")),
        "app-identity",
    ),
    "answering app executable differs": (
        lambda b: b.put(
            "app-identity",
            b.app_record(executable="/Applications/TestAnyware.app/Contents/MacOS/TestAnyware"),
        ),
        "app-identity",
    ),
    "app helper not ok": (
        lambda b: b.put("app-identity", b.app_record() | {"ok": False}),
        "app-identity",
    ),
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


def _slow_finalisation(builder: Builder, health_start: float) -> Json:
    """Finding R2 of k241: a valid record whose finalisation takes 1.5 s, within bounds."""
    termination = {
        "term_sent_after_ms": 0,
        "kill_sent_after_ms": 10,
        "leader_status": {"kind": "signalled", "signal": 15},
        "leader_exit_after_ms": 5,
        "group_empty": "yes",
        "group_empty_after_ms": 20,
        "within_bounds": True,
    }
    record = _record(
        builder.os_name,
        builder.id("cancel"),
        CANCELLED,
        *PARTIAL,
        termination=termination,
        duration=2000,
        finalised=1500,
    )
    helper = builder.helper(record=record)
    helper["health"] |= {"started_after": health_start, "ended_after": health_start + 0.1}
    helper["cancel"] |= {"started_after": 2.0, "ended_after": 3.7}
    helper["exec"] |= {"ended_after": 3.8}
    return helper


@pytest.mark.parametrize("os_name", OSES)
def test_health_overlap_accepts_a_slow_finalisation_within_bounds(
    tmp_path: Path, os_name: str
) -> None:
    builder = Builder(tmp_path, os_name)
    helper = _slow_finalisation(builder, 1.0)
    builder.put("cancel", helper, elapsed=4.0)
    verdict = checker.check_clone(builder.write())
    assert _failing_cases(verdict) == {} and verdict["ok"] is True
    lifecycle = helper_record_lifecycle(helper)
    # The spawn came by 3.70 - (2.000 + 1.500) = 0.20 s. The old inequality gave 1.70 s.
    latest = checker.spawned_by(lifecycle, helper["cancel"], helper["exec"])
    assert latest == pytest.approx(0.2)


def test_health_overlap_still_rejects_health_before_the_latest_spawn(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "linux")
    builder.put("cancel", _slow_finalisation(builder, 0.15), elapsed=4.0)
    failures = _failing_cases(checker.check_clone(builder.write()))
    assert any("latest possible spawn" in failure for failure in failures["cancel"]), failures


def helper_record_lifecycle(helper: Json) -> Json:
    envelope = json.loads(base64.b64decode(helper["exec"]["stdout"]))
    return cast(Json, envelope["lifecycle"])


def _top_failures(builder: Builder, order: list[str] | None = None) -> list[str]:
    verdict = checker.check_clone(builder.write(order))
    assert verdict["ok"] is False
    return cast(list[str], verdict["failures"])


def test_a_subject_that_differs_from_the_freeze_fails_the_clone(tmp_path: Path) -> None:
    # Finding R4 of k241: the same compatible build before and after the run is
    # still not the frozen build.
    builder = Builder(tmp_path, "macos")
    builder.subjects[CLI] = "9" * 64
    failures = _top_failures(builder)
    assert any(failure.startswith(f"subject {CLI}") for failure in failures), failures


def test_a_fixture_that_differs_from_the_freeze_fails_the_clone(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "linux")
    fixture = trial.FIXTURES["linux"]
    name = sorted(trial.FIXTURE_SHA256["linux"])[0]
    builder.subjects[str((fixture / name).absolute())] = "9" * 64
    assert any("subject" in failure for failure in _top_failures(builder))


def test_a_manifest_changed_after_the_run_fails_the_clone(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "macos")
    builder.write()
    builder.manifest_path.write_text(builder.manifest_path.read_text() + " ")
    failures = checker.check_clone(builder.directory)["failures"]
    assert any("no longer has its frozen digest" in failure for failure in failures), failures


def test_inventory_identities_must_be_the_manifest_identities(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "macos")
    builder.inventory["identities"] = builder.inventory["identities"] | {"app_build": "1.0.0"}
    failures = _top_failures(builder)
    assert "the inventory identities are not the manifest identities" in failures


def test_a_checker_module_changed_after_the_run_fails_the_clone(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "linux")
    module = str(Path(checker.__file__).absolute())
    builder.subjects[module] = "0" * 64
    failures = _top_failures(builder)
    assert any(failure.startswith("checker module") for failure in failures), failures


def test_the_checker_dependency_is_a_subject(tmp_path: Path) -> None:
    plan = _plan(tmp_path, "linux")
    subjects = {str(subject.absolute()) for subject in plan.subjects}
    for module in ("completion_k186_check.py", "acceptance_k223_app.py", "controller.py"):
        assert str((trial.HERE / module).absolute()) in subjects, module


def test_a_recorded_order_that_differs_from_the_plan_fails_the_clone(tmp_path: Path) -> None:
    # Finding R7 of k241: the same unique names, with timeout and after-timeout-01 swapped.
    builder = Builder(tmp_path, "macos")
    order = checker.expected_shape("macos")
    first, second = order.index("timeout"), order.index("after-timeout-01")
    order[first], order[second] = order[second], order[first]
    failures = _top_failures(builder, order)
    assert "the recorded call order is not the plan order" in failures


def test_a_receipt_argv_that_differs_from_the_plan_fails_the_clone(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "linux")
    builder.receipts["ordinary-02"]["argv"] = [*builder.argv("ordinary-02"), "--extra"]
    failures = _top_failures(builder)
    assert "the receipt argv of ordinary-02 is not its planned argv" in failures


def test_decreasing_start_times_fail_the_clone(tmp_path: Path) -> None:
    builder = Builder(tmp_path, "macos")
    builder.receipts["exit-23"]["started_monotonic"] = 999.0
    assert "the recorded start times decrease" in _top_failures(builder)


def test_app_helper_reports_doctor_whatever_its_exit(tmp_path: Path) -> None:
    fake = tmp_path / "fake-doctor"
    fake.write_text(
        f"#!{sys.executable}\nimport json, sys\n"
        "print(json.dumps({'checks': {'app': {'running': {'build': 'b'}}}}))\nsys.exit(1)\n"
    )
    fake.chmod(0o755)
    argv = [
        sys.executable,
        "-I",
        str(trial.HERE / "acceptance_k223_app.py"),
        "--cli",
        str(fake),
        "--observation",
        "5",
    ]
    completed = subprocess.run(argv, capture_output=True, timeout=30)
    assert completed.returncode == 0, completed.stderr
    record = json.loads(completed.stdout)
    assert record["ok"] is True and record["doctor"]["returncode"] == 1
    report = json.loads(base64.b64decode(record["doctor"]["stdout"]))
    assert report["checks"]["app"]["running"]["build"] == "b"
    assert record["doctor"]["argv"] == [str(fake), "doctor", "--json"]


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
