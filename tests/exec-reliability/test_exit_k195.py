"""Controls for the traced probe and checker of leaf `macos-exit-observation-instrumented-k195`."""

import json
from pathlib import Path
from typing import Any

import exit_k195
import exit_k195_check as checker
import timeout_k4
import timeout_k4_check

TOOLS = {"python": "/usr/bin/python3", "cli": "/checkout/testanyware", "tart": "/t"}
CONTROLS = Path(__file__).parent / "fixtures/macos-exit-observation-k195/controls-01"


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
    deadline: str, timed_out: bool | None, intentional: bool = False, **extra: Any
) -> dict[str, Any]:
    return {
        "deadline": deadline,
        "timed_out": timed_out,
        "intentional": intentional,
        "receipt": True,
        "receipt_failures": [],
        **extra,
    }


def test_cause_names_candidate_one_only_for_starved_exits() -> None:
    base = {"issue-sleep": _entry("command-alive", True, True, term="term-success", alive=[])}
    none = checker.cause({**base, "after-09-table": _entry("observed", False)}, [])
    assert none["stall"] == "not-reproduced" and none["statement"].startswith("unproved")
    starved = checker.cause(
        {**base, "after-09-table": _entry("exited-unobserved", True, starved=True)}, []
    )
    assert starved["statement"].startswith("candidate 1")
    unstarved = checker.cause(
        {**base, "after-09-table": _entry("exited-unobserved", True, starved=False)}, []
    )
    assert unstarved["statement"].startswith("unproved")
    alive = checker.cause(
        {
            **base,
            "after-09-table": _entry("command-alive", True, alive=[{"pid": 5, "path": "/bin/ps"}]),
        },
        [],
    )
    assert "/bin/ps" in alive["statement"]
    unknown = checker.cause({**base, "after-09-table": _entry("unknown", False)}, [])
    assert unknown["evidence"] == "incomplete" and unknown["stall"] == "unknown"


def test_damaged_evidence_is_never_complete() -> None:
    base = {"issue-sleep": _entry("command-alive", True, True, term="term-success", alive=[])}
    quiet = {**base, "after-09-table": _entry("observed", False)}
    assert checker.cause(quiet, [])["evidence"] == "complete"
    damaged = {
        "missing receipt": {**base, "after-09-table": _entry("observed", None, receipt=False)},
        "receipt failure": {
            **base,
            "after-09-table": _entry("observed", False, receipt_failures=["exit 1"]),
        },
        "intentional ran": {
            "issue-sleep": _entry("observed", False, True),
            "after-09-table": _entry("observed", False),
        },
    }
    for label, found in damaged.items():
        verdict = checker.cause(found, [])
        assert verdict["evidence"] == "incomplete", label
        assert verdict["stall"] == "unknown", label
    trace = checker.cause(quiet, ["seq is not contiguous"])
    assert trace["evidence"] == "incomplete" and trace["stall"] == "unknown"
    stalled = checker.cause(
        {**base, "after-09-table": _entry("exited-unobserved", True, starved=True)},
        ["1 trace line(s) did not parse"],
    )
    assert stalled["stall"] == "reproduced"
    assert stalled["statement"] == "unproved: the evidence is incomplete"


def _record(seq: int, t: int, event: str, call: int = 1, agent: int = 7, **fields: Any) -> Any:
    return {"seq": seq, "t_ns": t, "agent": agent, "call": call, "event": event, **fields}


def test_integrity_rejects_damaged_traces() -> None:
    good = [_record(1, 10, "trace-open", 0), _record(2, 20, "begin")]
    assert checker.integrity(good, 0, "7") == []
    assert checker.integrity(good, 1, "7")
    assert checker.integrity(good, 0, "8")
    assert checker.integrity([good[0], _record(3, 20, "begin")], 0, "7")
    assert checker.integrity([good[0], _record(2, 20, "trace-loop-error")], 0, "7")
    assert checker.integrity([*good, _record(3, 30, "trace-open", 0)], 0, "7")


def _timed_out(exit_t: int, status: int | None) -> list[Any]:
    """One timed-out call: 2 s budget, wait from 1 s, the agent signal at 3.1 s."""
    ms = 1_000_000
    records = [
        _record(1, 0, "begin", timeout=2),
        _record(2, 1 * ms, "spawned", pid=40),
        _record(3, 1 * ms, "watch", pid=40, parent=0),
        _record(4, 1 * ms, "wait-enter", phase="deadline"),
        _record(5, 1 * ms, "waiter-start", phase="deadline"),
        _record(6, 3_005 * ms, "wait-result", phase="deadline", result="timed-out"),
        _record(7, 3_100 * ms, "terminate-sent", pid=40, result=0),
    ]
    if status is not None:
        records.append(_record(8, exit_t * ms, "kernel-exit", pid=40, status=status))
    return records


def test_command_alive_needs_a_signal_death_after_the_agent_signal() -> None:
    assert checker.classify(_timed_out(3_101, 15))["deadline"] == "command-alive"
    assert checker.classify(_timed_out(3_101, 9))["deadline"] == "command-alive"
    assert checker.classify(_timed_out(3_099, 15))["deadline"] == "uncertain"
    for label, records in {
        "clean exit after the deadline": _timed_out(3_101, 0),
        "no exit recorded": _timed_out(0, None),
        "exit between due time and wait result": _timed_out(3_002, 0),
    }.items():
        verdict = checker.classify(records)
        assert verdict["deadline"] == "uncertain", label
        assert verdict["alive"] == [], label
    early = checker.classify(_timed_out(2_000, 0))
    assert early["deadline"] == "exited-unobserved" and early["starved"] is False


def test_starved_needs_sequence_and_clock_to_agree() -> None:
    ms = 1_000_000
    records = _timed_out(2_000, 0)
    late = [r for r in records if r["event"] != "waiter-start"]
    late.append(_record(9, 3_006 * ms, "waiter-start", phase="deadline"))
    assert checker.classify(late)["starved"] is True
    clash = [r for r in records if r["event"] != "waiter-start"]
    clash.append(_record(9, 3_000 * ms, "waiter-start", phase="deadline"))
    assert checker.classify(clash)["starved"] is None
