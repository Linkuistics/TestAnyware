"""Reproduce k110's `?E` group table with a real slow-exiting leader.

k110 failed test_review_group_kill_precedes_inspection[ignore-term]. Its leader
ignored TERM and received group KILL. Leader exit was observed 64 us after KILL.
The final ps table started 2.8 ms later and reported the leader as `?E`. The
parser rejected that state, so the group became unknown and termination failed.

Kqueue rejects a NOTE_EXIT registration with ESRCH from exit start, and the
runtime counts ESRCH as exit. A leader with a large address space keeps the
`?E` state for several milliseconds of teardown. This tool runs the real
controller with such a leader, then audits the leader PID by exact identity.
"""

import argparse
import hashlib
import json
import subprocess
import sys
import time
import uuid
from pathlib import Path
from typing import Any

LEADER = """
import os, signal, sys, time
signal.signal(signal.SIGTERM, signal.SIG_IGN)
block = bytearray(int(sys.argv[1]))
for index in range(0, len(block), 4096):
    block[index] = 1
os.write(1, b"%d\\n" % os.getpid())
time.sleep(30)
"""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(controller: Any, out: Path, index: int, size: int) -> dict[str, object]:
    argv = (sys.executable, "-I", "-c", LEADER, str(size))
    call = controller.Call(
        f"exiting-{index}", argv, 1.5, expected_exit=-9, expected_reason="deadline"
    )
    plan = controller.Plan(uuid.uuid4().hex, (call,), (), (), term_grace=0.15, kill_wait=0.5)
    path = controller.execute(plan, out / "evidence")
    receipt: dict[str, Any] = json.loads((path / f"exiting-{index}.json").read_text())
    groups = receipt.get("group_observations", [])
    final: dict[str, Any] = groups[-1] if groups else {}
    kill = next((item for item in receipt.get("signals", []) if item["signal"] == "KILL"), None)
    helper: dict[str, Any] = final.get("helper") or {}
    pid = receipt.get("pid")
    probe = subprocess.run(
        ("/bin/ps", "-o", "pid=,stat=", "-p", str(pid)), capture_output=True, check=False
    )
    return {
        "run": str(path.relative_to(out)),
        "pid": pid,
        "kill": kill,
        "leader_exit_monotonic": receipt.get("leader_exit_monotonic"),
        "query_created_monotonic": helper.get("created_monotonic"),
        "table": bytes.fromhex(helper["stdout"]).decode() if helper.get("stdout") else None,
        "state": final.get("state"),
        "error": final.get("error"),
        "returncode": receipt.get("returncode"),
        "reaped": receipt.get("reaped"),
        "termination_ok": receipt.get("termination_ok"),
        "ok": receipt.get("ok"),
        "leader_absent_after": probe.returncode != 0 and not probe.stdout.strip(),
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("inputs", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--repeat", type=int, default=10)
    parser.add_argument("--size", type=int, default=2_000_000_000)
    args = parser.parse_args()
    inputs, out = args.inputs.resolve(), args.out.resolve()
    out.mkdir(parents=True)
    sys.path.insert(0, str(inputs))
    import controller  # noqa: PLC0415  (imported from the frozen inputs)

    subjects = sorted(inputs.glob("*.py"))
    before = {path.name: sha256(path) for path in subjects}
    runs = [run(controller, out, index, args.size) for index in range(args.repeat)]
    time.sleep(0.1)
    after = {path.name: sha256(path) for path in subjects}
    summary = {
        "runs": len(runs),
        "exiting_tables": sum("?E" in str(item["table"]) for item in runs),
        "unknown": sum(item["state"] == "unknown" for item in runs),
        "ok": sum(item["ok"] is True for item in runs),
        "leaders_absent": all(item["leader_absent_after"] for item in runs),
    }
    result = {
        "control": "k110-exiting-leader-table",
        "inputs": str(inputs),
        "module": controller.__file__,
        "python": sys.version,
        "leader_bytes": args.size,
        "subjects_before": before,
        "subjects_after": after,
        "subjects_unchanged": before == after,
        "summary": summary,
        "runs": runs,
    }
    (out / "exiting-leader.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(summary))
    for item in runs:
        print(item["table"], item["state"], item["error"], item["returncode"], item["ok"])


if __name__ == "__main__":
    main()
