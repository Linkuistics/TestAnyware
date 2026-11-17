"""Controls for the k13 disconnect probes and checker. None of them needs a VM."""

from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path
from typing import Any

import pytest

import disconnect_k13
import disconnect_k13_burst as burst
import disconnect_k13_check as checker
import disconnect_k13_seam as seam
import timeout_k4

TOOLS = {"python": "/usr/bin/python3", "cli": "/opt/homebrew/bin/testanyware", "tart": "/t"}
NS = 1_000_000_000
START = 1_000 * NS
TAG = "MODE"  # the fixtures name this directory, and `retag` gives each mode its own


def at(seconds: float) -> int:
    return START + int(seconds * NS)


# ---- plan, payload and cleanup ----


def test_payload_is_the_issue_payload_with_only_its_directory_changed() -> None:
    for mode in burst.MODES:
        text = burst.payload(mode).replace(burst.guest_dir(mode), "/tmp/taw-exec-reliability")
        assert text == timeout_k4.ISSUE_CHILD


def test_plan_runs_three_bursts_between_two_health_checks(tmp_path: Path) -> None:
    plan = disconnect_k13.plan(TOOLS, tmp_path)
    names = [call.name for call in plan.calls]
    assert names == [
        "health-before",
        "burst-control",
        "burst-concurrent",
        "burst-term",
        "health-after",
    ]
    (resource,) = plan.resources
    assert resource.identity == f"k13-macos-{plan.run_id}"
    assert "--base" in resource.acquire.argv
    assert timeout_k4.GOLDEN in resource.acquire.argv
    for call in plan.calls[1:4]:
        assert call.budget == disconnect_k13.BURST_BUDGET
        assert call.argv[1:3] == ("-E", "-s")


def test_burst_budget_covers_the_longest_mode() -> None:
    longest = (
        burst.ORIGINAL_BOUND
        + burst.TRANSFER_BOUND
        + 6 * burst.FOLLOW_BOUND
        + 8 * 2 * burst.STOP_WAIT
    )
    assert disconnect_k13.BURST_BUDGET > longest


def test_cleanup_targets_only_the_k13_child_files() -> None:
    script = disconnect_k13.cleanup_script()
    assert "/tmp/taw-exec-reliability" not in script
    for mode in burst.MODES:
        assert f"{burst.guest_dir(mode)}/child.pid" in script
    assert script.endswith('[ "$outcome" = confirmed ]')
    assert len(script.split("\n")) == len(timeout_k4.OWNED_CLEANUP.split("\n"))


def test_host_cleanup_owns_only_the_shapes_the_run_starts() -> None:
    tok = seam.token(seam.CASES[0], "4718293846")
    shell = f"/bin/bash -c mkdir -p /tmp/k13x/c1\\012sleep {tok} & child=$!\\012wait"
    work = "/var/folders/xy/T/k13-build-abc"
    assert seam.owned(f"sleep {tok}", (tok,))
    assert seam.owned(shell, (tok,))
    assert seam.owned(f"{work}/copy/.build/debug/testanyware-agent --port 1", (), work)
    assert not seam.owned(f"grep {tok}", (tok,))
    assert not seam.owned(f"sleep {tok}0", (tok,))
    assert not seam.owned(f"tail -f {work}/build.log", (tok,), work)
    assert not seam.owned(f"/usr/bin/less {work}-other/log", (), work)


@pytest.mark.parametrize("mode", burst.MODES)
def test_inspection_reports_shell_and_child_states(mode: str) -> None:
    del mode
    tag = "selftest"
    shell = subprocess.Popen(["/bin/bash", "-c", burst.payload(tag)])  # noqa: S603
    try:
        time.sleep(0.5)
        roles = run_inspection(tag)
        assert roles["shell"]["state"] == "present"
        assert roles["child"]["state"] == "present"
        assert roles["child"]["ppid"] == str(shell.pid)
        assert roles["child"]["command"] == "sleep 120"
        shell.kill()
        shell.wait()
        time.sleep(0.2)
        roles = run_inspection(tag)
        assert roles["shell"]["state"] == "absent"
        assert roles["child"]["state"] == "present"
        assert roles["child"]["ppid"] == "1"
    finally:
        child = Path(burst.guest_dir(tag)) / "child.pid"
        if child.is_file():
            subprocess.run(["/bin/kill", "-KILL", child.read_text().strip()], check=False)  # noqa: S603
        subprocess.run(["/bin/rm", "-rf", burst.guest_dir(tag)], check=True)  # noqa: S603


def run_inspection(tag: str) -> dict[str, dict[str, str]]:
    stdout = subprocess.run(  # noqa: S603
        ["/bin/bash", "-c", burst.inspection(tag)], capture_output=True, text=True, check=True
    ).stdout
    roles = checker.parse_inspection(cli_record("inspect", stdout_json(stdout, 0), 0.0, 0.1))
    assert roles is not None
    return roles


# ---- live checker ----


def stdout_json(stdout: str, exit_code: int, **extra: object) -> dict[str, Any]:
    return {"ok": True, "exit_code": exit_code, "stdout": stdout, "timed_out": False, **extra}


def cli_record(
    name: str, body: dict[str, Any], begin: float, end: float, returncode: int = 0
) -> dict[str, Any]:
    return {
        "name": name,
        "start_ns": at(begin),
        "end_ns": at(end),
        "bound_exceeded": False,
        "returncode": returncode,
        "stdout_hex": json.dumps(body).encode().hex(),
        "stderr_hex": "",
    }


def inspection_text(shell: str, child: str, child_ppid: str = "500") -> str:
    shell_cmd = f"/bin/bash -c mkdir -p {burst.guest_dir(TAG)}" if shell == "present" else ""
    shell_ppid = "400" if shell == "present" else ""
    child_cmd = "sleep 120" if child == "present" else ""
    ppid = child_ppid if child == "present" else ""
    return (
        f"shell pid=500 {shell} ppid={shell_ppid} command={shell_cmd}\n"
        f"child pid=501 {child} ppid={ppid} command={child_cmd}\n"
    )


def inspect(
    name: str, begin: float, end: float, shell: str, child: str, ppid: str = "500"
) -> dict[str, Any]:
    return cli_record(name, stdout_json(inspection_text(shell, child, ppid), 0), begin, end)


def follow(begin: float, step: float, shell: str, child: str) -> list[dict[str, Any]]:
    t = begin
    records: list[dict[str, Any]] = []
    for name in checker.FOLLOW_NAMES:
        if name.startswith("inspect"):
            records.append(inspect(name, t, t + step, shell, child))
        elif name == "health":
            body = {"ok": True, "reachable": True, "accessibility_status": "granted"}
            records.append(cli_record(name, body, t, t + step))
        elif name == "ordinary":
            records.append(cli_record(name, stdout_json("ordinary-ok", 0), t, t + step))
        else:
            record = cli_record(name, {"ok": True}, t, t + step)
            record["local_hex"] = burst.PARTIAL.hex()
            records.append(record)
        t += step
    return records


def original_reply(exit_at: float) -> dict[str, Any]:
    body = {"ok": True, "exit_code": -1, "timed_out": True, "stdout": ""}
    return {
        "pid": 9,
        "signals": [],
        "bound_exceeded": False,
        "exit_ns": at(exit_at),
        "returncode": 0,
        "stdout_hex": json.dumps(body).encode().hex(),
        "stderr_hex": "",
    }


def retag(value: dict[str, Any], mode: str, source: str = TAG) -> dict[str, Any]:
    """Give every inspection record the payload directory of `mode`."""
    records = [*value["follow"], value["mid"], value["late"]]
    for record in records:
        if str(record["name"]).startswith("inspect"):
            text = bytes.fromhex(record["stdout_hex"]).decode()
            new = text.replace(burst.guest_dir(source), burst.guest_dir(mode))
            record["stdout_hex"] = new.encode().hex()
    return value


def result(mode: str, *, late_child: str = "absent", step: float = 0.2) -> dict[str, Any]:
    return retag(raw_result(mode, late_child=late_child, step=step), mode)


def raw_result(mode: str, *, late_child: str, step: float) -> dict[str, Any]:
    late = inspect("inspect-late", 30.0, 30.2, "absent", late_child, "1")
    if mode == "control":
        return {
            "mode": mode,
            "start_ns": START,
            "original": original_reply(20.1),
            "follow": follow(20.2, step, "absent", "absent"),
            "mid": inspect("inspect-mid", 21.5, 21.7, "absent", "absent"),
            "late": late,
        }
    original = original_reply(20.1)
    if mode == "term":
        original.update(
            signals=[{"signal": "SIGTERM", "ns": at(2.0), "result": "sent"}],
            exit_ns=at(2.02),
            returncode=-15,
            stdout_hex="",
        )
    return {
        "mode": mode,
        "start_ns": START,
        "original": original,
        "follow": follow(2.05 if mode == "term" else 2.0, step, "present", "present"),
        "mid": inspect("inspect-mid", 18.0, 18.2, "present", "present"),
        "late": late,
    }


def write_run(tmp_path: Path, results: dict[str, dict[str, Any]]) -> Path:
    run = tmp_path / "run"
    run.mkdir()
    for name in ("health-before", "health-after", *(f"burst-{m}" for m in results)):
        (run / f"{name}.json").write_text(json.dumps({"ok": True}))
    for mode, value in results.items():
        (run / f"burst-{mode}.stdout").write_text(json.dumps(value))
    out = tmp_path / "out"
    out.mkdir()
    (out / "summary.json").write_text(json.dumps({"verdict": True, "run": str(run)}))
    return out


def all_modes(**term: Any) -> dict[str, dict[str, Any]]:
    return {
        "control": result("control"),
        "concurrent": result("concurrent"),
        "term": result("term", **term),
    }


def retag_all(results: dict[str, dict[str, Any]]) -> dict[str, dict[str, Any]]:
    return {mode: retag(value, mode) for mode, value in results.items()}


def judged(tmp_path: Path, results: dict[str, dict[str, Any]]) -> dict[str, Any]:
    out = write_run(tmp_path, results)
    summary = json.loads((out / "summary.json").read_text())
    return checker.live_check(summary, Path(summary["run"]))


def mode_verdict(report: dict[str, Any], mode: str) -> dict[str, Any]:
    return next(v for v in report["modes"] if v["mode"] == mode)


def test_live_expected_run_has_no_problem(tmp_path: Path) -> None:
    report = judged(tmp_path, all_modes())
    assert report["problems"] == []
    term = mode_verdict(report, "term")
    assert term["handler"] == "deadline"
    assert term["child"] == "ended-with-handler"
    assert {f["class"] for f in term["follow"]} == {"responsive"}
    control = mode_verdict(report, "control")
    assert {f["class"] for f in control["follow"]} == {"baseline"}


def test_live_shell_of_another_mode_is_not_the_handler(tmp_path: Path) -> None:
    results = all_modes()
    results["term"] = retag(results["term"], "concurrent", source="term")
    report = judged(tmp_path, results)
    assert mode_verdict(report, "term")["handler_alive_at"] is None
    assert any("term: handler unknown" in p for p in report["problems"])


def test_live_surviving_child_is_a_finding_not_a_problem(tmp_path: Path) -> None:
    report = judged(tmp_path, all_modes(late_child="present"))
    assert report["problems"] == []
    assert mode_verdict(report, "term")["child"] == "survived-handler"


def test_live_slow_follow_up_is_blocked(tmp_path: Path) -> None:
    results = all_modes()
    slow = results["term"]["follow"][1]
    slow["end_ns"] = slow["start_ns"] + int((checker.LATENCY_ALLOWANCE + 1.0) * NS)
    report = judged(tmp_path, results)
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["health"] == "blocked"
    assert report["problems"] == []


def test_live_answer_after_handler_end_is_blocked(tmp_path: Path) -> None:
    results = all_modes()
    health = results["term"]["follow"][1]
    health["start_ns"], health["end_ns"] = at(17.0), at(21.0)
    # Inspections place the start before a sighting alive and the answer after a sighting gone.
    results["term"]["follow"][4] = inspect("inspect-after", 20.5, 20.7, "absent", "absent")
    report = judged(tmp_path, results)
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["health"] == "blocked"


def test_live_request_begun_after_handler_end_is_judged_by_latency(tmp_path: Path) -> None:
    results = all_modes()
    results["term"]["follow"][3]["start_ns"] = at(21.0)
    results["term"]["follow"][3]["end_ns"] = at(21.1)
    results["term"]["follow"][4] = inspect("inspect-after", 21.1, 21.3, "absent", "absent")
    results["term"]["mid"] = inspect("inspect-mid", 18.0, 18.2, "present", "present")
    report = judged(tmp_path, retag_all(results))
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["inspect-after"] == "responsive"


def test_live_slow_answer_of_unknown_order_is_blocked(tmp_path: Path) -> None:
    results = all_modes()
    health = results["term"]["follow"][1]
    health["start_ns"], health["end_ns"] = at(12.0), at(21.0)
    results["term"]["follow"][4] = inspect("inspect-after", 21.0, 21.2, "absent", "absent")
    report = judged(tmp_path, results)
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["health"] == "blocked"


def test_live_fast_answer_of_unknown_order_is_responsive(tmp_path: Path) -> None:
    results = all_modes()
    health = results["term"]["follow"][1]
    health["start_ns"], health["end_ns"] = at(20.0), at(20.3)
    results["term"]["follow"][4] = inspect("inspect-after", 20.3, 20.5, "absent", "absent")
    report = judged(tmp_path, results)
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["health"] == "responsive"
    assert report["problems"] == []


def test_live_failed_download_stays_a_failure(tmp_path: Path) -> None:
    results = all_modes()
    results["term"]["follow"][3]["local_hex"] = None
    results["term"]["follow"][3]["returncode"] = 1
    report = judged(tmp_path, results)
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["download"] == "failed"
    assert report["problems"] == []


def test_live_failed_late_inspection_is_unknown(tmp_path: Path) -> None:
    results = all_modes()
    results["term"]["late"]["returncode"] = 1
    report = judged(tmp_path, results)
    assert mode_verdict(report, "term")["child"] == "unknown"
    assert any("term: handler" in p for p in report["problems"])


def test_live_early_shell_end_is_cancelled(tmp_path: Path) -> None:
    results = all_modes()
    results["term"]["mid"] = inspect("inspect-mid", 18.0, 18.2, "absent", "absent")
    results["term"]["follow"][4] = inspect("inspect-after", 3.0, 3.2, "absent", "absent")
    report = judged(tmp_path, results)
    assert mode_verdict(report, "term")["handler"] == "cancelled"


def test_live_shell_alive_at_late_inspection_overran(tmp_path: Path) -> None:
    results = all_modes()
    results["term"]["late"] = inspect("inspect-late", 30.0, 30.2, "present", "present")
    report = judged(tmp_path, results)
    assert mode_verdict(report, "term")["handler"] == "overran"


def test_live_control_that_does_not_time_out_is_a_problem(tmp_path: Path) -> None:
    results = all_modes()
    results["control"]["original"]["stdout_hex"] = json.dumps({"ok": False}).encode().hex()
    report = judged(tmp_path, results)
    assert any(p.startswith("control:") for p in report["problems"])


def test_live_missing_burst_is_a_problem(tmp_path: Path) -> None:
    results = all_modes()
    del results["term"]
    report = judged(tmp_path, results)
    assert any("burst-term printed no JSON" in p for p in report["problems"])


# ---- host checker ----


def host_case(
    name: str, path: str, action: str, shell_end: float, child_end: float
) -> dict[str, Any]:
    case: dict[str, Any] = {
        "case": name,
        "path": path,
        "action": action,
        "start_ns": START,
        "act_ns": at(2.0),
        "identities": [
            {"role": "shell", "gone_ns": at(shell_end), "gone_reason": "absent"},
            {"role": "child", "gone_ns": at(child_end), "gone_reason": "absent"},
        ],
        "follow": [],
        "sweep": [],
    }
    if path == "http":
        body = b'{"stderr":"x","exitCode":-1,"timedOut":true,"stdout":""}'
        raw = b"HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\n" + body
        case["original"] = {"chunks": [{"ns": at(shell_end), "hex": raw.hex()}]}
        for kind, reply in (("health", b'{"platform":"macos"}'), ("ordinary", None)):
            reply = reply or b'{"stdout":"ordinary-ok","exitCode":0}'
            case["follow"].append(
                {
                    "kind": kind,
                    "path": "http",
                    "status": 200,
                    "body_hex": reply.hex(),
                    "start_ns": at(2.0),
                    "end_ns": at(2.05),
                }
            )
    else:
        reply = {"ok": True, "exit_code": -1, "timed_out": True}
        original: dict[str, Any] = {
            "returncode": 0,
            "exit_ns": at(shell_end + 0.01),
            "stdout_hex": json.dumps(reply).encode().hex(),
            "signals": [],
        }
        case["original"] = original
        if action == "term":
            original.update(
                returncode=-15,
                exit_ns=at(2.02),
                stdout_hex="",
                signals=[{"signal": "SIGTERM", "ns": at(2.0)}],
            )
        health = {"ok": True, "reachable": True}
        ordinary = {"stdout": "ordinary-ok", "exit_code": 0}
        for kind, rc, body in (("health", 4, health), ("ordinary", 0, ordinary)):
            case["follow"].append(
                {
                    "kind": kind,
                    "path": "cli",
                    "bound_exceeded": False,
                    "returncode": rc,
                    "stdout_hex": json.dumps(body).encode().hex(),
                    "start_ns": at(2.0),
                    "end_ns": at(2.1),
                }
            )
    if name == "http-control":
        case["follow"] = []
    return case


def host_seam(**changes: tuple[float, float]) -> dict[str, Any]:
    cases: list[dict[str, Any]] = []
    for case in checker.CASES:
        shell_end, child_end = changes.get(case.name.replace("-", "_"), (20.1, 20.1))
        cases.append(host_case(case.name, case.path, case.action, shell_end, child_end))
    return {
        "servers": [{"role": "agent", "ready": True}],
        "stops": [{"role": "agent", "state": "terminated"}],
        "final_sweep": [],
        "root_removed": True,
        "cases": cases,
    }


SUMMARY: dict[str, Any] = {"verdict": True, "leftovers": [], "build_dir_removed": True}


def test_host_expected_run_has_no_problem() -> None:
    report = checker.host_check(SUMMARY, host_seam())
    assert report["problems"] == []
    classes = {(c["case"], f["kind"]): f["class"] for c in report["cases"] for f in c["follow"]}
    assert set(classes.values()) == {"responsive"}


def test_host_disconnect_that_cancels_is_a_finding() -> None:
    report = checker.host_check(SUMMARY, host_seam(http_close=(2.3, 2.3)))
    verdict = next(c for c in report["cases"] if c["case"] == "http-close")
    assert verdict["handler"] == "cancelled"
    assert report["problems"] == []


def test_host_surviving_child_is_a_finding() -> None:
    report = checker.host_check(SUMMARY, host_seam(cli_term=(20.1, 25.0)))
    verdict = next(c for c in report["cases"] if c["case"] == "cli-term")
    assert verdict["child"] == "survived-handler"


def test_host_missing_identity_is_a_problem() -> None:
    seam = host_seam()
    seam["cases"][6]["identities"] = []
    report = checker.host_check(SUMMARY, seam)
    assert any("cli-term: handler unknown" in p for p in report["problems"])


def test_host_missing_output_is_a_problem() -> None:
    report = checker.host_check(SUMMARY, None)
    assert report["problems"] == ["the host call printed no JSON"]


def reported(record: dict[str, Any], stdout: str) -> None:
    body = {"ok": True, "exit_code": -1, "timed_out": True, "stdout": stdout}
    record["stdout_hex"] = json.dumps(body).encode().hex()
    record["end_ns"] = record["start_ns"] + 10 * NS


def test_live_completed_reported_timeout_is_a_finding(tmp_path: Path) -> None:
    results = all_modes()
    reported(results["term"]["follow"][2], "ordinary-ok")
    report = judged(tmp_path, results)
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["ordinary"] == "completed-reported-timeout"
    assert report["problems"] == []


def test_live_reported_timeout_inspection_still_places_the_handler(tmp_path: Path) -> None:
    results = all_modes()
    mid = results["term"]["mid"]
    text = json.loads(bytes.fromhex(mid["stdout_hex"]))["stdout"]
    reported(mid, text)
    report = judged(tmp_path, results)
    verdict = mode_verdict(report, "term")
    assert verdict["handler"] == "deadline"
    assert verdict["handler_alive_at"] == pytest.approx(18.0)


def test_live_partial_output_with_reported_timeout_is_failed(tmp_path: Path) -> None:
    results = all_modes()
    reported(results["term"]["follow"][2], "")
    report = judged(tmp_path, results)
    classes = {f["name"]: f["class"] for f in mode_verdict(report, "term")["follow"]}
    assert classes["ordinary"] == "failed"


def test_inspection_whose_shell_is_not_the_child_parent_is_rejected() -> None:
    record = inspect("inspect", 2.0, 2.1, "absent", "present", ppid="777")
    assert checker.parse_inspection(record) is None


def test_inspection_ignores_a_shell_record_from_an_earlier_child(tmp_path: Path) -> None:
    del tmp_path
    tag = "selftest-stale"
    directory = Path(burst.guest_dir(tag))
    directory.mkdir(parents=True)
    (directory / "shell.pid").write_text("1\n")
    (directory / "shell.of.999999").write_text("1\n")
    shell = subprocess.Popen(["/bin/bash", "-c", burst.payload(tag)])  # noqa: S603
    try:
        time.sleep(0.5)
        roles = run_inspection(tag)
        assert roles["shell"]["pid"] == str(shell.pid)
        assert roles["shell"]["state"] == "present"
    finally:
        shell.kill()
        shell.wait()
        child = directory / "child.pid"
        if child.is_file():
            subprocess.run(["/bin/kill", "-KILL", child.read_text().strip()], check=False)  # noqa: S603
        subprocess.run(["/bin/rm", "-rf", str(directory)], check=True)  # noqa: S603


def test_live_term_falls_back_to_the_control_reference(tmp_path: Path) -> None:
    results = all_modes()
    reported(results["concurrent"]["follow"][2], "ordinary-ok")
    report = judged(tmp_path, results)
    concurrent = {f["name"]: f["class"] for f in mode_verdict(report, "concurrent")["follow"]}
    term = {f["name"]: f for f in mode_verdict(report, "term")["follow"]}
    assert concurrent["ordinary"] == "completed-reported-timeout"
    assert term["ordinary"]["class"] == "responsive"
    assert term["ordinary"]["reference_latency"] == pytest.approx(0.2)
    assert report["problems"] == []


def test_host_completed_reported_timeout_in_a_control_leaves_the_comparison_unknown() -> None:
    seam = host_seam()
    control = next(c for c in seam["cases"] if c["case"] == "cli-concurrent")
    ordinary = control["follow"][1]
    body = {"stdout": "ordinary-ok", "exit_code": -1, "timed_out": True}
    ordinary["stdout_hex"] = json.dumps(body).encode().hex()
    report = checker.host_check(SUMMARY, seam)
    classes = {(c["case"], f["kind"]): f for c in report["cases"] for f in c["follow"]}
    assert classes[("cli-concurrent", "ordinary")]["class"] == "completed-reported-timeout"
    assert classes[("cli-term", "ordinary")]["class"] == "unknown"
    assert classes[("cli-term", "ordinary")]["unknown_reason"] == "no-sound-control"
    assert report["problems"] == ["cli-term: a follow-up outcome is unknown"]
