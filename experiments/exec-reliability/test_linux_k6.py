"""Controls for the k6 Linux disconnect tooling (leaf linux-disconnect-availability-k6).

Each container control builds a run directory without Docker. The cases are
synthetic records in the shape that `linux_k6_seam.py cases` prints. The
control case answers its follow-ups after the handler. A test case then
changes one property and expects the checker to classify it.
"""

import copy
import json
from pathlib import Path
from typing import Any

import pytest

import controller
import linux_k6
import linux_k6_check as checker
import linux_k6_seam as seam

HERE = Path(__file__).resolve().parent
S = 1_000_000_000
START = 100 * S
PORT = 18648
SHELL = {"role": "shell", "pid": 10, "ppid": 5}
CHILD = {"role": "child", "pid": 11, "ppid": 10}
ORPHAN = {"role": "child", "pid": 11, "ppid": 1}


def http(status: int, body: bytes) -> str:
    head = f"HTTP/1.0 {status} OK\r\nContent-Type: application/json\r\n\r\n".encode()
    return (head + body).hex()


HEALTH = http(200, json.dumps(checker.HEALTH_BODY).encode())
ORDINARY = http(200, json.dumps({"exitCode": 0, "stdout": "ordinary-ok", "stderr": ""}).encode())
DOWNLOAD = http(200, seam.PARTIAL)
DEADLINE = http(200, json.dumps(checker.DEADLINE_BODY).encode())
REPLIES = {"health": HEALTH, "health-deadline": HEALTH, "ordinary": ORDINARY, "download": DOWNLOAD}


def at(seconds: float) -> int:
    return START + int(seconds * S)


def follow(name: str, began: float, latency: float, port: int) -> dict[str, Any]:
    return {
        "name": name,
        "start_ns": at(began),
        "end_ns": at(began + latency),
        "response_hex": REPLIES[name],
        "bound": seam.FOLLOW_BOUND,
        "bound_exceeded": False,
        "error": None,
        "local_port": port,
    }


def case(name: str, server: str = "serial", gone: float = 20.05) -> dict[str, Any]:
    """One case whose shell ends at `gone` and whose child survives."""
    processes = [
        {"ns": at(0.05), "prev_ns": at(0.0), "value": [SHELL, CHILD]},
        {"ns": at(gone), "prev_ns": at(gone - seam.POLL), "value": [ORPHAN]},
    ]
    if name == checker.CONTROL:
        records = [
            follow("health", 20.06, 0.001, 50001),
            follow("ordinary", 20.07, 0.002, 50002),
            follow("download", 20.08, 0.001, 50003),
        ]
    else:
        records = [
            follow("health", 2.0, gone + 0.01 - 2.0, 50001),
            follow("ordinary", gone + 0.02, 0.002, 50002),
            follow("download", gone + 0.03, 0.001, 50003),
        ]
    return {
        "case": name,
        "server": server,
        "port": PORT,
        "ready": {"ready": True},
        "start_ns": START,
        "processes": processes,
        "after_handler": {"rows": [ORPHAN]},
        "original": {"response_hex": DEADLINE, "end_ns": at(20.01), "local_port": 50000},
        "act": {},
        "follow": records,
        "sockets": [],
        "evidence": {},
        "cleanup": {"kills": [{"pid": 11, "why": "killed"}], "left": []},
        "agent_stop": {},
        "agent_log_hex": "",
    }


def cases() -> list[dict[str, Any]]:
    result = [case(c.name, c.server) for c in seam.CASES]
    threaded = result[-1]
    threaded["follow"] = [
        follow("health", 2.0, 0.002, 50001),
        follow("ordinary", 2.01, 0.002, 50002),
        follow("download", 2.02, 0.001, 50003),
    ]
    return result


def run_dir(tmp_path: Path, items: list[dict[str, Any]], ok: bool = True) -> Path:
    run = tmp_path / "run"
    run.mkdir()
    (run / "cases.json").write_text(json.dumps({"ok": ok}))
    output: dict[str, Any] = {"cases": items, "final_sweep": {"rows": []}, "bounds": seam.bounds()}
    (run / "cases.stdout").write_text(json.dumps(output))
    return run


def judge(tmp_path: Path, items: list[dict[str, Any]]) -> dict[str, Any]:
    return checker.container_check({"verdict": True}, run_dir(tmp_path, items))


def classes(report: dict[str, Any], name: str) -> dict[str, str]:
    verdict = next(v for v in report["cases"] if v["case"] == name)
    return {f["name"]: f["class"] for f in verdict["follow"]}


# ---- shape ----


def test_cases_and_bounds_are_frozen() -> None:
    names = [c.name for c in seam.CASES]
    assert names[0] == checker.CONTROL
    assert [c.server for c in seam.CASES].count("threading") == 1
    assert seam.GUEST_TIMEOUT == 20
    assert seam.ACT_AFTER == 2.0
    assert seam.FOLLOW_BOUND == 30
    assert seam.TRANSFER_BOUND == 150
    assert checker.LATENCY_ALLOWANCE == 5.0


def test_both_plans_pass_the_controller_rules(tmp_path: Path) -> None:
    controller._validate(linux_k6.container_plan("/usr/bin/python3"), None)  # pyright: ignore[reportPrivateUsage]
    tools = {"python": "/usr/bin/python3", "cli": "/bin/testanyware", "tart": "/bin/tart"}
    controller._validate(linux_k6.live_plan(tools, tmp_path), None)  # pyright: ignore[reportPrivateUsage]


def test_the_sweep_owns_payloads_and_agents_but_not_the_container_init() -> None:
    tag = "000000000042"
    assert seam.owned_by_run(["sleep", f"120.3{tag}"], tag)
    assert seam.owned_by_run(["/bin/bash", "-c", f"sleep 120.3{tag} & wait"], tag)
    assert seam.owned_by_run(["python3", "/seam/linux_k6_seam.py", tag, "agent", "18649"], tag)
    assert not seam.owned_by_run(["/sbin/docker-init", "--", "python3", "x.py", tag, "cases"], tag)
    assert not seam.owned_by_run(["python3", "/seam/linux_k6_seam.py", tag, "cases"], tag)


# ---- container oracle ----


def test_clean_cases_are_complete_and_queued_health_is_blocked(tmp_path: Path) -> None:
    report = judge(tmp_path, cases())
    assert report["problems"] == []
    assert set(classes(report, checker.CONTROL).values()) == {"baseline"}
    assert classes(report, "serial-reset") == {
        "health": "blocked",
        "ordinary": "responsive",
        "download": "responsive",
    }
    assert set(classes(report, "threaded-reset").values()) == {"responsive"}
    for verdict in report["cases"]:
        assert verdict["handler"]["class"] == "deadline"
        assert verdict["child"]["class"] == "survived-handler"


def test_a_fast_answer_that_spans_the_handler_end_is_blocked(tmp_path: Path) -> None:
    items = cases()
    items[3]["follow"][0] = follow("health", 19.9, 0.2, 50001)
    report = judge(tmp_path, items)
    assert classes(report, "serial-reset")["health"] == "blocked"


def test_an_exceeded_bound_or_wrong_body_is_failed(tmp_path: Path) -> None:
    items = cases()
    items[5]["follow"][0]["bound_exceeded"] = True
    items[5]["follow"][0]["response_hex"] = ""
    items[4]["follow"][2]["response_hex"] = http(200, b"other\n")
    report = judge(tmp_path, items)
    assert classes(report, "host-deadline")["health"] == "failed"
    assert classes(report, "serial-half-close")["download"] == "failed"
    assert report["problems"] == []


@pytest.mark.parametrize(
    ("gone", "expected"), [(10.0, "cancelled"), (20.05, "deadline"), (30.0, "overran")]
)
def test_the_handler_class_follows_the_shell_end(
    tmp_path: Path, gone: float, expected: str
) -> None:
    items = cases()
    items[3] = case("serial-reset", gone=gone)
    report = judge(tmp_path, items)
    verdict = next(v for v in report["cases"] if v["case"] == "serial-reset")
    assert verdict["handler"]["class"] == expected


def test_a_child_that_ends_with_the_shell_did_not_survive(tmp_path: Path) -> None:
    items = cases()
    items[2]["after_handler"] = {"rows": []}
    items[2]["processes"][1]["value"] = []
    report = judge(tmp_path, items)
    verdict = next(v for v in report["cases"] if v["case"] == "serial-close")
    assert verdict["child"]["class"] == "ended-with-handler"


@pytest.mark.parametrize(
    "edit",
    ["no-shell", "left", "control-late", "receipt", "missing-case"],
)
def test_incomplete_evidence_is_a_problem(tmp_path: Path, edit: str) -> None:
    items = cases()
    ok = True
    if edit == "no-shell":
        items[1]["processes"] = []
        items[1]["after_handler"] = {"rows": []}
    elif edit == "left":
        items[1]["cleanup"]["left"] = [ORPHAN]
    elif edit == "control-late":
        items[0] = case(checker.CONTROL, gone=30.0)
    elif edit == "receipt":
        ok = False
    else:
        del items[2]
    report = checker.container_check({"verdict": True}, run_dir(tmp_path, items, ok))
    assert report["problems"]


def test_the_queue_record_reads_unread_request_bytes(tmp_path: Path) -> None:
    items = cases()
    agent = f"0100007F:{PORT:04X}"
    remote = "0100007F:C351"  # 50001, the health follow-up
    items[3]["sockets"] = [
        {"ns": at(0.0), "prev_ns": None, "value": [[f"00000000:{PORT:04X}", "x", "0A", 0, 0]]},
        {
            "ns": at(2.05),
            "prev_ns": at(2.0),
            "value": [[f"00000000:{PORT:04X}", "x", "0A", 0, 1], [agent, remote, "01", 0, 66]],
        },
        {"ns": at(20.06), "prev_ns": at(20.01), "value": [[agent, remote, "01", 0, 0]]},
    ]
    report = judge(tmp_path, items)
    verdict = next(v for v in report["cases"] if v["case"] == "serial-reset")
    assert verdict["listen_peak"] == 1
    queue = verdict["follow"][0]["queue"]
    assert queue == {"waited_from": pytest.approx(2.05), "read_by": pytest.approx(20.06)}
    assert verdict["queue_release"] == pytest.approx(0.01)


# ---- live rule ----


LIVE_01 = (
    HERE / "evidence/linux-disconnect-availability-k6/live-01/f5158780256d4fad9def8d77a9554797"
)


def live_result(mode: str) -> dict[str, Any]:
    return json.loads((LIVE_01 / f"burst-{mode}.stdout").read_bytes())


def test_the_linux_rules_judge_the_retained_live_01_records() -> None:
    control = live_result("control")
    assert checker.linux_original(control)["expected"] is True
    assert checker.linux_original(control)["timed_out"] is False
    assert checker.live_handler(control)[0] == "deadline"
    term = live_result("term")
    handler, lo, hi, basis = checker.live_handler(term)
    assert (handler, lo, basis) == ("deadline", None, "queue-release")
    assert hi == pytest.approx(20.069, abs=0.001)
    adopted = checker.adoption(term)
    assert adopted is not None
    assert adopted["parent_command"] == "/usr/lib/systemd/systemd --user"


def test_the_payload_shell_alive_gives_a_lower_end() -> None:
    term = copy.deepcopy(live_result("term"))
    first = term["follow"][0]
    body = json.loads(bytes.fromhex(first["stdout_hex"]))
    body["stdout"] = (
        "shell pid=900 present ppid=850 command=/bin/bash -c mkdir -p /tmp/taw-k13/term\n"
        "child pid=2189 present ppid=900 command=sleep 120"
    )
    first["stdout_hex"] = json.dumps(body).encode().hex()
    handler, lo, _hi, basis = checker.live_handler(term)
    assert (handler, basis) == ("deadline", "bracket")
    assert lo == pytest.approx(2.009, abs=0.001)


def test_a_reply_without_the_linux_deadline_text_is_unexpected() -> None:
    control = copy.deepcopy(live_result("control"))
    body = json.loads(bytes.fromhex(control["original"]["stdout_hex"]))
    body["stderr"] = ""
    control["original"]["stdout_hex"] = json.dumps(body).encode().hex()
    assert checker.linux_original(control)["expected"] is False
