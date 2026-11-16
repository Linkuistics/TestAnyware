"""Run the k152 soak once, to a plan frozen before the first run.

Each scheduled run is one pytest process in the frozen inputs, with its own
receipts, peer stamps and JUnit report. Loaded runs add busy processes that
this tool owns and audits. The tool never retries a run and never extends the
schedule. When the wall-clock budget is spent, it records the runs it did not
start and stops.
"""

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import threading
import time
import uuid
import xml.etree.ElementTree as ElementTree
from pathlib import Path
from typing import Any

import timeline

Record = dict[str, Any]
HERE = Path(__file__).resolve().parent


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def subjects(inputs: Path, plan: Path) -> dict[str, str]:
    paths = [
        *sorted(inputs.glob("*.py")),
        inputs / "pyproject.toml",
        inputs / "uv.lock",
        HERE / "soak.py",
        HERE / "timeline.py",
        plan,
    ]
    return {str(path): sha256(path) for path in paths}


def schedule(plan: Record) -> list[Record]:
    """Expand the plan into its exact run order, idle and loaded alternating."""
    runs: list[Record] = []
    for block in plan["blocks"]:
        for index in range(block["runs"]):
            load = "idle" if index % 2 == 0 else "loaded"
            runs.append({"workload": block["workload"], "load": load})
    return runs


class LoadSampler:
    """Sample the one-minute load average. This thread is the only writer."""

    def __init__(self, interval: float) -> None:
        self.interval = interval
        self.samples: list[Record] = []
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self.run, daemon=True)

    def run(self) -> None:
        while not self.stop.is_set():
            self.samples.append({"monotonic": time.monotonic(), "loadavg": os.getloadavg()})
            self.stop.wait(self.interval)


def junit(path: Path) -> Record:
    if not path.is_file():
        return {"present": False}
    root = ElementTree.parse(path).getroot()
    cases: list[Record] = []
    for case in root.iter("testcase"):
        outcome = "passed"
        message = None
        for child in case:
            if child.tag in {"failure", "error", "skipped"}:
                outcome = child.tag
                message = child.get("message")
        cases.append(
            {
                "nodeid": f"{case.get('classname')}::{case.get('name')}",
                "time": float(case.get("time") or 0),
                "outcome": outcome,
                "message": message,
            }
        )
    return {
        "present": True,
        "cases": cases,
        "failed": [case for case in cases if case["outcome"] in {"failure", "error"}],
    }


def ps(pid: int) -> Record:
    completed = subprocess.run(
        ("/bin/ps", "-o", "pid=,stat=", "-p", str(pid)),
        capture_output=True,
        text=True,
        check=False,
        timeout=5,
    )
    if completed.returncode == 1 and not completed.stdout and not completed.stderr:
        state = "absent"
    elif completed.returncode == 0 and completed.stdout.split()[:1] == [str(pid)]:
        state = "present"
    else:
        state = "unknown"
    return {
        "pid": pid,
        "state": state,
        "returncode": completed.returncode,
        "stdout": completed.stdout,
        "stderr": completed.stderr,
    }


def start_load(count: int) -> list[subprocess.Popen[bytes]]:
    return [
        subprocess.Popen((sys.executable, "-I", "-c", "while True: pass")) for _ in range(count)
    ]


def stop_load(load: list[subprocess.Popen[bytes]]) -> list[Record]:
    audit: list[Record] = []
    for process in load:
        process.send_signal(signal.SIGKILL)
        audit.append({"pid": process.pid, "returncode": process.wait(timeout=5)})
    for item in audit:
        item["after"] = ps(item["pid"])["state"]
    return audit


def execute(inputs: Path, out: Path, index: int, item: Record, plan: Record) -> Record:
    workload = plan["workloads"][item["workload"]]
    run = out / "runs" / f"{index:03d}-{item['workload']}-{item['load']}-{uuid.uuid4().hex[:8]}"
    stamps, evidence = run / "stamps", run / "receipts"
    stamps.mkdir(parents=True)
    evidence.mkdir()
    env = dict(os.environ, PEER_STAMPS=str(stamps), DIAGNOSTIC_EVIDENCE=str(evidence))
    load = start_load(plan["load_processes"] if item["load"] == "loaded" else 0)
    record: Record = {
        "index": index,
        "run": str(run),
        **item,
        "load_pids": [process.pid for process in load],
        "loadavg_before": os.getloadavg(),
        "wall_start_ns": time.time_ns(),
        "started_monotonic": time.monotonic(),
    }
    argv = (
        sys.executable,
        "-m",
        "pytest",
        "-vv",
        "-p",
        "no:cacheprovider",
        f"--junitxml={run / 'pytest.xml'}",
        *workload["tests"],
    )
    record["argv"] = list(argv)
    timed_out = False
    with (run / "pytest.stdout").open("wb") as stdout, (run / "pytest.stderr").open("wb") as err:
        process = subprocess.Popen(
            argv, cwd=inputs, env=env, stdout=stdout, stderr=err, start_new_session=True
        )
        try:
            returncode = process.wait(timeout=workload["timeout_seconds"])
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            returncode = process.wait(timeout=5)
    record["finished_monotonic"] = time.monotonic()
    record["loadavg_after"] = os.getloadavg()
    record["load_audit"] = stop_load(load)
    record["returncode"] = returncode
    record["timed_out"] = timed_out
    record["junit"] = junit(run / "pytest.xml")
    found = timeline.records(evidence, stamps)
    checked = [{"record": entry, "problems": timeline.check(entry)} for entry in found]
    (run / "timeline.json").write_text(json.dumps(checked, indent=2) + "\n")
    record["timeline_records"] = len(checked)
    record["timeline_problems"] = [entry for entry in checked if entry["problems"]]
    peers = sorted(timeline.load_stamps(stamps))
    record["process_audit"] = [ps(pid) for pid in peers]
    record["residual_processes"] = [
        entry for entry in record["process_audit"] if entry["state"] != "absent"
    ]
    (run / "run.json").write_text(json.dumps(record, indent=2) + "\n")
    return record


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--plan", type=Path, required=True)
    args = parser.parse_args()
    inputs, out, plan_path = args.inputs.resolve(), args.out.resolve(), args.plan.resolve()
    out.mkdir(parents=True, exist_ok=False)
    plan: Record = json.loads(plan_path.read_text())
    (out / "plan.json").write_bytes(plan_path.read_bytes())
    before = subjects(inputs, plan_path)
    order = schedule(plan)
    sampler = LoadSampler(plan["load_sample_seconds"])
    sampler.thread.start()
    started = time.monotonic()
    runs: list[Record] = []
    unstarted: list[Record] = []
    for index, item in enumerate(order):
        if time.monotonic() - started >= plan["wall_budget_seconds"]:
            unstarted = [dict(entry, index=i) for i, entry in enumerate(order) if i >= index]
            break
        record = execute(inputs, out, index, item, plan)
        runs.append(record)
        failed = len(record["junit"].get("failed", []))
        label = f"{index:03d} {item['workload']} {item['load']}"
        print(f"{label} rc={record['returncode']} failed={failed}", flush=True)
    finished = time.monotonic()
    sampler.stop.set()
    sampler.thread.join()
    after = subjects(inputs, plan_path)
    summary: Record = {
        "leaf": "historical-failure-soak-k152",
        "plan_sha256": sha256(plan_path),
        "inputs": str(inputs),
        "subjects_before": before,
        "subjects_after": after,
        "subjects_unchanged": before == after,
        "started_monotonic": started,
        "finished_monotonic": finished,
        "wall_seconds": finished - started,
        "scheduled": len(order),
        "completed": len(runs),
        "unstarted": unstarted,
        "failed_runs": [
            {
                "index": run["index"],
                "run": run["run"],
                "workload": run["workload"],
                "load": run["load"],
                "returncode": run["returncode"],
                "timed_out": run["timed_out"],
                "failed": run["junit"].get("failed", []),
            }
            for run in runs
            if run["returncode"] != 0 or run["timed_out"]
        ],
        "timeline_problem_runs": [run["index"] for run in runs if run["timeline_problems"]],
        "residual_process_runs": [run["index"] for run in runs if run["residual_processes"]],
        "load_residue_runs": [
            run["index"]
            for run in runs
            if any(item["after"] != "absent" for item in run["load_audit"])
        ],
        "runs": [
            {key: value for key, value in run.items() if key != "process_audit"} for run in runs
        ],
    }
    (out / "load.json").write_text(json.dumps(sampler.samples, indent=2) + "\n")
    (out / "soak.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(
        json.dumps(
            {
                key: summary[key]
                for key in (
                    "scheduled",
                    "completed",
                    "wall_seconds",
                    "subjects_unchanged",
                    "timeline_problem_runs",
                    "residual_process_runs",
                    "load_residue_runs",
                )
            }
        )
    )
    failures = [[run["index"], run["workload"], run["load"]] for run in summary["failed_runs"]]
    print(json.dumps(failures))


if __name__ == "__main__":
    main()
