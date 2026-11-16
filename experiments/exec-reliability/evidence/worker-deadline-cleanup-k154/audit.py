"""Join the k154 control runs and manual removals into one resource audit.

Each entry names one child that a control run created, and how it was removed.
The audit reads only retained files and writes resource-audit.json.
"""

import hashlib
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parent


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fixture_kills(receipts: Path) -> dict[int, str]:
    """Map each PID that a fixture removed to the receipt that records it."""
    found: dict[int, str] = {}
    if not receipts.exists():
        return found
    for path in sorted(receipts.glob("*.json")):
        text = path.read_text()
        if '"fixture_kill"' not in text:
            continue
        data = json.loads(text)
        audit = data.get("audit", data)
        kill = audit.get("fixture_kill") or data.get("fixture_kill")
        if not kill:
            # A null record means the child exited before the fixture checked it.
            continue
        pid = kill.get("pid", audit.get("pid"))
        if isinstance(pid, int):
            found[pid] = path.name
    return found


def main() -> None:
    removals: dict[str, dict[int, dict[str, Any]]] = {}
    for path in sorted((ROOT / "removals").glob("*.json")):
        data = json.loads(path.read_text())
        run = Path(data["source"]).parent.name
        removals[run] = {entry["pid"]: entry for entry in data["entries"]}
    runs: list[dict[str, Any]] = []
    for control in sorted((ROOT / "runs").glob("*/control.json")):
        data = json.loads(control.read_text())
        run = control.parent.name
        kills = fixture_kills(control.parent / "receipts")
        manual = removals.get(run, {})
        children: list[dict[str, Any]] = []
        for entry in data["resources"]:
            pid = entry["pid"]
            child: dict[str, Any] = {
                "pid": pid,
                "sources": entry.get("sources", ["deadline audit receipt"]),
                "state_after_tests": entry.get("state_after_tests"),
                "owner_record_after_tests": entry["owner_record_after_tests"],
            }
            if pid in manual:
                child["removal"] = manual[pid]["removal"]
                child["removal_record"] = f"removals/{run}.json"
            elif pid in kills:
                child["removal"] = "fixture SIGKILL to exact unreaped child, then reaped"
                child["removal_record"] = f"runs/{run}/receipts/{kills[pid]}"
            elif entry.get("state_after_tests") == "reaped before audit":
                child["removal"] = "runtime signal or child exit, reaped by the runtime"
            else:
                child["removal"] = entry.get("removal")
            children.append(child)
        runs.append(
            {
                "run": run,
                "control_sha256": sha256(control),
                "stall": data["stall"],
                "tests": len(data["tests"]),
                "verdict": data["verdict"],
                "reasons": data["reasons"],
                "children": len(children),
                "fixture_removals": sum(1 for c in children if c["pid"] in kills),
                "manual_removals": sum(1 for c in children if c["pid"] in manual),
                "children_of_control_after_cleanup": data["children_after_cleanup"],
                "resources": children,
            }
        )
    unremoved = [
        f"{run['run']}:{child['pid']}"
        for run in runs
        for child in run["resources"]
        if child["removal"] is None
    ]
    audit = {
        "leaf": "worker-deadline-cleanup-k154",
        "method": "exact PID only; no group, pattern or name signal",
        "runs": runs,
        "unremoved": unremoved,
    }
    (ROOT / "resource-audit.json").write_text(json.dumps(audit, indent=2) + "\n")
    print(json.dumps({r["run"]: [r["verdict"], r["children"]] for r in runs}, indent=1))
    print("unremoved", unremoved)


if __name__ == "__main__":
    main()
