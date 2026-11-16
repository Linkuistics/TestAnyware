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
until this script reaps it. The script stops and reaps the control on every
exit after it starts, and it records any error and any cleanup failure.

The restored broad cleanup signals every process of this user that matches the
pattern. A scan cannot make that safe, because a process can start between the
scan and the signal. So the broad arm runs only with `--isolated-host`, which
declares that no other process of this user can match. On that host, the
script still refuses a match that it sees before the build or before a run. A
failed scan stops the script, because a failed scan does not show absence.

Every input, bound and hash goes into OUT/plan.json before the first run. OUT
must not exist. A run counts only when the suite finishes inside its bound and
both fixture cases finish. In the owned arm, the run also needs a cleanup
receipt from each fixture case and no survivor. Otherwise its verdict is
`invalid-exercise`. The script exits 0 only when at least one run happens,
every run gives the `--expect` verdict and the owned arm leaves no matching
process of its own behind. Suite failures are recorded per test case. They are
not part of the verdict.
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
FIXTURE_CASES = (
    "ProcessRunnerTests.testDescendantHoldingFDDoesNotWedgeAfterParentExits",
    "ProcessRunnerTests.testDetachedRunReturnsImmediately",
)


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
    """Lists the processes of this user that the broad pattern selects.

    `pgrep` exits 1 when nothing matches. Any other failure raises, because a
    failed scan does not show that no process matches.
    """
    done = subprocess.run(
        ["pgrep", "-U", str(os.getuid()), "-lf", BROAD_PATTERN],
        capture_output=True,
        text=True,
        timeout=10,
        check=False,
    )
    if done.returncode not in (0, 1) or (done.returncode == 0) != bool(done.stdout.strip()):
        raise RuntimeError(f"pgrep failed: exit {done.returncode}, stderr {done.stderr.strip()!r}")
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


def exercise_problems(arm: str, suite: dict[str, Any]) -> list[str]:
    """Explains why a run did not exercise the fixture cleanup, if it did not.

    A control that survives a suite which never reached the fixture cases is
    not evidence. The owned arm also needs a cleanup receipt from each case.
    """
    problems: list[str] = []
    if suite["hit_bound"]:
        problems.append(f"suite stopped at the {SUITE_BOUND_SECONDS} s bound")
    finished = [c["case"] for c in suite["xctest_cases"]]
    for case in FIXTURE_CASES:
        if not any(name.endswith(case) for name in finished):
            problems.append(f"{case} did not finish")
    if arm == "owned":
        receipts = [line for line in suite["fixture_lines"] if not line.startswith("unresolved")]
        if len(receipts) < len(FIXTURE_CASES):
            problems.append(f"{len(receipts)} cleanup receipts, expected {len(FIXTURE_CASES)}")
        problems.extend(
            f"fixture cleanup: {line}"
            for line in suite["fixture_lines"]
            if line.startswith("unresolved") or line.endswith("survived")
        )
    return problems


def stop_control_safely(proc: subprocess.Popen[bytes]) -> dict[str, Any]:
    """Stops and reaps the control, and records a cleanup failure instead of
    raising it, so the error that ended the run stays the one that propagates.
    """
    try:
        return stop_control(proc)
    except Exception as exc:  # noqa: BLE001 - the failure is the evidence
        return {"cleanup_error": repr(exc), "alive_after_cleanup": proc.poll() is None}


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
    parser.add_argument(
        "--isolated-host",
        action="store_true",
        help="declare that no other process of this user can match; the broad arm needs it",
    )
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be at least 1")
    if args.arm == "broad" and not args.isolated_host:
        parser.error(
            "the broad arm restores an unrestricted `pkill -f`, which a scan cannot make "
            "safe on a shared host; pass --isolated-host only on a host where no other "
            "process of this user can match"
        )

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
        "isolated_host": args.isolated_host,
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
            matching = matching_pids()
            if args.arm == "broad" and matching:
                raise SystemExit(
                    f"refusing broad run {n}: other processes of this user match "
                    f"{BROAD_PATTERN!r}: {matching}"
                )
            before = {row["pid"] for row in matching}
            control = subprocess.Popen(
                CONTROL_ARGV,
                stdin=subprocess.DEVNULL,
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True,
            )
            record: dict[str, Any] = {"control": {"pid": control.pid}, "matching_before": matching}
            try:
                record["control"]["started"] = started(control.pid)
                selected = str(control.pid) in {row["pid"] for row in matching_pids()}
                record["control"]["selected_by_broad_pattern"] = selected
                suite = run_bounded(
                    suite_argv, suite_cwd, run_dir / "suite.log", SUITE_BOUND_SECONDS
                )
                record["suite"] = suite | parse_suite(run_dir / "suite.log")
            except BaseException as exc:
                record["error"] = repr(exc)
                raise
            finally:
                # Runs on every exit after the control starts. A cleanup
                # failure is recorded beside the error that ended the run.
                record["control"] |= stop_control_safely(control)
                if "error" in record:
                    (run_dir / "run.json").write_text(json.dumps(record, indent=2) + "\n")
            if "cleanup_error" in record["control"]:
                (run_dir / "run.json").write_text(json.dumps(record, indent=2) + "\n")
                raise RuntimeError(f"control cleanup failed: {record['control']}")
            after = [r for r in matching_pids() if r["pid"] not in before]
            record["matching_new_after"] = after
            problems = exercise_problems(args.arm, record["suite"])
            record["exercise_problems"] = problems
            verdict = "survives" if record["control"]["alive_after_suite"] else "killed"
            if problems:
                verdict = "invalid-exercise"
            if not selected:
                verdict = "invalid-control"
            record["verdict"] = verdict
            verdicts.append(verdict)
            leaks.extend(after)
            (run_dir / "run.json").write_text(json.dumps(record, indent=2) + "\n")
            print(
                f"run-{n:02d}: verdict={verdict} suite_exit={suite['exit']} "
                f"failed={record['suite']['xctest_failed']} problems={problems}",
                flush=True,
            )
    finally:
        if args.arm == "broad":
            shutil.rmtree(package.parent, ignore_errors=True)

    subjects_after = {
        str(p.relative_to(ROOT)): sha256(p) for p in (TEST_FILE, FIXTURE_FILE, RUNNER, SELF)
    }
    ok = bool(verdicts) and all(v == args.expect for v in verdicts)
    ok = ok and subjects_after == subjects
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
