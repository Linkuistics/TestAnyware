"""Frozen macOS disconnect cases on the local host (leaf macos-disconnect-availability-k13).

    disconnect_k13_host.py OUT

The script builds one subject into a temporary directory outside OUT: a copy of
`agents/macos` whose `main.swift` binds 127.0.0.1 on the port in its first
argument. `AgentServer.swift` and `ProcessRunner.swift` stay byte-identical,
and the script checks that. The checkout CLI and app come from
`cli-rs/target/debug`. The Taskfile builds them first.

The script freezes the cases, the bounds, the subject digests and the plan in
OUT/inventory.json before the first case runs. Then it runs one controller plan
once. The plan has one call: `disconnect_k13_seam.py`. No call retries.

After the plan, the script audits the host for owned leftovers: a binary inside
the build directory, or the payload sleep or payload shell of a token of this run.
A process that only mentions the directory or a token is not owned. The script
checks the start time and command of each match again, then kills and records it. Then
it removes the build directory. `disconnect_k13_check.py` judges the evidence.
This script judges nothing beyond the controller verdict.
"""

from __future__ import annotations

import os
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import uuid
from dataclasses import asdict
from pathlib import Path
from typing import Any

from controller import Call, Plan, execute_outcome, write_json
from disconnect_k13_seam import (
    ACT_AFTER,
    CASES,
    FOLLOW_BOUND,
    GUEST_TIMEOUT,
    KILL_WAIT,
    OBSERVE_BOUND,
    ORDINARY,
    ORDINARY_TIMEOUT,
    ORIGINAL_BOUND,
    TERM_GRACE,
    owned,
    token,
)
from outcomes_k12 import LOOPBACK_MAIN, run_build, run_tag, sha256, tool_version
from outcomes_k12_seam import current, processes

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
AGENT_PACKAGE = REPO / "agents/macos"
RUNNER = AGENT_PACKAGE / "Sources/TestAnywareAgent/ProcessRunner.swift"
SERVER = AGENT_PACKAGE / "Sources/testanyware-agent/AgentServer.swift"
SEAM = HERE / "disconnect_k13_seam.py"
CLI = REPO / "cli-rs/target/debug/testanyware"
APP = REPO / "cli-rs/target/debug/testanyware-app"
LOCKFILES = (REPO / "cli-rs/Cargo.lock", AGENT_PACKAGE / "Package.resolved", HERE / "uv.lock")

# Seven cases, each at most OBSERVE_BOUND + ORIGINAL_BOUND seconds, plus server starts.
SEAM_BUDGET = 900.0


def build(work: Path, out: Path) -> tuple[Path, dict[str, Any]]:
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
    return agent, record


def audit(work: Path, tag: str | None) -> list[dict[str, Any]]:
    """Record and kill each owned leftover: a build binary or a payload of this run."""
    tokens = tuple(token(case, tag) for case in CASES) if tag else ()
    found: list[dict[str, Any]] = []
    for pid, start, command in processes():
        if pid == os.getpid() or not owned(command, tokens, str(work)):
            continue
        if current(pid) != (start, command):
            found.append({"pid": pid, "start": start, "command": command, "killed": False})
            continue
        try:
            os.kill(pid, signal.SIGKILL)
            killed = True
        except ProcessLookupError:
            killed = False
        found.append({"pid": pid, "start": start, "command": command, "killed": killed})
    return found


def plan(python: str, agent: Path) -> Plan:
    run_id = uuid.uuid4().hex
    argv = (python, "-E", "-s", str(SEAM), run_tag(run_id), str(agent), str(APP), str(CLI))
    subjects = (
        Path(__file__),
        SEAM,
        HERE / "controller.py",
        HERE / "outcomes_k12.py",
        HERE / "outcomes_k12_seam.py",
        HERE / "disconnect_k13_check.py",
        RUNNER,
        SERVER,
        CLI,
        APP,
        agent,
        *(path for path in LOCKFILES if path.exists()),
    )
    return Plan(run_id, (Call("host", argv, SEAM_BUDGET),), subjects=subjects)


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: disconnect_k13_host.py OUT")
    out = Path(sys.argv[1]).resolve()
    for path in (CLI, APP):
        if not path.exists():
            raise SystemExit(f"unavailable prerequisite: {path} (run task cli:build app:build)")
    out.mkdir(parents=True, exist_ok=False)
    work = Path(tempfile.mkdtemp(prefix="k13-build-"))
    summary: dict[str, Any] = {"build_dir": str(work)}
    tag: str | None = None
    try:
        agent, built = build(work, out)
        frozen = plan(sys.executable, agent)
        tag = run_tag(frozen.run_id)
        write_json(
            out / "inventory.json",
            {
                "leaf": "macos-disconnect-availability-k13",
                "rule": "one plan, run once; no retry; failed samples stay",
                "host": {
                    "sw_vers": tool_version(["/usr/bin/sw_vers"]),
                    "machine": platform.machine(),
                    "kernel": platform.release(),
                },
                "run_tag": tag,
                "commit": tool_version(["/usr/bin/git", "-C", str(REPO), "rev-parse", "HEAD"]),
                "tools": {
                    "swift": tool_version(["/usr/bin/swift", "--version"]),
                    "cargo": tool_version(["cargo", "--version"]),
                    "python": sys.version,
                },
                "lockfiles": {
                    str(path.relative_to(REPO)): sha256(path) for path in LOCKFILES if path.exists()
                },
                "sources": {
                    str(path.relative_to(REPO)): sha256(path)
                    for path in (RUNNER, SERVER, SEAM, Path(__file__).resolve())
                },
                "executables": {
                    "cli": {"path": str(CLI), "sha256": sha256(CLI)},
                    "app": {"path": str(APP), "sha256": sha256(APP)},
                },
                "build": built,
                "bounds": {
                    "guest_timeout": GUEST_TIMEOUT,
                    "act_after": ACT_AFTER,
                    "follow_bound": FOLLOW_BOUND,
                    "ordinary_timeout": ORDINARY_TIMEOUT,
                    "term_grace": TERM_GRACE,
                    "kill_wait": KILL_WAIT,
                    "observe_bound": OBSERVE_BOUND,
                    "original_bound": ORIGINAL_BOUND,
                    "seam_budget": SEAM_BUDGET,
                },
                "ordinary": ORDINARY,
                "cases": [asdict(case) | {"token": token(case, tag)} for case in CASES],
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
        summary["leftovers"] = audit(work, tag)
        shutil.rmtree(work, ignore_errors=True)
        summary["build_dir_removed"] = not work.exists()
        write_json(out / "summary.json", summary)
    print(f"verdict {summary.get('verdict')} run {summary.get('run')}", flush=True)
    if summary.get("verdict") is not True or summary["leftovers"]:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
