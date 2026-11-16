"""Measure the scope-parent leader's exit latency after group TERM.

k132 failed test_review_exited_leader_does_not_hide_a_surviving_group_member in
both parameters. Each receipt shows group TERM after declared readiness. The
leader then exited 168 ms after TERM, or met KILL at the 150 ms grace deadline.
The fixture path after TERM is one 5 ms poll, a 30 ms child wait and exit.

This tool runs the same controller call repeatedly against one input set. It
records TERM to leader-exit latency, so the historical latency can be compared
with the fixture's measured distribution. The stubborn child is released and
audited by exact PID after each run. With --load, the tool also owns busy
processes that compete for the processor, then kills and reaps each by PID.
"""

import argparse
import hashlib
import json
import statistics
import subprocess
import sys
import time
import uuid
from pathlib import Path
from typing import Any

BUDGET = 0.3
TERM_GRACE = 0.15
KILL_WAIT = 0.3


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(controller: Any, inputs: Path, out: Path, index: int) -> dict[str, Any]:
    work = out / f"work-{index}"
    work.mkdir()
    release, ready = work / "release", work / "ready"
    peer = inputs / "peer.py"
    argv = (sys.executable, str(peer), "scope-parent", str(release), str(ready), "stubborn-child")
    call = controller.Call("stubborn", argv, BUDGET, expected_exit=-15, expected_reason="deadline")
    plan = controller.Plan(
        uuid.uuid4().hex, (call,), (), (peer,), term_grace=TERM_GRACE, kill_wait=KILL_WAIT
    )
    try:
        path = controller.execute(plan, out / "evidence")
    finally:
        release.write_bytes(b"release")
    receipt = json.loads((path / "stubborn.json").read_text())
    signals = receipt.get("signals", [])
    term = next((item for item in signals if item["signal"] == "TERM"), None)
    kill = next((item for item in signals if item["signal"] == "KILL"), None)
    leader_exit = receipt.get("leader_exit_monotonic")
    child = (path / "stubborn.stdout").read_text().strip()
    audit: list[str] = []
    if child.isdigit():
        end = time.monotonic() + 1
        while time.monotonic() < end:
            probe = subprocess.run(
                ("/bin/ps", "-o", "pid=,stat=", "-p", child), capture_output=True, check=False
            )
            audit.append(probe.stdout.decode().strip())
            if probe.returncode != 0:
                break
            time.sleep(0.005)
    latency = None
    if term is not None and isinstance(leader_exit, float):
        latency = leader_exit - term["monotonic"]
    return {
        "run": str(path.relative_to(out)),
        "signal_scope": receipt.get("signal_scope"),
        "term": term,
        "kill": kill,
        "term_deadline_monotonic": receipt.get("term_deadline_monotonic"),
        "leader_exit_monotonic": leader_exit,
        "term_to_exit": latency,
        "returncode": receipt.get("returncode"),
        "child": child,
        "child_audit": audit,
        "child_absent": not child.isdigit() or (bool(audit) and audit[-1] == ""),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--repeat", type=int, default=20)
    parser.add_argument("--load", type=int, default=0)
    args = parser.parse_args()
    inputs, out = args.inputs.resolve(), args.out.resolve()
    out.mkdir(parents=True)
    sys.path.insert(0, str(inputs))
    import controller  # noqa: PLC0415  (imported from the frozen inputs)

    subjects = sorted(inputs.glob("*.py"))
    before = {path.name: sha256(path) for path in subjects}
    load = [
        subprocess.Popen((sys.executable, "-I", "-c", "while True: pass")) for _ in range(args.load)
    ]
    try:
        runs = [run(controller, inputs, out, index) for index in range(args.repeat)]
    finally:
        load_audit: list[dict[str, int]] = []
        for process in load:
            process.kill()
            load_audit.append({"pid": process.pid, "returncode": process.wait(timeout=5)})
    after = {path.name: sha256(path) for path in subjects}
    latencies = [float(item["term_to_exit"]) for item in runs if item["term_to_exit"] is not None]
    summary = {
        "count": len(latencies),
        "min": min(latencies, default=None),
        "median": statistics.median(latencies) if latencies else None,
        "max": max(latencies, default=None),
        "killed": sum(item["kill"] is not None for item in runs),
        "group_scope": sum(item["signal_scope"] == "group" for item in runs),
        "children_absent": all(item["child_absent"] for item in runs),
    }
    result = {
        "control": "scope-parent-exit-latency-after-group-term",
        "inputs": str(inputs),
        "python": sys.version,
        "budget": BUDGET,
        "term_grace": TERM_GRACE,
        "kill_wait": KILL_WAIT,
        "load_processes": args.load,
        "load_audit": load_audit,
        "historical_k132": {"term_to_exit_drop_kill": 0.167848375, "kill_before_exit": True},
        "subjects_before": before,
        "subjects_after": after,
        "subjects_unchanged": before == after,
        "summary": summary,
        "runs": runs,
    }
    (out / "exit-latency.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
