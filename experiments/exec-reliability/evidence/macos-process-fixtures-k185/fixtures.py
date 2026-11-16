"""k185 control for owned process fixtures in the macOS agent tests.

The script starts one unrelated control process outside the test suite. The
control runs `sleep 300`, so the old broad pattern `pkill -f 'sleep 30'` selects
it. The script then runs the suite and checks whether the control survived.

`--arm owned` runs `task agent:macos:test` on the working tree, which uses
owned fixtures. The expected verdict is `survives`. `--arm broad` is the fault
control. It copies the agent package to a temporary directory and restores the
test file of `--baseline-rev`, which has the broad cleanup. The expected
verdict is `killed`.

The control is a child of this script, so its PID cannot name another process
until this script reaps it. Before the broad arm runs, the script lists the
processes of this user that match the broad pattern. It refuses to run when
any process other than its own control matches, because the restored cleanup
would stop that process.

Every input, bound and hash goes into OUT/plan.json before the first run. OUT
must not exist. The script exits 0 only when every run gives the `--expect`
verdict and the owned arm leaves no matching process of its own behind.
Suite failures are recorded per test case. They are not part of the verdict.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
PACKAGE = ROOT / "agents/macos"
TESTS = PACKAGE / "Tests/TestAnywareAgentTests"
TEST_FILE = TESTS / "ProcessRunnerTests.swift"
FIXTURE_FILE = TESTS / "OwnedProcessFixture.swift"
RUNNER = PACKAGE / "Sources/TestAnywareAgent/ProcessRunner.swift"
SELF = Path(__file__).resolve()
TEST_REL = str(TEST_FILE.relative_to(ROOT))

BROAD_PATTERN = "sleep 30"
CONTROL_ARGV = ["sleep", "300"]
BUILD_BOUND_SECONDS = 900.0
SUITE_BOUND_SECONDS = 240.0
STOP_GRACE_SECONDS = 2.0
KILL_PATTERN = re.compile(r"\b(pkill|killall)\b")
XCTEST_CASE = re.compile(r"Test Case '-\[(\S+) (\w+)\]' (passed|failed) \(([\d.]+) seconds\)")
SWIFT_TESTING_FAIL = re.compile(r"Test (\w+)\(\) (?:failed|recorded an issue)")
FIXTURE_LINE = re.compile(r"owned-process-fixture: (.+)")


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256(path: Path) -> str:
    return sha256_bytes(path.read_bytes())


def tool_version(argv: list[str]) -> str:
    done = subprocess.run(argv, capture_output=True, text=True, timeout=60, check=False)
    return (done.stdout + done.stderr).strip()


def baseline_test_source(rev: str) -> bytes:
    done = subprocess.run(
        ["git", "-C", str(ROOT), "show", f"{rev}:{TEST_REL}"],
        capture_output=True,
        timeout=60,
        check=True,
    )
    return done.stdout


def kill_audit(baseline: bytes) -> dict[str, Any]:
    """Counts broad kill commands in the agent package source.

    The baseline test file is the positive control: the same scan must find
    its broad cleanup.
    """
    current: dict[str, int] = {}
    for path in sorted(PACKAGE.rglob("*.swift")):
        if ".build" in path.parts:
            continue
        hits = len(KILL_PATTERN.findall(path.read_text(encoding="utf-8")))
        if hits:
            current[str(path.relative_to(ROOT))] = hits
    return {
        "pattern": KILL_PATTERN.pattern,
        "current_hits": current,
        "baseline_test_hits": len(KILL_PATTERN.findall(baseline.decode("utf-8"))),
    }


def matching_pids() -> list[dict[str, str]]:
    """Lists the processes of this user that the broad pattern selects."""
    done = subprocess.run(
        ["pgrep", "-U", str(os.getuid()), "-lf", BROAD_PATTERN],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    rows: list[dict[str, str]] = []
    for line in done.stdout.splitlines():
        pid, _, command = line.strip().partition(" ")
        rows.append({"pid": pid, "command": command, "started": started(int(pid))})
    return rows


def started(pid: int) -> str:
    done = subprocess.run(
        ["ps", "-o", "lstart=", "-p", str(pid)],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    return done.stdout.strip()


def run_bounded(argv: list[str], cwd: Path, log: Path, bound: float) -> dict[str, Any]:
    """Runs argv in its own session, kills the group at the bound."""
    t0 = time.monotonic()
    with log.open("wb") as sink:
        proc = subprocess.Popen(
            argv, cwd=cwd, stdout=sink, stderr=subprocess.STDOUT, start_new_session=True
        )
        try:
            code: int | None = proc.wait(timeout=bound)
            bounded = False
        except subprocess.TimeoutExpired:
            os.killpg(proc.pid, signal.SIGKILL)
            proc.wait(timeout=STOP_GRACE_SECONDS)
            code = None
            bounded = True
    return {
        "argv": argv,
        "cwd": str(cwd),
        "exit": code,
        "hit_bound": bounded,
        "seconds": round(time.monotonic() - t0, 3),
        "log": log.name,
        "log_sha256": sha256(log),
    }


def parse_suite(log: Path) -> dict[str, Any]:
    text = log.read_text(encoding="utf-8", errors="replace")
    cases = [
        {"case": f"{m[1]}.{m[2]}", "result": m[3], "seconds": float(m[4])}
        for m in XCTEST_CASE.finditer(text)
    ]
    failures = [line for line in text.splitlines() if ": error: " in line]
    return {
        "xctest_cases": cases,
        "xctest_failed": [c["case"] for c in cases if c["result"] == "failed"],
        "swift_testing_failed": SWIFT_TESTING_FAIL.findall(text),
        "failure_lines": failures,
        "fixture_lines": FIXTURE_LINE.findall(text),
    }


def stop_control(proc: subprocess.Popen[bytes]) -> dict[str, Any]:
    code = proc.poll()
    if code is not None:
        return {"alive_after_suite": False, "returncode": code, "stopped_by": "suite"}
    proc.terminate()
    try:
        code = proc.wait(timeout=STOP_GRACE_SECONDS)
        how = "script-term"
    except subprocess.TimeoutExpired:
        proc.kill()
        code = proc.wait(timeout=STOP_GRACE_SECONDS)
        how = "script-kill"
    return {"alive_after_suite": True, "returncode": code, "stopped_by": how}


def prepare_broad(baseline: bytes, out: Path) -> Path:
    """Copies the package with the baseline test file into a temporary tree."""
    tree = Path(tempfile.mkdtemp(prefix="testanyware-k185-broad-"))
    package = tree / "macos"
    shutil.copytree(PACKAGE, package, ignore=shutil.ignore_patterns(".build"))
    # An APFS clone of the build directory saves most of the dependency build.
    subprocess.run(
        ["cp", "-c", "-R", str(PACKAGE / ".build"), str(package / ".build")],
        check=True,
        timeout=600,
    )
    (package / "Tests/TestAnywareAgentTests/ProcessRunnerTests.swift").write_bytes(baseline)
    (package / "Tests/TestAnywareAgentTests/OwnedProcessFixture.swift").unlink()
    build = run_bounded(
        ["swift", "build", "--build-tests"], package, out / "broad-build.log", BUILD_BOUND_SECONDS
    )
    (out / "broad-build.json").write_text(json.dumps(build, indent=2) + "\n")
    if build["exit"] != 0:
        raise SystemExit(f"broad build failed: {build}")
    return package


def main() -> int:
    parser = argparse.ArgumentParser(description=(__doc__ or "").splitlines()[0])
    parser.add_argument("out", type=Path)
    parser.add_argument("--arm", choices=("owned", "broad"), required=True)
    parser.add_argument("--expect", choices=("survives", "killed"), required=True)
    parser.add_argument("--runs", type=int, default=3)
    parser.add_argument("--baseline-rev", required=True)
    args = parser.parse_args()

    out: Path = args.out.resolve()
    if out.exists():
        raise SystemExit(f"OUT exists: {out}")
    out.mkdir(parents=True)

    baseline = baseline_test_source(args.baseline_rev)
    subjects = {
        str(p.relative_to(ROOT)): sha256(p) for p in (TEST_FILE, FIXTURE_FILE, RUNNER, SELF)
    }
    plan: dict[str, Any] = {
        "arm": args.arm,
        "expect": args.expect,
        "runs": args.runs,
        "baseline_rev": args.baseline_rev,
        "baseline_test_sha256": sha256_bytes(baseline),
        "subjects_before": subjects,
        "broad_pattern": BROAD_PATTERN,
        "control_argv": CONTROL_ARGV,
        "bounds": {
            "build_seconds": BUILD_BOUND_SECONDS,
            "suite_seconds": SUITE_BOUND_SECONDS,
            "stop_grace_seconds": STOP_GRACE_SECONDS,
        },
        "host": {"platform": platform.platform(), "uid": os.getuid()},
        "swift": tool_version(["swift", "--version"]),
        "kill_audit": kill_audit(baseline),
        "matching_before": matching_pids(),
    }
    (out / "plan.json").write_text(json.dumps(plan, indent=2) + "\n")

    if args.arm == "broad":
        if plan["matching_before"]:
            raise SystemExit(
                "refusing the broad arm: other processes of this user match "
                f"{BROAD_PATTERN!r}: {plan['matching_before']}"
            )
        package = prepare_broad(baseline, out)
        suite_argv = ["swift", "test", "--skip-build"]
    else:
        package = PACKAGE
        prebuild = run_bounded(
            ["swift", "build", "--build-tests"], package, out / "build.log", BUILD_BOUND_SECONDS
        )
        (out / "build.json").write_text(json.dumps(prebuild, indent=2) + "\n")
        if prebuild["exit"] != 0:
            raise SystemExit(f"build failed: {prebuild}")
        suite_argv = ["mise", "exec", "--", "task", "agent:macos:test"]
    suite_cwd = package if args.arm == "broad" else ROOT

    verdicts: list[str] = []
    leaks: list[dict[str, str]] = []
    try:
        for n in range(1, args.runs + 1):
            run_dir = out / f"run-{n:02d}"
            run_dir.mkdir()
            before = {row["pid"] for row in matching_pids()}
            control = subprocess.Popen(
                CONTROL_ARGV,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            selected = str(control.pid) in {row["pid"] for row in matching_pids()}
            record: dict[str, Any] = {
                "control": {
                    "pid": control.pid,
                    "started": started(control.pid),
                    "selected_by_broad_pattern": selected,
                },
            }
            suite = run_bounded(suite_argv, suite_cwd, run_dir / "suite.log", SUITE_BOUND_SECONDS)
            record["suite"] = suite | parse_suite(run_dir / "suite.log")
            record["control"] |= stop_control(control)
            after = [r for r in matching_pids() if r["pid"] not in before]
            record["matching_new_after"] = after
            verdict = "survives" if record["control"]["alive_after_suite"] else "killed"
            if not selected:
                verdict = "invalid-control"
            record["verdict"] = verdict
            verdicts.append(verdict)
            leaks.extend(after)
            (run_dir / "run.json").write_text(json.dumps(record, indent=2) + "\n")
            print(
                f"run-{n:02d}: verdict={verdict} suite_exit={suite['exit']} "
                f"failed={record['suite']['xctest_failed']}",
                flush=True,
            )
    finally:
        if args.arm == "broad":
            shutil.rmtree(package.parent, ignore_errors=True)

    subjects_after = {
        str(p.relative_to(ROOT)): sha256(p) for p in (TEST_FILE, FIXTURE_FILE, RUNNER, SELF)
    }
    ok = all(v == args.expect for v in verdicts) and subjects_after == subjects
    if args.arm == "owned" and leaks:
        ok = False
    summary = {
        "verdicts": verdicts,
        "expect": args.expect,
        "subjects_unchanged": subjects_after == subjects,
        "new_matching_after_runs": leaks,
        "pass": ok,
    }
    (out / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
