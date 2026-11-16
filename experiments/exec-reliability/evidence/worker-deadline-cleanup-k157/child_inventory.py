"""List each child PID that the k154 control runs record, with its removal.

This script reads the retained k154 evidence and does not change it. It
repairs two k156 review findings against k154 audit.py:

- R1: the control inventory found children only through deadline-audit
  receipts, stalled signals, owner records and child rows after the tests. This
  script also reads each PID that a fixture or runtime receipt names.
- R2: k154 audit.py credited the runtime with each child that was "reaped
  before audit". This script names a reaper or a signaller only when a receipt
  or a control record holds that fact. Otherwise the actor stays "unknown".

Each PID below is a direct child of the control process. Pytest runs inside
the control, so the runtime, the fixtures and the control fork from one
process. Helper and workload identities inside a worker are grandchildren, and
this script does not list them.
"""

from __future__ import annotations

import hashlib
import json
import sys
from collections import Counter
from pathlib import Path
from typing import Any, cast

HERE = Path(__file__).resolve().parent
K154 = HERE.parent / "worker-deadline-cleanup-k154"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def obj(value: object) -> dict[str, Any] | None:
    """Return a JSON object, or None for any other value."""
    return cast(dict[str, Any], value) if isinstance(value, dict) else None


def pid_of(value: object) -> int | None:
    record = obj(value)
    pid = record.get("pid") if record else None
    return pid if isinstance(pid, int) else None


def receipt_facts(name: str, data: dict[str, Any]) -> list[tuple[int, dict[str, Any]]]:
    """Return (pid, fact) pairs from one receipt. Each fact names its field."""
    facts: list[tuple[int, dict[str, Any]]] = []

    def owner(field: str, value: object) -> None:
        # A runtime owner record. reaped True means the runtime's waitpid
        # collected the exit. Its signals list holds the runtime's own signals.
        record, pid = obj(value), pid_of(value)
        if record is None or pid is None:
            return
        fact: dict[str, Any] = {"field": field}
        if record.get("reaped") is True:
            fact["reaper"] = "runtime"
        if record.get("signals"):
            fact["signaller"] = "runtime"
        facts.append((pid, fact))

    def audit(field: str, value: object, fallback: int | None = None) -> None:
        # A fixture audit record. A wait status means the fixture's own waitpid
        # collected the exit. reaped True without a status means that the audit
        # saw ECHILD, so another caller reaped first. The shutdown tests omit
        # the PID from the audit (test_shutdown.py:169), so the runtime owner of
        # the same receipt supplies it.
        record = obj(value)
        pid = pid_of(value) or fallback
        if record is None or pid is None:
            return
        fact: dict[str, Any] = {"field": field, "source": record.get("source")}
        if record.get("reaped") is True and "status" in record:
            fact["reaper"] = "fixture"
        elif record.get("reaped") is True:
            fact["absent"] = "audit saw the child already reaped"
        if record.get("fixture_kill"):
            fact["signaller"] = "fixture"
        facts.append((pid, fact))

    def kill(field: str, value: object) -> None:
        # A fixture kill record with a wait status: the fixture signalled the
        # exact child and collected its exit.
        record, pid = obj(value), pid_of(value)
        if record is None or pid is None:
            return
        fact: dict[str, Any] = {"field": field, "signaller": "fixture"}
        if "status" in record:
            fact["reaper"] = "fixture"
        facts.append((pid, fact))

    runtime = obj(data.get("runtime"))
    runtime_pid = None
    if runtime is not None:
        record = runtime.get("owner", runtime)
        runtime_pid = pid_of(record)
        owner("runtime.owner" if "owner" in runtime else "runtime", record)
    owner("owner", data.get("owner"))
    owner("worker", data.get("worker"))
    audit("audit", data.get("audit"), runtime_pid)
    audit_record = obj(data.get("audit"))
    if audit_record is not None:
        kill("audit.fixture_kill", audit_record.get("fixture_kill"))
    kill("fixture_kill", data.get("fixture_kill"))
    top = pid_of(data)
    if "source" in data:
        audit("receipt", data)
    elif top is not None and "worker" not in data:
        # In a worker-* receipt the top-level pid is the helper, whose parent is
        # the worker (worker.helper_identity.ppid), so it is a grandchild.
        facts.append((top, {"field": "pid"}))
    identity = pid_of(data.get("identity"))
    if identity is not None:
        facts.append((identity, {"field": "identity"}))
    calls = cast(list[object], data.get("calls") or [])
    for call in calls:
        if (pid := pid_of(call)) is not None:
            facts.append((pid, {"field": "calls"}))
    return [(pid, {"receipt": name, **fact}) for pid, fact in facts]


def control_facts(resource: dict[str, Any]) -> dict[str, Any]:
    fact: dict[str, Any] = {
        "receipt": "control.json",
        "field": "resources",
        "state_after_tests": resource.get("state_after_tests"),
    }
    if resource.get("removal") == "SIGKILL to exact PID by control, then reaped":
        fact.update(signaller="control", reaper="control")
    elif resource.get("state_after_tests") == "reaped before audit":
        fact["absent"] = "control waitpid saw the child already reaped"
    return fact


def actor(facts: list[dict[str, Any]], key: str) -> str:
    named = sorted({fact[key] for fact in facts if key in fact})
    if not named:
        return "unknown"
    return named[0] if len(named) == 1 else "conflict: " + ", ".join(named)


def audit_run(run: Path, manual: dict[int, dict[str, Any]]) -> dict[str, Any]:
    control = json.loads((run / "control.json").read_text())
    receipts = sorted((run / "receipts").glob("*.json"))
    facts: dict[int, list[dict[str, Any]]] = {}
    checkers: set[int] = set()
    for path in receipts:
        data = json.loads(path.read_text())
        if isinstance(data.get("checker_pid"), int):
            checkers.add(data["checker_pid"])
        for pid, fact in receipt_facts(path.name, data):
            facts.setdefault(pid, []).append(fact)
    control_pids: set[int] = set()
    for resource in control["resources"]:
        control_pids.add(resource["pid"])
        facts.setdefault(resource["pid"], []).append(control_facts(resource))
    for pid, entry in manual.items():
        facts.setdefault(pid, []).append(
            {
                "receipt": "removals",
                "field": "entries",
                "signaller": "manual exact-PID SIGKILL",
                "reaper": "launchd after the control exited",
                "removal": entry.get("removal"),
            }
        )
    children: list[dict[str, Any]] = []
    for pid in sorted(facts):
        rows = facts[pid]
        children.append(
            {
                "pid": pid,
                "in_control_inventory": pid in control_pids,
                "signaller": actor(rows, "signaller"),
                "reaper": actor(rows, "reaper"),
                "absence_observed": sorted({row["absent"] for row in rows if "absent" in row}),
                "facts": rows,
            }
        )
    reapers = Counter[str](str(child["reaper"]) for child in children)
    signallers = Counter[str](str(child["signaller"]) for child in children)
    return {
        "run": run.name,
        "control_json_sha256": sha256(run / "control.json"),
        "receipts": len(receipts),
        "receipts_sha256": hashlib.sha256(
            b"".join(sha256(path).encode() + b"\n" for path in receipts)
        ).hexdigest(),
        "control_verdict": control["verdict"],
        "control_reasons": control["reasons"],
        "checker_pids": sorted(checkers),
        "checker_is_listed_child": bool(checkers & set(facts)),
        "children_listed": len(children),
        "children_in_control_inventory": len(control_pids),
        "children_missing_from_control_inventory": len(set(facts) - control_pids),
        "children_of_control_after_cleanup": control["children_after_cleanup"],
        "reapers": dict(sorted(reapers.items())),
        "signallers": dict(sorted(signallers.items())),
        "children": children,
    }


def main() -> int:
    manual: dict[str, dict[int, dict[str, Any]]] = {}
    for path in sorted((K154 / "removals").glob("*.json")):
        data = json.loads(path.read_text())
        manual[Path(data["source"]).parent.name] = {
            entry["pid"]: entry for entry in data["entries"]
        }
    runs = [
        audit_run(run, manual.get(run.name, {}))
        for run in sorted((K154 / "runs").glob("*"))
        if (run / "control.json").exists()
    ]
    reapers = Counter[str]()
    signallers = Counter[str]()
    for run in runs:
        reapers.update(run["reapers"])
        signallers.update(run["signallers"])
    totals: dict[str, Any] = {
        "children_listed": sum(run["children_listed"] for run in runs),
        "children_missing_from_control_inventory": sum(
            run["children_missing_from_control_inventory"] for run in runs
        ),
        "reapers": dict(sorted(reapers.items())),
        "signallers": dict(sorted(signallers.items())),
        "runs_with_children_after_cleanup": [
            run["run"] for run in runs if run["children_of_control_after_cleanup"]
        ],
    }
    result: dict[str, Any] = {
        "leaf": "worker-deadline-cleanup-k157",
        "script_sha256": sha256(Path(__file__)),
        "k154_audit_py_sha256": sha256(K154 / "audit.py"),
        "method": (
            "Read each receipt of each k154 control run. List each PID that a"
            " runtime owner, fixture audit, fixture kill, identity journal, call"
            " record or the control inventory names. Name a reaper or a signaller"
            " only from a recorded fact. Otherwise record unknown."
        ),
        "excluded": (
            "Helper and request identities inside a worker are grandchildren. The"
            " full-suite run has no control.json and is outside the k154 claim."
        ),
        "runs": runs,
        "totals": totals,
    }
    (HERE / "child-inventory.json").write_text(json.dumps(result, indent=2) + "\n")
    summary: dict[str, Any] = totals | {
        "per_run": {
            run["run"]: {
                k: run[k]
                for k in (
                    "children_listed",
                    "children_missing_from_control_inventory",
                    "reapers",
                    "checker_pids",
                    "checker_is_listed_child",
                )
            }
            for run in runs
        }
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
