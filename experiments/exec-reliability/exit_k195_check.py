"""Judge one traced k195 run: the route, each exec ordering and the cleanup.

Usage: exit_k195_check.py RUN

RUN is the controller run directory `OUT/<run_id>`. The checker reads the plan
digest from `OUT/inventory.json` and the guest trace from `OUT/downloads`.

The route is `traced` only when the verify exec shows that its own parent, a PID
other than the agent before the install, opened the trace. Otherwise the route is
`unavailable` and nothing about the guest ordering is judged.

For each exec call the checker finds its trace call by order and command. It then
classifies the deadline phase from the kernel clock:

- `observed`: the agent saw the exit before the deadline.
- `exited-unobserved`: the shell exited before the agent's deadline result. This
  is the k3 candidate. `starved` is true when the inner wait block started only
  after the deadline result.
- `command-alive`: the shell had not exited at the deadline. `alive` names the
  children with no exit before the deadline, by their exec path.

A timed-out call also gets the shape of its TERM phase: `term-success` (the 30 s
shape) or `term-timeout-kill` (the 33 s shape).

A missing or unparsable record never proves an absence. Such a call is `unknown`.
The checker exits 0 when the route is judged and disposal left no residual.
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


def classify(group: list[dict[str, Any]]) -> dict[str, Any]:
    """Classify the deadline and TERM phases of one traced exec call."""
    spawned = _first(group, "spawned")
    shell = spawned.get("pid")
    deadline = _first(group, "wait-result", "deadline")
    begin = _first(group, "begin")
    if not isinstance(shell, int) or not deadline or not begin:
        return {"deadline": "unknown", "reason": "spawned, begin or deadline record absent"}
    t0 = int(begin["t_ns"])

    def ms(record: dict[str, Any]) -> float | None:
        return round((int(record["t_ns"]) - t0) / 1e6, 1) if record else None

    exits = {r.get("pid"): r for r in group if r.get("event") == "kernel-exit"}
    execs = {r.get("pid"): r.get("path") for r in group if r.get("event") == "kernel-exec"}
    watched = [r.get("pid") for r in group if r.get("event") == "watch"]
    shell_exit = exits.get(shell, {})
    result: dict[str, Any] = {
        "shell": shell,
        "result": deadline.get("result"),
        "deadline_ms": ms(deadline),
        "shell_exit_ms": ms(shell_exit),
        "threads_at_deadline": deadline.get("threads"),
        "missed_watches": sum(r.get("event") == "watch-missed" for r in group),
    }
    if deadline.get("result") == "success":
        result["deadline"] = "observed"
    elif shell_exit and int(shell_exit["t_ns"]) < int(deadline["t_ns"]):
        waiter = _first(group, "waiter-start", "deadline")
        result["deadline"] = "exited-unobserved"
        result["waiter_start_ms"] = ms(waiter)
        result["starved"] = not waiter or int(waiter["seq"]) > int(deadline["seq"])
    elif deadline.get("result") == "timed-out":
        result["deadline"] = "command-alive"
        result["alive"] = [
            {"pid": pid, "path": execs.get(pid)}
            for pid in watched
            if pid not in exits or int(exits[pid]["t_ns"]) > int(deadline["t_ns"])
        ]
    else:
        result["deadline"] = "unknown"
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


def orderings(run: Path, records: list[dict[str, Any]]) -> dict[str, dict[str, Any]]:
    """Match each workload exec to its trace call by order and command."""
    calls = _calls(records)
    begins = sorted(
        (number, str(_first(group, "begin").get("command", ""))) for number, group in calls.items()
    )
    found: dict[str, dict[str, Any]] = {}
    cursor = 0
    for name in WORKLOAD:
        if name not in COMMANDS:
            continue
        want = COMMANDS[name][:COMMAND_PREFIX]
        match = next((i for i in range(cursor, len(begins)) if begins[i][1] == want), None)
        receipt = _receipt(run, name) or {}
        entry: dict[str, Any] = {
            "intentional": name in INTENTIONAL,
            "timed_out": receipt.get("timed_out"),
            "exit_code": receipt.get("exit_code"),
            "receipt_failures": receipt_failures(receipt or None, COMMANDS[name]),
        }
        if match is None:
            entry["deadline"] = "unknown"
            entry["reason"] = "no trace call with this command"
        else:
            cursor = match + 1
            entry["trace_call"] = begins[match][0]
            entry.update(classify(calls[begins[match][0]]))
        found[name] = entry
    return found


def cause(found: dict[str, dict[str, Any]]) -> dict[str, Any]:
    """State which candidate the unintended timeouts establish, if any."""
    unintended = {n: e for n, e in found.items() if not e["intentional"] and e["timed_out"]}
    kinds = {e.get("deadline") for e in unintended.values()}
    if any(e.get("deadline") == "unknown" for e in found.values()):
        evidence = "incomplete"
    else:
        evidence = "complete"
    if not unintended:
        stall = "not-reproduced" if evidence == "complete" else "unknown"
        statement = "unproved: no unintended timeout recurred"
    elif kinds == {"exited-unobserved"} and all(e.get("starved") for e in unintended.values()):
        stall = "reproduced"
        statement = "candidate 1: the shell exited and the starved inner wait block missed it"
    elif kinds == {"command-alive"}:
        stall = "reproduced"
        paths = sorted({str(a.get("path")) for e in unintended.values() for a in e["alive"]})
        statement = f"the guest command was still running at the deadline: {paths}"
    else:
        stall = "reproduced"
        statement = f"unproved: mixed or unknown orderings {sorted(map(str, kinds))}"
    shapes = {n: e.get("term") for n, e in found.items() if e["timed_out"]}
    return {"evidence": evidence, "stall": stall, "statement": statement, "term_shapes": shapes}


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
        "checker": "k195-1",
        **judged,
        "trace_records": len(records),
        "trace_unparsed": bad,
        "trace_loop_errors": sum(r.get("event") == "trace-loop-error" for r in records),
    }
    if judged["route"] == "traced" and records:
        found = orderings(run, records)
        result["calls"] = found
        result.update(cause(found))
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
    return (
        0 if result["route"] in {"traced", "unavailable"} and not result["disposal_failures"] else 1
    )


if __name__ == "__main__":
    sys.exit(main())
