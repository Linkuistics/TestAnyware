"""Judge one traced k195 run: the route, each exec ordering and the cleanup.

Usage: exit_k195_check.py RUN

RUN is the controller run directory `OUT/<run_id>`. The checker reads the plan
digest from `OUT/inventory.json` and the guest trace from `OUT/downloads`.

The route is `traced` only when the verify exec shows that its own parent, a PID
other than the agent before the install, opened the trace. Otherwise the route is
`unavailable` and nothing about the guest ordering is judged.

For each exec call the checker finds its trace call by order and command. A kernel
record time is when the trace thread received the event, so it is an upper bound on
the exit. The deadline is due at the wait-enter mark plus the requested timeout.
The checker classifies the deadline phase as follows:

- `observed`: the agent saw the exit before the deadline.
- `exited-unobserved`: the shell exit is stamped before the deadline was due. This
  is the k3 candidate. `starved` is true when the inner wait block started after
  the deadline result in both the sequence and the clock. It is null when the two
  orders disagree.
- `command-alive`: the shell died from SIGTERM or SIGKILL after the agent sent TERM.
  `alive` names each watched process with the same proof, by its exec path.
- `uncertain`: the shell exit is absent, or its stamp falls after the due time
  without that proof. `unresolved` lists the processes in that state.

`coverage` lists each sign that the watched descendants are only a lower bound.

A timed-out call also gets the shape of its TERM phase: `term-success` (the 30 s
shape) or `term-timeout-kill` (the 33 s shape).

A missing or unparsable record never proves an absence. Such a call is `unknown`.
The evidence is complete only when every receipt is present and valid, each
intentional call timed out, every trace record comes from the verified agent in an
unbroken sequence, and every trace call matches the workload. The checker exits 0
only for a traced route with complete evidence, confirmed cleanup and no disposal
residual.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any, cast

from exit_k195 import STAGED, VERIFY, workload_calls
from timeout_k4 import OWNED_CLEANUP
from timeout_k4_check import cleanup_state, receipt_failures

INTENTIONAL = ("issue-sleep", "issue-child")
_PLACEHOLDER = "/vm-placeholder"
WORKLOAD = tuple(call.name for call in workload_calls(_PLACEHOLDER, _PLACEHOLDER))
COMMANDS = {
    call.name: call.argv[-1]
    for call in workload_calls(_PLACEHOLDER, _PLACEHOLDER)
    if call.argv[0] != "/bin/sh" and call.argv[2] == "exec"
}
HEALTH_CALLS = tuple(
    call.name for call in workload_calls(_PLACEHOLDER, _PLACEHOLDER) if call.argv[0] == "/bin/sh"
)
COMMAND_PREFIX = 160  # the seam keeps the first 160 characters of each command


def _read_json(path: Path) -> Any:
    try:
        return json.loads(path.read_bytes())
    except (OSError, ValueError):
        return None


def _dict(value: object) -> dict[str, Any]:
    return cast(dict[str, Any], value) if isinstance(value, dict) else {}


def _receipt(run: Path, name: str) -> dict[str, Any] | None:
    value = _read_json(run / f"{name}.stdout")
    return cast(dict[str, Any], value) if isinstance(value, dict) else None


def _fields(stdout: str) -> dict[str, str]:
    pairs = (line.split("=", 1) for line in stdout.splitlines() if "=" in line)
    return {key: value for key, value in pairs}


def load_trace(path: Path) -> tuple[list[dict[str, Any]], int]:
    """Return the parsed records and the number of lines that did not parse."""
    if not path.is_file():
        return [], 0
    records: list[dict[str, Any]] = []
    bad = 0
    for line in path.read_text(encoding="utf-8", errors="replace").splitlines():
        try:
            records.append(_dict(json.loads(line)))
        except ValueError:
            bad += 1
    return records, bad


def route(run: Path, digest: str | None) -> dict[str, Any]:
    """Judge whether the traced agent answered the workload."""
    failures: list[str] = []
    install = _receipt(run, "install-agent")
    verify = _receipt(run, "verify-agent")
    failures += [f"install: {f}" for f in receipt_failures(install, None)]
    failures += [f"verify: {f}" for f in receipt_failures(verify, VERIFY)]
    before = _fields(str((install or {}).get("stdout") or "")).get("old")
    after = _fields(str((verify or {}).get("stdout") or ""))
    first = _dict(_read_json_text(after.get("first", "")))
    if (install or {}).get("exit_code") != 0 or "restart-scheduled" not in str(
        (install or {}).get("stdout")
    ):
        failures.append("install exec did not schedule the restart")
    if STAGED not in str((install or {}).get("command")):
        failures.append("install exec does not name the staged agent")
    if digest is None or after.get("sha") != digest:
        failures.append(f"installed digest {after.get('sha')!r} is not {digest!r}")
    if before is None or after.get("agent") is None or before == after.get("agent"):
        failures.append(f"agent PID before {before!r} and after {after.get('agent')!r}")
    if first.get("event") != "trace-open" or str(first.get("agent")) != after.get("agent"):
        failures.append(f"first trace record {first!r} is not trace-open by the answering agent")
    return {
        "route": "traced" if not failures else "unavailable",
        "agent_before": before,
        "agent_after": after.get("agent"),
        "agent_started": after.get("started"),
        "failures": failures,
    }


def _read_json_text(text: str) -> Any:
    try:
        return json.loads(text)
    except ValueError:
        return None


def _calls(records: list[dict[str, Any]]) -> dict[int, list[dict[str, Any]]]:
    calls: dict[int, list[dict[str, Any]]] = {}
    for record in records:
        number = record.get("call")
        if isinstance(number, int) and number > 0:
            calls.setdefault(number, []).append(record)
    for group in calls.values():
        group.sort(key=lambda r: int(r.get("seq", 0)))
    return calls


def _first(group: list[dict[str, Any]], event: str, phase: str | None = None) -> dict[str, Any]:
    for record in group:
        if record.get("event") == event and (phase is None or record.get("phase") == phase):
            return record
    return {}


def integrity(records: list[dict[str, Any]], bad: int, agent: str | None) -> list[str]:
    """List the defects that stop the trace from counting as complete evidence."""
    failures: list[str] = []
    if bad:
        failures.append(f"{bad} trace lines did not parse")
    errors = sum(r.get("event") == "trace-loop-error" for r in records)
    if errors:
        failures.append(f"{errors} trace loop errors")
    opens = sum(r.get("event") == "trace-open" for r in records)
    if opens != 1:
        failures.append(f"{opens} trace-open records, not 1")
    foreign = sorted({str(r.get("agent")) for r in records} - {str(agent)})
    if foreign:
        failures.append(f"records from agent PIDs {foreign}, not the verified agent {agent!r}")
    seqs = [r.get("seq") for r in records]
    ints = [s for s in seqs if isinstance(s, int)]
    if len(ints) != len(seqs) or sorted(ints) != list(range(1, len(seqs) + 1)):
        failures.append("trace sequence is not unique and contiguous from 1")
    if any(not isinstance(r.get("t_ns"), int) for r in records):
        failures.append("trace record without an integer t_ns")
    return failures


def _signal(status: object) -> int | None:
    """Return the signal that ended a process, from its wait status."""
    if not isinstance(status, int) or isinstance(status, bool):
        return None
    number = status & 0x7F
    return number if number not in (0, 0x7F) else None


def _follows(record: dict[str, Any], mark: dict[str, Any]) -> bool:
    """True when the record comes after the mark in both the sequence and the clock."""
    keys = ("seq", "t_ns")
    if not mark or any(not isinstance(r.get(k), int) for r in (record, mark) for k in keys):
        return False
    return record["seq"] > mark["seq"] and record["t_ns"] > mark["t_ns"]


def _coverage(group: list[dict[str, Any]]) -> list[str]:
    """Name each sign that the watched descendant set is only a lower bound."""
    notes = [
        str(r.get("event"))
        for r in group
        if r.get("event") in ("watch-missed", "adopt-truncated", "fork-unadopted")
    ]
    forks: dict[object, int] = {}
    adopted: dict[object, int] = {}
    for record in group:
        if record.get("event") == "kernel-fork":
            forks[record.get("pid")] = forks.get(record.get("pid"), 0) + 1
        elif record.get("event") == "watch" and record.get("parent"):
            adopted[record.get("parent")] = adopted.get(record.get("parent"), 0) + 1
    notes += [
        f"pid {pid} forked {count} times, {adopted.get(pid, 0)} children watched"
        for pid, count in forks.items()
        if count > adopted.get(pid, 0)
    ]
    return notes


def classify(group: list[dict[str, Any]]) -> dict[str, Any]:
    """Classify the deadline and TERM phases of one traced exec call.

    A kernel record time is when the trace thread received the event, so it only
    bounds the exit from above. The deadline is due at the wait-enter mark plus the
    requested timeout. An exit stamped before that time proves the exit. A process
    proves it was alive at the deadline only by dying from SIGTERM or SIGKILL after
    the agent sent TERM. This assumes the agent is the only sender of those signals.
    """
    spawned = _first(group, "spawned")
    shell = spawned.get("pid")
    begin = _first(group, "begin")
    enter = _first(group, "wait-enter", "deadline")
    deadline = _first(group, "wait-result", "deadline")
    timeout = begin.get("timeout")
    if (
        not isinstance(shell, int)
        or not deadline
        or not enter
        or not isinstance(timeout, (int, float))
    ):
        return {"deadline": "unknown", "reason": "spawned, begin, wait-enter or deadline absent"}
    t0 = int(begin["t_ns"])
    due = int(enter["t_ns"]) + int(timeout * 1_000_000_000)

    def ms(record: dict[str, Any]) -> float | None:
        return round((int(record["t_ns"]) - t0) / 1e6, 1) if record else None

    exits = {r.get("pid"): r for r in group if r.get("event") == "kernel-exit"}
    execs = {r.get("pid"): r.get("path") for r in group if r.get("event") == "kernel-exec"}
    watched = [r.get("pid") for r in group if r.get("event") == "watch"]
    shell_exit = exits.get(shell, {})
    result: dict[str, Any] = {
        "shell": shell,
        "result": deadline.get("result"),
        "deadline_due_ms": round((due - t0) / 1e6, 1),
        "deadline_ms": ms(deadline),
        "shell_exit_ms": ms(shell_exit),
        "threads_at_deadline": deadline.get("threads"),
        "coverage": _coverage(group),
    }
    if deadline.get("result") == "success":
        result["deadline"] = "observed"
    elif deadline.get("result") != "timed-out":
        result["deadline"] = "unknown"
    elif shell_exit and int(shell_exit["t_ns"]) < due:
        waiter = _first(group, "waiter-start", "deadline")
        result["deadline"] = "exited-unobserved"
        result["waiter_start_ms"] = ms(waiter)
        if not waiter or _follows(waiter, deadline):
            result["starved"] = True
        elif _follows(deadline, waiter):
            result["starved"] = False
        else:
            result["starved"] = None
    else:
        term = _first(group, "terminate-sent")

        def proven(pid: object) -> bool:
            record = exits.get(pid)
            return (
                bool(record) and _signal(record.get("status")) in (9, 15) and _follows(record, term)
            )

        result["alive"] = [{"pid": p, "path": execs.get(p)} for p in watched if proven(p)]
        result["unresolved"] = [
            {"pid": p, "path": execs.get(p), "exit_ms": ms(exits.get(p, {}))}
            for p in watched
            if not proven(p) and (p not in exits or int(exits[p]["t_ns"]) >= due)
        ]
        if proven(shell):
            result["deadline"] = "command-alive"
        else:
            result["deadline"] = "uncertain"
            result["reason"] = "the shell exit is absent or not proved later than the deadline"
    if deadline.get("result") == "timed-out":
        term = _first(group, "wait-result", "terminate")
        killed = _first(group, "kill-sent")
        if term.get("result") == "success" and not killed:
            result["term"] = "term-success"
        elif term.get("result") == "timed-out" and killed:
            result["term"] = "term-timeout-kill"
        else:
            result["term"] = "unknown"
        result["term_ms"] = ms(_first(group, "terminate-sent"))
        result["return_ms"] = ms(_first(group, "return"))
    return result


def orderings(
    run: Path, records: list[dict[str, Any]]
) -> tuple[dict[str, dict[str, Any]], list[str]]:
    """Match each workload exec to its trace call by order and command.

    Return the matches and the trace calls that match neither a workload exec nor the
    verify exec that opens the run. An unmatched call means the workload and the trace
    disagree, so it makes the evidence incomplete.
    """
    calls = _calls(records)
    begins = sorted(
        (number, str(_first(group, "begin").get("command", ""))) for number, group in calls.items()
    )
    found: dict[str, dict[str, Any]] = {}
    matched: set[int] = set()
    cursor = 0
    for name in WORKLOAD:
        if name not in COMMANDS:
            continue
        want = COMMANDS[name][:COMMAND_PREFIX]
        match = next((i for i in range(cursor, len(begins)) if begins[i][1] == want), None)
        receipt = _receipt(run, name)
        entry: dict[str, Any] = {
            "intentional": name in INTENTIONAL,
            "receipt": receipt is not None,
            "timed_out": (receipt or {}).get("timed_out"),
            "exit_code": (receipt or {}).get("exit_code"),
            "receipt_failures": receipt_failures(receipt, COMMANDS[name]),
        }
        if match is None:
            entry["deadline"] = "unknown"
            entry["reason"] = "no trace call with this command"
        else:
            matched.add(match)
            cursor = match + 1
            entry["trace_call"] = begins[match][0]
            entry.update(classify(calls[begins[match][0]]))
        found[name] = entry
    verify = VERIFY[:COMMAND_PREFIX]
    extra = [
        f"trace call {number} matches no workload exec"
        for i, (number, command) in enumerate(begins)
        if i not in matched and not (i == 0 and command == verify)
    ]
    return found, extra


def cause(found: dict[str, dict[str, Any]], failures: list[str]) -> dict[str, Any]:
    """State which candidate the unintended timeouts establish, if any."""
    failures = list(failures)
    for name, entry in found.items():
        if not entry.get("receipt"):
            failures.append(f"{name}: receipt absent")
        failures += [f"{name}: {f}" for f in entry["receipt_failures"]]
        if not isinstance(entry.get("timed_out"), bool):
            failures.append(f"{name}: timeout outcome unknown")
        elif entry["intentional"] and not entry["timed_out"]:
            failures.append(f"{name}: the intentional timeout did not time out")
        if entry.get("deadline") == "unknown":
            failures.append(f"{name}: deadline ordering unknown")
    evidence = "complete" if not failures else "incomplete"
    unintended = {n: e for n, e in found.items() if not e["intentional"] and e["timed_out"]}
    kinds = {e.get("deadline") for e in unintended.values()}
    if not unintended:
        stall = "not-reproduced" if evidence == "complete" else "unknown"
        statement = "unproved: no unintended timeout recurred"
    elif evidence != "complete":
        stall = "reproduced"
        statement = "unproved: the evidence is incomplete"
    elif kinds == {"exited-unobserved"} and all(e.get("starved") for e in unintended.values()):
        stall = "reproduced"
        statement = "candidate 1: the shell exited and the starved inner wait block missed it"
    elif kinds == {"command-alive"}:
        stall = "reproduced"
        paths = sorted({str(a.get("path")) for e in unintended.values() for a in e["alive"]})
        statement = f"the guest command was still running at the deadline: {paths}"
    else:
        stall = "reproduced"
        statement = f"unproved: mixed, uncertain or unknown orderings {sorted(map(str, kinds))}"
    shapes = {n: e.get("term") for n, e in found.items() if e["timed_out"]}
    lower = sorted(n for n, e in found.items() if e.get("coverage"))
    return {
        "evidence": evidence,
        "evidence_failures": failures,
        "stall": stall,
        "statement": statement,
        "term_shapes": shapes,
        "coverage_lower_bound": lower,
    }


def health(run: Path, name: str) -> dict[str, Any]:
    """Read one wrapped health call: the CLI exit and the accessibility it reports."""
    try:
        text = (run / f"{name}.stdout").read_text(encoding="utf-8", errors="replace")
    except OSError:
        return {"answered": False}
    lines = text.splitlines()
    code = lines[-1].removeprefix("health-exit=") if lines else None
    body = _dict(_read_json_text("\n".join(lines[:-1])))
    return {
        "answered": bool(body),
        "cli_exit": code,
        "accessibility": body.get("accessibility_status"),
    }


def _disposal(run: Path) -> list[str]:
    summary = _dict(_read_json(run.parent / "summary.json"))
    entries = [_dict(e) for e in cast(list[object], summary.get("ledger") or [])]
    failures: list[str] = []
    if not entries or any(e.get("state") != "disposed" for e in entries):
        failures.append(f"ledger not disposed: {[e.get('state') for e in entries]}")
    if summary.get("residuals") != []:
        failures.append(f"residuals {summary.get('residuals')!r}")
    return failures


def check(run: Path) -> dict[str, Any]:
    inventory = _dict(_read_json(run.parent / "inventory.json"))
    digest = inventory.get("agent_sha256")
    judged = route(run, digest if isinstance(digest, str) else None)
    records, bad = load_trace(run.parent / "downloads" / "trace.jsonl")
    result: dict[str, Any] = {
        "checker": "k199-1",
        **judged,
        "trace_records": len(records),
        "trace_unparsed": bad,
        "trace_loop_errors": sum(r.get("event") == "trace-loop-error" for r in records),
    }
    if judged["route"] == "traced" and records:
        found, extra = orderings(run, records)
        result["calls"] = found
        result.update(cause(found, integrity(records, bad, judged["agent_after"]) + extra))
    else:
        result.update(evidence="absent", stall="unknown", statement="condition unavailable")
    result["health"] = {name: health(run, name) for name in HEALTH_CALLS}
    cleanup = _dict(_read_json(run / "cleanup.json"))
    result["cleanup"] = cleanup_state(cleanup or None, _receipt(run, "cleanup"))
    result["owned_cleanup_command_matches"] = (_receipt(run, "cleanup") or {}).get(
        "command"
    ) == OWNED_CLEANUP
    result["disposal_failures"] = _disposal(run)
    return result


def main() -> int:
    if len(sys.argv) != 2:
        raise SystemExit("usage: exit_k195_check.py RUN")
    result = check(Path(sys.argv[1]))
    print(json.dumps(result, indent=2))
    accepted = (
        result["route"] == "traced"
        and result.get("evidence") == "complete"
        and result["cleanup"].get("state") == "confirmed"
        and result["owned_cleanup_command_matches"]
        and not result["disposal_failures"]
    )
    return 0 if accepted else 1


if __name__ == "__main__":
    sys.exit(main())
