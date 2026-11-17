"""k195 deterministic exit-observation controls for the macOS agent.

The script builds the agent tests with `-Xswiftc -DEXIT_TRACE` in a fresh
temporary scratch directory. It then runs only `ExitTraceControlTests`, once
per declared run, each time with a new trace file. The four controls force
these orderings and read the trace to confirm them:

- normal: the kernel exit comes before the agent sees the exit.
- starved: the shell exits, but the inner wait block of `waitForExit` gets no
  global-queue worker, so the agent reports a timeout.
- late-term: the shell outlives the deadline and exits on TERM inside the
  3 s TERM wait.
- ignored-term: the shell ignores TERM, the 3 s wait times out and the agent
  sends SIGKILL.

The script also builds the default release agent and checks that it holds no
`ExitTrace` symbol. That check shows that the seam compiles out.

Every input, bound and hash goes into OUT/plan.json before the first build.
OUT must not exist. Each subprocess runs in its own session, and the script
kills that session's process group at its bound. The temporary scratch
directory is removed on every exit. The script exits 0 only when both builds
pass, the default agent has no seam symbol, and every declared run passes all
four controls inside its bound.
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
SOURCES = (
    PACKAGE / "Package.swift",
    PACKAGE / "Package.resolved",
    PACKAGE / "Sources/TestAnywareAgent/ExitTrace.swift",
    PACKAGE / "Sources/TestAnywareAgent/ProcessRunner.swift",
    PACKAGE / "Tests/TestAnywareAgentTests/ExitTraceControlTests.swift",
    PACKAGE / "Tests/TestAnywareAgentTests/OwnedProcessFixture.swift",
    Path(__file__).resolve(),
)
TRACE_FLAGS = ["-Xswiftc", "-DEXIT_TRACE"]
CONTROLS = (
    "testNormalExitIsObservedAfterKernelExit",
    "testStarvedPoolMissesAnExitedShell",
    "testLateExitEndsOnTerm",
    "testIgnoredTermWaitsThreeSecondsThenKills",
)
SUITE = "ExitTraceControlTests"
BUILD_BOUND_SECONDS = 900.0
RUN_BOUND_SECONDS = 120.0
STOP_GRACE_SECONDS = 2.0
XCTEST_CASE = re.compile(r"Test Case '-\[(\S+) (\w+)\]' (passed|failed) \(([\d.]+) seconds\)")
CONTROL_LINE = re.compile(r"k195-control (.+)")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def text_of(argv: list[str]) -> str:
    done = subprocess.run(argv, capture_output=True, text=True, timeout=60, check=False)
    return (done.stdout + done.stderr).strip()


def run_bounded(
    argv: list[str], log: Path, bound: float, env: dict[str, str] | None = None
) -> dict[str, Any]:
    """Runs argv in its own session and kills the group at the bound."""
    t0 = time.monotonic()
    with log.open("wb") as sink:
        proc = subprocess.Popen(
            argv,
            cwd=PACKAGE,
            stdout=sink,
            stderr=subprocess.STDOUT,
            start_new_session=True,
            env=env,
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
        "exit": code,
        "hit_bound": bounded,
        "seconds": round(time.monotonic() - t0, 3),
        "log": log.name,
        "log_sha256": sha256(log),
    }


def trace_summary(path: Path) -> dict[str, Any]:
    if not path.exists():
        return {"present": False}
    records = [json.loads(line) for line in path.read_text(encoding="utf-8").splitlines()]
    begins = [r for r in records if r.get("event") == "begin"]
    return {
        "present": True,
        "sha256": sha256(path),
        "records": len(records),
        "begins": len(begins),
        "marked_begins": sum("k195-" in str(r.get("command", "")) for r in begins),
        "loop_errors": sum(r.get("event") == "trace-loop-error" for r in records),
        "missed_watches": sum(r.get("event") == "watch-missed" for r in records),
    }


def judge_run(log: Path, trace: Path, run: dict[str, Any]) -> dict[str, Any]:
    text = log.read_text(encoding="utf-8", errors="replace")
    cases = {m[2]: m[3] for m in XCTEST_CASE.finditer(text) if m[1].endswith(SUITE)}
    summary = trace_summary(trace)
    problems: list[str] = []
    if run["hit_bound"]:
        problems.append("hit bound")
    if run["exit"] != 0:
        problems.append(f"exit {run['exit']}")
    for name in CONTROLS:
        if cases.get(name) != "passed":
            problems.append(f"{name}: {cases.get(name, 'absent')}")
    if summary.get("marked_begins") != len(CONTROLS):
        problems.append(f"marked begins {summary.get('marked_begins')}")
    if summary.get("loop_errors") or summary.get("missed_watches"):
        problems.append("trace loop error or missed watch")
    return {
        "cases": cases,
        "orderings": CONTROL_LINE.findall(text),
        "trace": summary,
        "problems": problems,
        "verdict": "pass" if not problems else "fail",
    }


def default_has_no_seam(binary: Path) -> dict[str, Any]:
    if not binary.exists():
        return {"binary": None, "seam_symbols": None, "verdict": "fail"}
    done = subprocess.run(["nm", "-j", str(binary)], capture_output=True, text=True, timeout=60)
    hits = [s for s in done.stdout.splitlines() if "ExitTrace" in s]
    ok = done.returncode == 0 and not hits
    return {
        "binary_sha256": sha256(binary),
        "nm_exit": done.returncode,
        "seam_symbols": len(hits),
        "verdict": "pass" if ok else "fail",
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="k195 exit-observation controls")
    parser.add_argument("out", type=Path)
    parser.add_argument("--runs", type=int, default=3)
    args = parser.parse_args()
    out: Path = args.out.resolve()
    out.mkdir(parents=True)
    scratch = Path(tempfile.mkdtemp(prefix="k195-controls-"))
    plan = {
        "leaf": "macos-exit-observation-instrumented-k195",
        "runs": args.runs,
        "controls": list(CONTROLS),
        "bounds": {
            "build_seconds": BUILD_BOUND_SECONDS,
            "run_seconds": RUN_BOUND_SECONDS,
            "stop_grace_seconds": STOP_GRACE_SECONDS,
        },
        "scratch": str(scratch),
        "source_commit": text_of(["git", "-C", str(ROOT), "rev-parse", "HEAD"]),
        "dirty_files": text_of(["git", "-C", str(ROOT), "status", "--porcelain", "agents"]),
        "dirty_diff_sha256": hashlib.sha256(
            subprocess.run(
                ["git", "-C", str(ROOT), "diff", "HEAD", "--", "agents"],
                capture_output=True,
                timeout=60,
                check=True,
            ).stdout
        ).hexdigest(),
        "inputs_before": {str(p.relative_to(ROOT)): sha256(p) for p in SOURCES},
        "swift": text_of(["swift", "--version"]),
        "host": {"macos": platform.mac_ver()[0], "machine": platform.machine()},
        "cpus": os.cpu_count(),
    }
    (out / "plan.json").write_text(json.dumps(plan, indent=2) + "\n")
    result: dict[str, Any] = {"runs": []}
    try:
        trace_build = scratch / "trace"
        result["trace_build"] = run_bounded(
            ["swift", "build", "--build-tests", "--scratch-path", str(trace_build), *TRACE_FLAGS],
            out / "build-trace.log",
            BUILD_BOUND_SECONDS,
        )
        default_build = scratch / "default"
        result["default_build"] = run_bounded(
            [
                "swift",
                "build",
                "-c",
                "release",
                "--product",
                "testanyware-agent",
                "--scratch-path",
                str(default_build),
            ],
            out / "build-default.log",
            BUILD_BOUND_SECONDS,
        )
        result["default_seam"] = default_has_no_seam(default_build / "release/testanyware-agent")
        if result["trace_build"]["exit"] == 0:
            for index in range(1, args.runs + 1):
                trace = out / f"trace-{index:02d}.jsonl"
                log = out / f"run-{index:02d}.log"
                env = dict(os.environ, TESTANYWARE_EXIT_TRACE=str(trace))
                run = run_bounded(
                    [
                        "swift",
                        "test",
                        "--skip-build",
                        "--scratch-path",
                        str(trace_build),
                        *TRACE_FLAGS,
                        "--filter",
                        SUITE,
                    ],
                    log,
                    RUN_BOUND_SECONDS,
                    env,
                )
                run.update(judge_run(log, trace, run))
                result["runs"].append(run)
    except Exception as error:  # noqa: BLE001 - record and still clean up
        result["error"] = repr(error)
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
        result["scratch_removed"] = not scratch.exists()
        result["inputs_after"] = {str(p.relative_to(ROOT)): sha256(p) for p in SOURCES}
    passed = (
        "error" not in result
        and result.get("trace_build", {}).get("exit") == 0
        and result.get("default_build", {}).get("exit") == 0
        and result.get("default_seam", {}).get("verdict") == "pass"
        and len(result["runs"]) == args.runs
        and all(r["verdict"] == "pass" for r in result["runs"])
        and result["inputs_after"] == plan["inputs_before"]
        and result["scratch_removed"]
    )
    result["verdict"] = "pass" if passed else "fail"
    (out / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"verdict": result["verdict"], "out": str(out)}))
    return 0 if passed else 1


if __name__ == "__main__":
    sys.exit(main())
