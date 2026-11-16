"""Extract one timing record per k152 soak workload and check its fields.

A record joins three writers, each with its own file: the controller receipt,
the peer stamps (PEER_STAMPS, one JSON-lines file per process) and the
late-created fixture witnesses. All times are time.monotonic() on one host.
"""

import argparse
import json
from pathlib import Path
from typing import Any

Record = dict[str, Any]
EMPTY: Record = {}

SCOPE_RECEIPTS = ("stubborn.json", "live-exit.json")

# Pairs that the runtime or the fixture orders by construction. TERM is not
# ordered against child readiness: the open cases are about that race.
SCOPE_ORDER = (
    ("spawn", "peer_start"),
    ("peer_start", "handler_installed"),
    ("handler_installed", "child_spawned"),
    ("child_spawned", "child_ready"),
    ("spawn", "term"),
    ("term", "kill"),
    ("term", "leader_exit"),
    ("helper_start", "helper_end"),
)
SCOPE_REQUIRED = (
    "spawn",
    "peer_start",
    "handler_installed",
    "child_spawned",
    "child_ready",
    "term",
    "leader_exit",
    "helper_start",
    "helper_end",
)
PREBUFFER_ORDER = (
    ("dispatch", "helper_start"),
    ("helper_start", "identity_written"),
    ("identity_written", "helper_end_bound"),
    ("helper_end_bound", "term"),
    ("wait_started", "wait_left"),
)
PREBUFFER_REQUIRED = (
    "dispatch",
    "helper_start",
    "identity_written",
    "helper_end_bound",
    "term",
    "wait_started",
    "wait_left",
)


def load_stamps(directory: Path) -> dict[int, list[Record]]:
    stamps: dict[int, list[Record]] = {}
    if not directory.is_dir():
        return stamps
    for path in sorted(directory.glob("*.jsonl")):
        for line in path.read_text().splitlines():
            event: Record = json.loads(line)
            stamps.setdefault(int(event["pid"]), []).append(event)
    return stamps


def first(events: list[Record], name: str) -> Record | None:
    return next((event for event in events if event["event"] == name), None)


def scope_record(path: Path, stamps: dict[int, list[Record]]) -> Record:
    receipt: Record = json.loads(path.read_text())
    peer = stamps.get(int(receipt["pid"]), [])
    spawned = first(peer, "child_spawned")
    child = int(spawned["child"]) if spawned else None
    signals = {item["signal"]: item["monotonic"] for item in receipt["signals"]}
    helpers: list[Record] = [
        {
            "group_started": item.get("started_monotonic"),
            "helper_start": (item.get("helper") or EMPTY).get("created_monotonic"),
            "helper_end": (item.get("helper") or EMPTY).get("reaped_monotonic"),
            "capture_deadline": (item.get("helper") or EMPTY).get("capture_deadline_monotonic"),
            "helper_kill": (item.get("helper") or EMPTY).get("kill_monotonic"),
            "helper_error": (item.get("helper") or EMPTY).get("error"),
            "state": item.get("state"),
        }
        for item in receipt.get("group_observations", [])
    ]
    record: Record = {
        "kind": "scope",
        "receipt": str(path),
        "run_id": path.parent.name,
        "name": receipt["name"],
        "reason": receipt.get("reason"),
        "returncode": receipt.get("returncode"),
        "termination_ok": receipt.get("termination_ok"),
        "leader": receipt["pid"],
        "child": child,
        "spawn": receipt["started_monotonic"],
        "peer_start": (first(peer, "start") or EMPTY).get("monotonic"),
        "handler_installed": (first(peer, "handler_installed") or EMPTY).get("monotonic"),
        "child_spawned": spawned["monotonic"] if spawned else None,
        "child_ready": None,
        "readiness_loop_left": (first(peer, "readiness_loop_left") or EMPTY).get("monotonic"),
        "pid_printed": (first(peer, "pid_printed") or EMPTY).get("monotonic"),
        "term": signals.get("TERM"),
        "kill": signals.get("KILL"),
        "term_deadline": receipt.get("term_deadline_monotonic"),
        "leader_exit": receipt.get("leader_exit_monotonic"),
        "helper_start": helpers[0]["helper_start"] if helpers else None,
        "helper_end": helpers[0]["helper_end"] if helpers else None,
        "helpers": helpers,
    }
    if child is not None:
        record["child_ready"] = (first(stamps.get(child, []), "ready_written") or EMPTY).get(
            "monotonic"
        )
    return record


def prebuffer_record(witness: Path, receipts: list[Path]) -> Record:
    info: Record = json.loads(witness.read_text())
    parent_path = witness.with_suffix(".parent.json")
    parent: Record = json.loads(parent_path.read_text()) if parent_path.exists() else {}
    publication = witness.stem.split("-")[3]
    # retain() names its receipt after the capture returns, so the first
    # receipt for this publication written after the witness belongs to it.
    stamp = int(witness.stem.rsplit("-", 1)[1])
    receipt = next(
        (
            json.loads(path.read_text())
            for path in receipts
            if path.name.startswith(f"worker-late-created-prebuffer-{publication}-")
            and int(path.stem.rsplit("-", 1)[1]) > stamp
        ),
        None,
    )
    worker: Record = (receipt or EMPTY).get("worker") or EMPTY
    term = next(
        (item["monotonic"] for item in worker.get("signals", []) if item["signal"] == "SIGTERM"),
        None,
    )
    return {
        "kind": "prebuffer",
        "witness": str(witness),
        "publication": publication,
        "receipt_retained": receipt is not None,
        "outcome": parent.get("outcome"),
        "returncode": worker.get("returncode"),
        "dispatch": worker.get("dispatch_monotonic"),
        "helper_start": (info.get("identity") or EMPTY).get("created_monotonic"),
        "identity_written": info.get("identity_written_monotonic"),
        "helper_end_bound": info.get("pipe_before_term_monotonic"),
        "term": term,
        "wait_started": parent.get("wait_started_monotonic"),
        "wait_left": parent.get("wait_left_monotonic"),
        "query_deadline": info.get("query_deadline"),
    }


def records(evidence: Path, stamps_directory: Path) -> list[Record]:
    stamps = load_stamps(stamps_directory)
    found: list[Record] = []
    if not evidence.is_dir():
        return found
    for run in sorted(path for path in evidence.iterdir() if path.is_dir()):
        for name in SCOPE_RECEIPTS:
            if (run / name).is_file():
                found.append(scope_record(run / name, stamps))
    receipts = sorted(evidence.glob("worker-late-created-prebuffer-*.json"))
    for witness in sorted(evidence.glob("late-created-prebuffer-*.json")):
        if not witness.name.endswith(".parent.json"):
            found.append(prebuffer_record(witness, receipts))
    return found


def check(record: Record) -> list[str]:
    """Return every missing required field and every reversed ordered pair."""
    if record["kind"] == "scope":
        required, order = SCOPE_REQUIRED, SCOPE_ORDER
    else:
        required, order = PREBUFFER_REQUIRED, PREBUFFER_ORDER
    if record.get("reason") == "exit":
        # The uninterrupted live-exit leader leaves on its own, without TERM.
        required = tuple(name for name in required if name != "term")
    problems = [f"missing {name}" for name in required if record.get(name) is None]
    for before, after in order:
        if record.get(before) is not None and record.get(after) is not None:
            if record[before] > record[after]:
                problems.append(f"{before} after {after}")
    return problems


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path)
    parser.add_argument("stamps", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    found = records(args.evidence, args.stamps)
    result = [{"record": record, "problems": check(record)} for record in found]
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    flagged = sum(bool(item["problems"]) for item in result)
    print(json.dumps({"records": len(result), "with_problems": flagged}))


if __name__ == "__main__":
    main()
