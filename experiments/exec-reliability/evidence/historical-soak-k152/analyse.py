"""Classify each k152 soak run against the six open cases, item by item.

The input is one finished soak directory. The tool reads only retained files
and writes one analysis file. Each recurrence test restates the historical
symptom as a predicate on a timing record, a receipt or a JUnit failure.
"""

import argparse
import json
import statistics
from pathlib import Path
from typing import Any, cast

Record = dict[str, Any]
TERM_GRACE = 0.15  # Plan term_grace in test_controller.run().


def receipts(run: Path) -> list[tuple[Path, Record]]:
    found: list[tuple[Path, Record]] = []
    root = run / "receipts"
    for path in sorted(root.rglob("*.json")):
        try:
            data: object = json.loads(path.read_text())
        except (json.JSONDecodeError, UnicodeDecodeError):
            continue
        if isinstance(data, dict):
            found.append((path, cast(Record, data)))
    return found


def helper_misses(run: Path) -> list[Record]:
    """Every real /bin/ps group helper that the runtime killed in the run's receipts.

    Fixtures that inject a fake or slow ps use another argv[0], and worker
    receipts kill workers by design, so neither counts as the historical case.
    """
    misses: list[Record] = []
    for path, data in receipts(run):
        observations: list[Record] = data.get("group_observations") or []
        helpers: list[Record] = [item.get("helper") or {} for item in observations]
        for helper in helpers:
            argv: list[str] = helper.get("argv") or [""]
            if argv[0] == "/bin/ps" and helper.get("kill_monotonic") is not None:
                misses.append(
                    {
                        "receipt": str(path),
                        "argv": helper.get("argv"),
                        "error": helper.get("error"),
                        "created_to_kill": helper["kill_monotonic"] - helper["created_monotonic"],
                        "capture_window": helper["capture_deadline_monotonic"]
                        - helper["created_monotonic"],
                    }
                )
    return misses


def scope_findings(record: Record) -> list[str]:
    found: list[str] = []
    term, kill, leader = record.get("term"), record.get("kill"), record.get("leader_exit")
    if record["name"] == "live-exit" and record["reason"] == "interrupted" and kill is not None:
        found.append("interrupted-live-group-k145")
    if term is not None and leader is not None and leader - term > TERM_GRACE:
        found.append("exited-leader-grace-k132")
    printed = record.get("pid_printed")
    if term is not None and (printed is None or printed > term):
        found.append("empty-pid-k85")
    return found


def describe(values: list[float]) -> Record:
    if not values:
        return {"n": 0}
    ordered = sorted(values)
    return {
        "n": len(ordered),
        "min": ordered[0],
        "median": statistics.median(ordered),
        "p95": ordered[min(len(ordered) - 1, int(0.95 * len(ordered)))],
        "max": ordered[-1],
    }


def intervals(records: list[Record]) -> Record:
    spans: dict[str, list[float]] = {}

    def add(name: str, start: float | None, end: float | None) -> None:
        if start is not None and end is not None:
            spans.setdefault(name, []).append(end - start)

    for record in records:
        if record["kind"] == "scope":
            add("spawn_to_peer_start", record["spawn"], record["peer_start"])
            add("spawn_to_handler", record["spawn"], record["handler_installed"])
            add("spawn_to_child_ready", record["spawn"], record["child_ready"])
            add("child_ready_to_term", record["child_ready"], record["term"])
            add("term_to_leader_exit", record["term"], record["leader_exit"])
            for helper in record["helpers"]:
                add("ps_helper_start_to_end", helper["helper_start"], helper["helper_end"])
        else:
            add("dispatch_to_helper_start", record["dispatch"], record["helper_start"])
            add("helper_start_to_end_bound", record["helper_start"], record["helper_end_bound"])
            add("parent_wait", record["wait_started"], record["wait_left"])
    return {name: describe(values) for name, values in sorted(spans.items())}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("soak", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    summary: Record = json.loads((args.soak / "soak.json").read_text())
    per_run: list[Record] = []
    grouped: dict[str, list[Record]] = {}
    for run in summary["runs"]:
        path = Path(run["run"])
        checked: list[Record] = json.loads((path / "timeline.json").read_text())
        records = [entry["record"] for entry in checked]
        grouped.setdefault(f"{run['workload']}/{run['load']}", []).extend(records)
        cases: dict[str, list[str]] = {}
        for record in records:
            if record["kind"] == "scope":
                for case in scope_findings(record):
                    cases.setdefault(case, []).append(record["receipt"])
            elif record.get("outcome") == "expired":
                cases.setdefault("prebuffer-k106", []).append(record["witness"])
        failed: list[Record] = run["junit"].get("failed", [])
        for failure in failed:
            message = failure.get("message") or ""
            if "did not prebuffer" in message:
                cases.setdefault("prebuffer-k106", []).append(failure["nodeid"])
            if "invalid literal for int() with base 10: ''" in message:
                cases.setdefault("empty-pid-k85", []).append(failure["nodeid"])
        misses = helper_misses(path)
        if misses:
            cases["helper-capture-deadline"] = [miss["receipt"] for miss in misses]
        per_run.append(
            {
                "index": run["index"],
                "workload": run["workload"],
                "load": run["load"],
                "returncode": run["returncode"],
                "timed_out": run["timed_out"],
                "seconds": run["finished_monotonic"] - run["started_monotonic"],
                "loadavg_before": run["loadavg_before"],
                "loadavg_after": run["loadavg_after"],
                "tests": len(run["junit"].get("cases", [])),
                "failed": failed,
                "cases": cases,
                "helper_misses": misses,
                "timeline_problems": run["timeline_problems"],
            }
        )
    recurrences: dict[str, list[int]] = {}
    for item in per_run:
        for case in item["cases"]:
            recurrences.setdefault(case, []).append(item["index"])
    analysis: Record = {
        "soak": str(args.soak),
        "completed": summary["completed"],
        "scheduled": summary["scheduled"],
        "unstarted": len(summary["unstarted"]),
        "wall_seconds": summary["wall_seconds"],
        "subjects_unchanged": summary["subjects_unchanged"],
        "runs_by_condition": {
            key: sum(1 for item in per_run if f"{item['workload']}/{item['load']}" == key)
            for key in sorted(grouped)
        },
        "recurrences": recurrences,
        "failed_runs": [item for item in per_run if item["failed"] or item["returncode"] != 0],
        "intervals": {key: intervals(records) for key, records in sorted(grouped.items())},
        "runs": per_run,
    }
    args.out.write_text(json.dumps(analysis, indent=2) + "\n")
    print(
        json.dumps(
            {
                key: analysis[key]
                for key in ("completed", "unstarted", "wall_seconds", "recurrences")
            }
        )
    )


if __name__ == "__main__":
    main()
