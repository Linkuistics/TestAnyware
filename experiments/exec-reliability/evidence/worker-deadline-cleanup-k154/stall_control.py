"""Reproduce the k152 run 42 leak without a host stall, and audit each child.

The control runs selected deadline tests in-process, in one frozen input set.
With --stall, the runtime's first worker signal waits until the shutdown
deadline. The runtime then refuses every signal, as it did in k152 run 42.

After the tests, the control audits each child that a test created:
- whether it is still an unreaped child of this process;
- whether its owner record is still in worker_capture._owners.
The control then removes any remaining exact child with SIGKILL to its PID.
An unreaped direct child keeps its PID, so that signal cannot reach another
process. The control never signals a process group or a PID it did not audit.
"""

# The control reads and wraps private runtime state on purpose, as the tests do.
# pyright: reportPrivateUsage=false

import argparse
import hashlib
import json
import math
import os
import signal
import subprocess
import sys
import time
from collections.abc import Generator, Mapping
from pathlib import Path
from typing import Any

import pytest

DEFAULT_TESTS = [
    "test_worker_capture.py::test_worker_shutdown_deadline_closes_owner_operations[stale-kill-pid]",
    "test_worker_capture.py::test_worker_shutdown_deadline_closes_owner_operations[stale-kill-group]",
]


def digest(inputs: Path) -> dict[str, str]:
    return {
        path.name: hashlib.sha256(path.read_bytes()).hexdigest()
        for path in sorted(inputs.glob("*.py"))
    }


def children() -> list[str]:
    """Return ps rows for each process whose parent is this process, except ps."""
    with subprocess.Popen(
        ["ps", "-ax", "-o", "pid=,ppid=,pgid=,stat=,lstart=,command="],
        stdout=subprocess.PIPE,
        text=True,
    ) as ps:
        output, _ = ps.communicate()
    if ps.returncode:
        raise subprocess.CalledProcessError(ps.returncode, ps.args)
    mine, own = str(os.getpid()), str(ps.pid)
    return [
        row.strip()
        for row in output.splitlines()
        if row.split()[1:2] == [mine] and row.split()[:1] != [own]
    ]


class Stall:
    """Delay the runtime's first signal of each test until its deadline."""

    def __init__(self, enabled: bool) -> None:
        self.enabled = enabled
        self.records: dict[str, dict[str, object]] = {}
        self.outcomes: dict[str, dict[str, str]] = {}

    @pytest.hookimpl(wrapper=True)
    def pytest_runtest_call(self, item: pytest.Item) -> Generator[None, object, object]:
        import worker_capture

        real = worker_capture._signal
        record: dict[str, object] = {"stalled": False}
        self.records[item.nodeid] = record

        def stalled_signal(owner: Any, sig: signal.Signals) -> None:
            deadline = owner.shutdown_deadline_monotonic
            # An owner with no finite deadline has no deadline to stall past.
            if self.enabled and not record["stalled"] and math.isfinite(deadline):
                started = time.monotonic()
                while (remaining := deadline - time.monotonic()) > 0:
                    time.sleep(remaining)
                record.update(
                    stalled=True,
                    signal=sig.name,
                    started=started,
                    resumed=time.monotonic(),
                    deadline=deadline,
                    pid=owner.pid,
                )
            real(owner, sig)

        # The test captures worker_capture._signal at its start, so this wrapper
        # sits under the test's own wrapper and under the runtime's calls.
        worker_capture._signal = stalled_signal
        try:
            outcome: object = yield
            return outcome
        finally:
            worker_capture._signal = real

    def pytest_runtest_logreport(self, report: pytest.TestReport) -> None:
        if report.when == "call" or report.outcome != "passed":
            self.outcomes.setdefault(report.nodeid, {})[report.when] = report.outcome


def audit_child(pid: int, owners: Mapping[int, object]) -> dict[str, object]:
    entry: dict[str, object] = {"pid": pid, "owner_record_after_tests": pid in owners}
    try:
        found, status = os.waitpid(pid, os.WNOHANG)
    except ChildProcessError:
        entry.update(state_after_tests="reaped before audit", removal="not needed")
        return entry
    if found == pid:
        entry.update(
            state_after_tests="exited but unreaped",
            status=status,
            removal="reaped by control",
        )
        return entry
    entry["state_after_tests"] = "alive and unreaped"
    try:
        entry["pgid"] = os.getpgid(pid)
    except OSError as error:
        entry["pgid_errno"] = error.errno
    sent = time.monotonic()
    try:
        os.kill(pid, signal.SIGKILL)
    except OSError as error:
        entry["kill_errno"] = error.errno
    end = sent + 1
    while time.monotonic() < end:
        found, status = os.waitpid(pid, os.WNOHANG)
        if found == pid:
            entry.update(
                removal="SIGKILL to exact PID by control, then reaped",
                status=status,
                kill_to_reap_seconds=time.monotonic() - sent,
            )
            break
        time.sleep(0.001)
    else:
        entry["removal"] = "SIGKILL sent, not reaped within 1 s"
    return entry


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("out", type=Path)
    parser.add_argument("--stall", action="store_true")
    parser.add_argument("--test", action="append", dest="tests")
    parser.add_argument(
        "--survey",
        action="store_true",
        help="record tests that pass or never signal as notes, not failures",
    )
    args = parser.parse_args()
    inputs, out = args.inputs.resolve(), args.out.resolve()
    tests = args.tests or DEFAULT_TESTS
    out.mkdir(parents=True, exist_ok=False)
    receipts = out / "receipts"
    os.environ["DIAGNOSTIC_EVIDENCE"] = str(receipts)
    before = digest(inputs)
    os.chdir(inputs)
    sys.path.insert(0, str(inputs))
    stall = Stall(args.stall)
    started = time.time()
    code = pytest.main(
        ["-vv", "-p", "no:cacheprovider", f"--junitxml={out / 'pytest.xml'}", *tests],
        plugins=[stall],
    )
    import worker_capture

    owners = dict(worker_capture._owners)
    rows_before = children()
    audits = sorted(receipts.glob("worker-deadline-audit-*.json")) if receipts.exists() else []
    # Each source names a child that this process forked. audit_child proves the
    # PID is still this process's child with waitpid before it sends a signal.
    sources: dict[int, list[str]] = {}
    for path in audits:
        sources.setdefault(json.loads(path.read_text())["audit"]["pid"], []).append(
            "deadline audit receipt"
        )
    for record in stall.records.values():
        if isinstance(record.get("pid"), int):
            sources.setdefault(int(record["pid"]), []).append("stalled signal")  # type: ignore[arg-type]
    for pid in owners:
        sources.setdefault(pid, []).append("owner record after tests")
    for row in rows_before:
        sources.setdefault(int(row.split()[0]), []).append("child row after tests")
    resources = [{"sources": sources[pid], **audit_child(pid, owners)} for pid in sorted(sources)]
    rows_after = children()
    after = digest(inputs)
    residual = [
        entry
        for entry in resources
        if entry.get("state_after_tests") != "reaped before audit"
        or entry["owner_record_after_tests"]
    ]
    failed = {nodeid for nodeid, phases in stall.outcomes.items() if phases.get("call") == "failed"}
    stalled = [nodeid for nodeid, record in stall.records.items() if record["stalled"]]
    reasons: list[str] = []
    notes: list[str] = []
    expected = notes if args.survey else reasons
    if residual:
        reasons.append("a test left a child or an owner record")
    if args.stall and sorted(failed) != sorted(tests):
        expected.append("a stalled test did not fail")
    if args.stall and sorted(stalled) != sorted(tests):
        expected.append("a test did not reach the stalled signal")
    if before != after:
        reasons.append("inputs changed during the control")
    if rows_after:
        reasons.append("a child of the control remains")
    result = {
        "inputs": str(inputs),
        "inputs_sha256_before": before,
        "inputs_sha256_after": after,
        "control_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        "stall": args.stall,
        "survey": args.survey,
        "tests": tests,
        "pytest_exit": int(code),
        "outcomes": stall.outcomes,
        "stalls": stall.records,
        "resources": resources,
        "children_before_cleanup": rows_before,
        "children_after_cleanup": rows_after,
        "wall_started": started,
        "wall_finished": time.time(),
        "verdict": "fail" if reasons else "pass",
        "reasons": reasons,
        "notes": notes,
    }
    (out / "control.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"verdict": result["verdict"], "reasons": reasons}, indent=2))
    return 1 if reasons else 0


if __name__ == "__main__":
    sys.exit(main())
