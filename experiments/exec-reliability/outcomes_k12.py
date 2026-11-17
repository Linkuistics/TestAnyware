"""Frozen macOS exec outcomes on the local host (leaf macos-exec-outcomes-k12).

    outcomes_k12.py OUT

The script builds four subjects into a temporary directory outside OUT:

1. `probe-default`: the unchanged `ProcessRunner.swift` with
   `evidence/macos-exec-outcomes-k12/probe/main.swift`.
2. `probe-traced`: the same source with `-DEXIT_TRACE` and `ExitTrace.swift`.
3. `agent`: a copy of `agents/macos` whose `main.swift` binds 127.0.0.1 on the
   port in its first argument. `AgentServer.swift` and `ProcessRunner.swift` stay
   byte-identical, and the script checks that.
4. The checkout CLI and app come from `cli-rs/target/debug`. The Taskfile builds
   them first.

The script freezes the cases, the bounds, the subject digests and the plan in
OUT/inventory.json before the first case runs. Then it runs one controller plan
once. Each seam of `outcomes_k12_seam.py` is one call. No call retries.

After the plan, the script audits the host for owned leftovers: any process whose
command names the build directory or a k12 sleep token. It kills and records
each one. Then it removes the build directory. `outcomes_k12_check.py` judges the
evidence. This script judges nothing beyond the controller verdict.
"""

from __future__ import annotations

import hashlib
import os
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import uuid
from dataclasses import asdict
from pathlib import Path
from typing import Any

from controller import Call, Plan, execute_outcome, write_json
from outcomes_k12_seam import (
    CLI_MARGIN,
    GRACE,
    GUEST_TIMEOUT,
    HOST_MARGIN,
    KILL_WAIT,
    ORDINARY_TIMEOUT,
    READY_BOUND,
    SEAMS,
    STOP_BOUND,
    cases,
    processes,
)

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
AGENT_PACKAGE = REPO / "agents/macos"
SOURCES = AGENT_PACKAGE / "Sources/TestAnywareAgent"
RUNNER = SOURCES / "ProcessRunner.swift"
TRACE = SOURCES / "ExitTrace.swift"
SERVER = AGENT_PACKAGE / "Sources/testanyware-agent/AgentServer.swift"
PROBE = HERE / "evidence/macos-exec-outcomes-k12/probe/main.swift"
SEAM = HERE / "outcomes_k12_seam.py"
CLI = REPO / "cli-rs/target/debug/testanyware"
APP = REPO / "cli-rs/target/debug/testanyware-app"
LOCKFILES = (REPO / "cli-rs/Cargo.lock", AGENT_PACKAGE / "Package.resolved", HERE / "uv.lock")

BUILD_BOUND = 600.0  # one build step must finish within this
REFERENCE_BUDGET = 60.0
PROBE_BUDGET = 300.0
HTTP_CLI_BUDGET = 600.0
TOKEN = re.compile(r"^sleep 30\.[1-6][12]$")

LOOPBACK_MAIN = """\
import Foundation
import Hummingbird

// k12 loopback entry point: the router of AgentServer.swift on 127.0.0.1 only.
let port = CommandLine.arguments.dropFirst().first.flatMap(Int.init) ?? 0
let app = Application(
    router: buildAgentRouter(),
    configuration: .init(address: .hostname("127.0.0.1", port: port))
)
try await app.runService()
"""


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tool_version(argv: list[str]) -> str:
    done = subprocess.run(argv, capture_output=True, text=True, timeout=60, check=False)
    return (done.stdout + done.stderr).strip()


def run_build(name: str, argv: list[str], out: Path, cwd: Path | None = None) -> dict[str, Any]:
    start = time.monotonic()
    done = subprocess.run(argv, capture_output=True, cwd=cwd, timeout=BUILD_BOUND, check=False)
    seconds = time.monotonic() - start
    (out / f"build-{name}.stdout").write_bytes(done.stdout)
    (out / f"build-{name}.stderr").write_bytes(done.stderr)
    if done.returncode != 0:
        raise SystemExit(f"build {name} failed with exit {done.returncode}")
    return {"name": name, "argv": argv, "cwd": str(cwd) if cwd else None, "seconds": seconds}


def build(work: Path, out: Path) -> tuple[dict[str, Path], list[dict[str, Any]]]:
    built: dict[str, Path] = {}
    records: list[dict[str, Any]] = []
    for name, extra in (("probe-default", []), ("probe-traced", ["-DEXIT_TRACE", str(TRACE)])):
        binary = work / name
        argv = [
            "/usr/bin/swiftc",
            "-O",
            "-swift-version",
            "5",
            "-module-name",
            "K12Probe",
            *extra,
            str(RUNNER),
            str(PROBE),
            "-o",
            str(binary),
        ]
        record = run_build(name, argv, out)
        record["binary_sha256"] = sha256(binary)
        records.append(record)
        built[name] = binary
    copy = work / "agent"
    subprocess.run(
        [
            "/usr/bin/rsync",
            "-a",
            "--exclude",
            ".build",
            "--exclude",
            "Tests",
            f"{AGENT_PACKAGE}/",
            f"{copy}/",
        ],
        check=True,
        timeout=60,
    )
    (copy / "Sources/testanyware-agent/main.swift").write_text(LOOPBACK_MAIN)
    manifest = copy / "Package.swift"
    text = manifest.read_text()
    pruned = re.sub(r"\n        \.testTarget\(.*?\n        \),", "", text, count=1, flags=re.DOTALL)
    if pruned == text:
        raise SystemExit("the agent manifest has no test target to remove")
    manifest.write_text(pruned)
    for original, copied in (
        (RUNNER, copy / "Sources/TestAnywareAgent/ProcessRunner.swift"),
        (SERVER, copy / "Sources/testanyware-agent/AgentServer.swift"),
    ):
        if original.read_bytes() != copied.read_bytes():
            raise SystemExit(f"the agent copy changed {original.name}")
    argv = [
        "/usr/bin/swift",
        "build",
        "--package-path",
        str(copy),
        "--product",
        "testanyware-agent",
    ]
    record = run_build("agent", argv, out)
    agent = copy / ".build/debug/testanyware-agent"
    record["binary_sha256"] = sha256(agent)
    record["manifest_sha256"] = sha256(manifest)
    record["main_sha256"] = sha256(copy / "Sources/testanyware-agent/main.swift")
    records.append(record)
    built["agent"] = agent
    return built, records


def audit(work: Path) -> list[dict[str, Any]]:
    """Record and kill each owned leftover: the build directory or a k12 token."""
    found: list[dict[str, Any]] = []
    for pid, start, command in processes():
        if pid == os.getpid():
            continue
        if str(work) in command or TOKEN.match(command.strip()):
            try:
                os.kill(pid, signal.SIGKILL)
                killed = True
            except ProcessLookupError:
                killed = False
            found.append({"pid": pid, "start": start, "command": command, "killed": killed})
    return found


def plan(python: str, built: dict[str, Path]) -> Plan:
    run_id = uuid.uuid4().hex
    seam = (python, "-I", str(SEAM))
    calls = (
        Call("reference", (*seam, "reference"), REFERENCE_BUDGET),
        Call("process", (*seam, "process", str(built["probe-default"])), PROBE_BUDGET),
        Call("traced", (*seam, "traced", str(built["probe-traced"])), PROBE_BUDGET),
        Call(
            "http-cli",
            (*seam, "http-cli", str(built["agent"]), str(APP), str(CLI)),
            HTTP_CLI_BUDGET,
        ),
    )
    subjects = (
        Path(__file__),
        SEAM,
        HERE / "controller.py",
        HERE / "outcomes_k12_check.py",
        PROBE,
        RUNNER,
        TRACE,
        SERVER,
        CLI,
        APP,
        *built.values(),
        *(path for path in LOCKFILES if path.exists()),
    )
    return Plan(run_id, calls, subjects=subjects)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: outcomes_k12.py OUT")
    out = Path(sys.argv[1]).resolve()
    for path in (CLI, APP):
        if not path.exists():
            raise SystemExit(f"unavailable prerequisite: {path} (run task cli:build app:build)")
    out.mkdir(parents=True, exist_ok=False)
    work = Path(tempfile.mkdtemp(prefix="k12-build-"))
    summary: dict[str, Any] = {"build_dir": str(work)}
    try:
        built, builds = build(work, out)
        frozen = plan(sys.executable, built)
        write_json(
            out / "inventory.json",
            {
                "leaf": "macos-exec-outcomes-k12",
                "rule": "one plan, run once; no retry; failed samples stay",
                "host": {
                    "sw_vers": tool_version(["/usr/bin/sw_vers"]),
                    "machine": platform.machine(),
                    "kernel": platform.release(),
                },
                "commit": tool_version(["/usr/bin/git", "-C", str(REPO), "rev-parse", "HEAD"]),
                "tools": {
                    "swiftc": tool_version(["/usr/bin/swiftc", "--version"]),
                    "cargo": tool_version(["cargo", "--version"]),
                    "python": sys.version,
                },
                "lockfiles": {
                    str(path.relative_to(REPO)): sha256(path) for path in LOCKFILES if path.exists()
                },
                "sources": {
                    str(path.relative_to(REPO)): sha256(path)
                    for path in (RUNNER, TRACE, SERVER, PROBE, SEAM, Path(__file__).resolve())
                },
                "executables": {
                    "cli": {"path": str(CLI), "sha256": sha256(CLI)},
                    "app": {"path": str(APP), "sha256": sha256(APP)},
                },
                "builds": builds,
                "bounds": {
                    "guest_timeout": GUEST_TIMEOUT,
                    "ordinary_timeout": ORDINARY_TIMEOUT,
                    "host_margin": HOST_MARGIN,
                    "cli_margin": CLI_MARGIN,
                    "grace": GRACE,
                    "kill_wait": KILL_WAIT,
                    "ready_bound": READY_BOUND,
                    "stop_bound": STOP_BOUND,
                    "build_bound": BUILD_BOUND,
                },
                "cases": {
                    seam: [
                        asdict(case) | {"stdout": case.stdout.hex(), "stderr": case.stderr.hex()}
                        for case in cases(seam)
                    ]
                    for seam in SEAMS
                },
                "plan": asdict(frozen),
            },
        )
        outcome = execute_outcome(frozen, out)
        summary.update(
            verdict=outcome.verdict,
            run=str(outcome.run_dir),
            ledger=outcome.ledger,
            residuals=outcome.residuals,
        )
    finally:
        summary["leftovers"] = audit(work)
        shutil.rmtree(work, ignore_errors=True)
        summary["build_dir_removed"] = not work.exists()
        write_json(out / "summary.json", summary)
    print(f"verdict {summary.get('verdict')} run {summary.get('run')}", flush=True)
    if summary.get("verdict") is not True or summary["leftovers"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
