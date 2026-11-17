"""Controls for the traced probe and checker of leaf `macos-exit-observation-instrumented-k195`."""

import json
from pathlib import Path
from typing import Any

import exit_k195
import exit_k195_check as checker
import timeout_k4
import timeout_k4_check

TOOLS = {"python": "/usr/bin/python3", "cli": "/checkout/testanyware", "tart": "/t"}
CONTROLS = Path(__file__).parent / "evidence/macos-exit-observation-k195/controls-01"


def _agent(tmp_path: Path) -> Path:
    agent = tmp_path / "testanyware-agent"
    agent.write_bytes(b"traced agent")
    return agent


def test_plan_runs_the_k4_workload_after_the_install(tmp_path: Path) -> None:
    plan = exit_k195.plan(TOOLS, _agent(tmp_path), tmp_path)
    names = tuple(call.name for call in plan.calls)
    install = ("health-before", "upload-agent", "install-agent", "restart-pause", "verify-agent")
    k4 = timeout_k4_check.CONDITION_1[1:] + timeout_k4_check.AFTER
    assert names == install + k4 + ("download-trace",)
    assert checker.WORKLOAD == k4
    (resource,) = plan.resources
    assert resource.identity == f"k195-exit-macos-{plan.run_id}"
    assert resource.cleanup.argv[-1] == timeout_k4.OWNED_CLEANUP
    assert all(call.argv[0] in {TOOLS["cli"], "/bin/sh", "/bin/sleep"} for call in plan.calls)


def test_workload_keeps_the_k4_agent_requests(tmp_path: Path) -> None:
    plan = exit_k195.plan(TOOLS, _agent(tmp_path), tmp_path)
    vm = plan.resources[0].identity
    frozen = {
        c.name: c
        for c in [
            *timeout_k4.issue_calls(TOOLS["cli"], vm),
            *timeout_k4.after_calls(TOOLS["cli"], vm),
        ]
    }
    for call in plan.calls:
        if call.name not in frozen:
            continue
        if call.argv[0] == "/bin/sh":
            assert call.argv[3:] == frozen[call.name].argv
            assert frozen[call.name].argv[1:3] == ("agent", "health")
        else:
            assert call == frozen[call.name]


def test_install_checks_the_digest_and_detaches_the_restart() -> None:
    command = exit_k195.install_command("ab" * 32)
    assert "set -eu" in command and "sudo -n install -m 0755" in command
    assert command.count("ab" * 32) == 2
    assert "nohup sh -c 'sleep 2; launchctl kickstart -k gui/$(id -u)/" in command
    assert command.endswith("echo restart-scheduled")


def _records(path: Path) -> list[dict[str, Any]]:
    return checker.load_trace(path)[0]


def test_classify_reads_the_four_real_control_orderings() -> None:
    records = _records(CONTROLS / "trace-01.jsonl")
    groups = checker._calls(records)  # pyright: ignore[reportPrivateUsage]
    classes = [checker.classify(groups[n]) for n in sorted(groups)]
    ignored, late, first_true, second_true = classes
    assert ignored["deadline"] == "command-alive" and ignored["term"] == "term-timeout-kill"
    assert late["deadline"] == "command-alive" and late["term"] == "term-success"
    trues = {first_true["deadline"], second_true["deadline"]}
    assert trues == {"observed", "exited-unobserved"}
    starved = next(c for c in (first_true, second_true) if c["deadline"] == "exited-unobserved")
    assert starved["starved"] is True and starved["term"] == "term-success"
    assert starved["shell_exit_ms"] < 10 < 1900 < starved["deadline_ms"]


def test_classify_without_a_deadline_record_is_unknown() -> None:
    group = [{"seq": 1, "t_ns": 1, "call": 1, "event": "begin", "command": "x"}]
    assert checker.classify(group)["deadline"] == "unknown"


def _write_run(tmp_path: Path, old: str, new: str, first_agent: str, digest: str) -> Path:
    out = tmp_path / "out"
    run = out / ("a" * 32)
    (out / "downloads").mkdir(parents=True)
    run.mkdir()
    (out / "inventory.json").write_text(json.dumps({"agent_sha256": "ab" * 32}))
    install = exit_k195.install_command("ab" * 32)
    receipt = {
        "schema_version": "1.0",
        "ok": True,
        "exit_code": 0,
        "timed_out": False,
        "stderr": "",
    }
    (run / "install-agent.stdout").write_text(
        json.dumps({**receipt, "command": install, "stdout": f"old={old}\nrestart-scheduled\n"})
    )
    first = json.dumps(
        {"seq": 1, "t_ns": 5, "agent": int(first_agent), "call": 0, "event": "trace-open"}
    )
    verify = f"agent={new}\nsha={digest}\nstarted=x\nfirst={first}\n"
    (run / "verify-agent.stdout").write_text(
        json.dumps({**receipt, "command": exit_k195.VERIFY, "stdout": verify})
    )
    return run


def test_route_accepts_only_the_traced_agent_answering(tmp_path: Path) -> None:
    good = checker.route(_write_run(tmp_path / "1", "10", "20", "20", "ab" * 32), "ab" * 32)
    assert good["route"] == "traced", good["failures"]
    same = checker.route(_write_run(tmp_path / "2", "10", "10", "10", "ab" * 32), "ab" * 32)
    assert same["route"] == "unavailable"
    other = checker.route(_write_run(tmp_path / "3", "10", "20", "30", "ab" * 32), "ab" * 32)
    assert other["route"] == "unavailable"
    digest = checker.route(_write_run(tmp_path / "4", "10", "20", "20", "cd" * 32), "ab" * 32)
    assert digest["route"] == "unavailable"


def test_absent_trace_never_proves_a_cause(tmp_path: Path) -> None:
    run = _write_run(tmp_path, "10", "20", "20", "ab" * 32)
    result = checker.check(run)
    assert result["route"] == "traced"
    assert result["evidence"] == "absent" and result["stall"] == "unknown"
    assert result["cleanup"]["state"] == "unknown"
    assert result["disposal_failures"]


def _entry(
    deadline: str, timed_out: bool, intentional: bool = False, **extra: Any
) -> dict[str, Any]:
    return {"deadline": deadline, "timed_out": timed_out, "intentional": intentional, **extra}


def test_cause_names_candidate_one_only_for_starved_exits() -> None:
    base = {"issue-sleep": _entry("command-alive", True, True, term="term-success", alive=[])}
    none = checker.cause({**base, "after-09-table": _entry("observed", False)})
    assert none["stall"] == "not-reproduced" and none["statement"].startswith("unproved")
    starved = checker.cause(
        {**base, "after-09-table": _entry("exited-unobserved", True, starved=True)}
    )
    assert starved["statement"].startswith("candidate 1")
    unstarved = checker.cause(
        {**base, "after-09-table": _entry("exited-unobserved", True, starved=False)}
    )
    assert unstarved["statement"].startswith("unproved")
    alive = checker.cause(
        {
            **base,
            "after-09-table": _entry("command-alive", True, alive=[{"pid": 5, "path": "/bin/ps"}]),
        }
    )
    assert "/bin/ps" in alive["statement"]
    unknown = checker.cause({**base, "after-09-table": _entry("unknown", False)})
    assert unknown["evidence"] == "incomplete" and unknown["stall"] == "unknown"
