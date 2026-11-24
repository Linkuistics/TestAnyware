"""Frozen Linux exec outcomes and timeout lifecycle in containers (leaf linux-timeout-lifecycle-k5).

    linux_k5.py OUT

The script runs the unchanged Linux agent in fresh containers of one pinned
image. No clone and no VM take part. The checkout CLI and app come from
`cli-rs/target/debug`. The Taskfile builds them first.

The script freezes the cases, the bounds, the image digest, the subject digests
and the plan in OUT/inventory.json before the first case runs. Then it runs one
controller plan once. Each call is one seam group of `linux_k5_seam.py`:

1. `reference`, `process` and `traced` each run in one fresh container as uid
   1000, with an init process and no network.
2. `host` starts one fresh agent container per case and host seam. It drives the
   containers with raw HTTP and with the checkout CLI through a test-mode app.

No call retries. Each container carries the label `testanyware.k5.run=TAG`. The
tag is 12 decimal digits taken from the run ID, so only this run carries it.

After the plan, the script removes each container that carries the tag and
records it. A container that the plan already removed does not appear. Then the
script lists the label again. `linux_k5_check.py` judges the evidence. This
script judges nothing beyond the controller verdict.
"""

from __future__ import annotations

import ast
import hashlib
import json
import platform
import shutil
import subprocess
import sys
import time
import uuid
from dataclasses import asdict
from pathlib import Path
from typing import Any

from controller import Call, Plan, execute_outcome, write_json
from linux_k5_seam import (
    AGENT_PORT,
    CLEANUP_BOUND,
    DISPOSAL_BOUND,
    GUEST_TIMEOUT,
    LABEL,
    OBSERVE_BOUND,
    ORDINARY_TIMEOUT,
    SEAMS,
    cases,
)
from outcomes_k12_seam import GRACE, HOST_MARGIN, KILL_WAIT, READY_BOUND, STOP_BOUND

# python:3.12-slim on linux/arm64. The daemon knows the image only by this repo digest.
IMAGE = "python@sha256:dddfd7e07f9d15aeeca61529320492139d21cac7f0070c00609243e51e4e0016"
HERE = Path(__file__).resolve().parent
REPO = HERE.parents[1]
AGENT_PACKAGE = REPO / "agents/linux/testanyware_agent"
STUB = HERE / "linux_k5_stub"
SEAM = HERE / "linux_k5_seam.py"
CLI = REPO / "cli-rs/target/debug/testanyware"
APP = REPO / "cli-rs/target/debug/testanyware-app"
LOCKFILES = (REPO / "cli-rs/Cargo.lock", HERE / "uv.lock")

INSIDE_BUDGET = 180.0  # eleven cases, four of them deadline cases of six seconds
HOST_BUDGET = 900.0  # 33 agent containers, then 33 cases with observation and cleanup
DOCKER_BOUND = 60.0  # one docker query or removal
# -I would drop the script directory from sys.path, and the seam imports its siblings.
HOST_FLAGS = ("-E", "-s", "-B")


def run_tag(run_id: str) -> str:
    """The 12-digit run tag that labels each container and ends each sleep token."""
    return f"{int(run_id, 16) % 10**12:012d}"


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def tool_version(argv: list[str]) -> str:
    done = subprocess.run(argv, capture_output=True, text=True, timeout=60, check=False)
    return (done.stdout + done.stderr).strip()


def docker(argv: list[str]) -> dict[str, Any]:
    started = time.monotonic()
    try:
        done = subprocess.run(  # noqa: S603 - docker with fixed arguments
            ["docker", *argv], capture_output=True, text=True, timeout=DOCKER_BOUND, check=False
        )
        receipt: dict[str, Any] = {
            "returncode": done.returncode,
            "stdout": done.stdout,
            "stderr": done.stderr,
        }
    except subprocess.TimeoutExpired:
        receipt = {"returncode": None, "bound_exceeded": True}
    return receipt | {"argv": argv, "seconds": time.monotonic() - started}


def labelled(tag: str) -> list[str]:
    receipt = docker(["ps", "-a", "-q", "--filter", f"label={LABEL}={tag}"])
    return str(receipt.get("stdout", "")).split()


def audit(tag: str | None) -> dict[str, Any]:
    """Remove each container that carries the tag of this run, then list the label again."""
    if tag is None:
        return {"tag": None, "found": []}
    found = labelled(tag)
    result: dict[str, Any] = {"tag": tag, "found": found}
    if found:
        result["removal"] = docker(["rm", "-f", *found])
    result["after"] = labelled(tag)
    return result


def docker_path() -> str:
    """The absolute docker executable. The controller refuses a bare name."""
    found = shutil.which("docker")
    if found is None:
        raise SystemExit("unavailable prerequisite: docker")
    return str(Path(found).absolute())


def inside_argv(docker_exe: str, tag: str, seam: str) -> tuple[str, ...]:
    return (
        docker_exe,
        "run",
        "--rm",
        "--init",
        "--network",
        "none",
        "-u",
        "1000:1000",
        "--name",
        f"k5-{tag}-inside-{seam}",
        "--label",
        f"{LABEL}={tag}",
        "-v",
        f"{REPO / 'agents/linux'}:/agent:ro",
        "-v",
        f"{STUB}:/stub:ro",
        "-v",
        f"{HERE}:/seam:ro",
        IMAGE,
        "python3",
        "-E",
        "-s",
        "-B",
        "/seam/linux_k5_seam.py",
        tag,
        "inside",
        seam,
    )


def plan(python: str) -> Plan:
    run_id = uuid.uuid4().hex
    tag = run_tag(run_id)
    docker_exe = docker_path()
    calls = tuple(
        Call(seam, inside_argv(docker_exe, tag, seam), INSIDE_BUDGET)
        for seam in ("reference", "process", "traced")
    ) + (
        Call(
            "host",
            (python, *HOST_FLAGS, str(SEAM), tag, "host", IMAGE, str(APP), str(CLI)),
            HOST_BUDGET,
        ),
    )
    subjects = (*source_paths(), CLI, APP, *(path for path in LOCKFILES if path.exists()))
    return Plan(run_id, calls, subjects=subjects)


def local_modules(roots: tuple[Path, ...]) -> list[Path]:
    """The modules of this directory that the roots import, directly or through each other."""
    seen: set[Path] = set()
    pending = list(roots)
    while pending:
        path = pending.pop()
        if path in seen:
            continue
        seen.add(path)
        for node in ast.walk(ast.parse(path.read_text())):
            if isinstance(node, ast.Import):
                names = [alias.name for alias in node.names]
            elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                names = [node.module]
            else:
                continue
            for name in names:
                candidate = HERE / f"{name.split('.')[0]}.py"
                if candidate.is_file():
                    pending.append(candidate)
    return sorted(seen)


def source_paths() -> list[Path]:
    """Each source file that shapes the run: the experiment closure, the agent and the stubs.

    The plan hashes these before and after the run, and the summary records them.
    One list serves both, so the two records cannot drift apart.
    """
    roots = (Path(__file__).resolve(), SEAM, HERE / "linux_k5_check.py")
    return [
        *local_modules(roots),
        *sorted(AGENT_PACKAGE.glob("*.py")),
        *sorted(STUB.rglob("*.py")),
    ]


def sources() -> dict[str, str]:
    return {str(path.relative_to(REPO)): sha256(path) for path in source_paths()}


def image_record() -> dict[str, Any]:
    receipt = docker(["image", "inspect", IMAGE])
    if receipt.get("returncode") != 0:
        raise SystemExit(f"unavailable prerequisite: {IMAGE} (docker pull it first)")
    inspected = json.loads(str(receipt["stdout"]))[0]
    return {
        "reference": IMAGE,
        "id": inspected.get("Id"),
        "repo_digests": inspected.get("RepoDigests"),
        "architecture": inspected.get("Architecture"),
    }


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit("usage: linux_k5.py OUT")
    out = Path(sys.argv[1]).resolve()
    for path in (CLI, APP):
        if not path.exists():
            raise SystemExit(f"unavailable prerequisite: {path} (run task cli:build app:build)")
    image = image_record()
    out.mkdir(parents=True, exist_ok=False)
    summary: dict[str, Any] = {}
    tag: str | None = None
    try:
        frozen = plan(sys.executable)
        tag = run_tag(frozen.run_id)
        write_json(
            out / "inventory.json",
            {
                "leaf": "linux-timeout-lifecycle-k5",
                "rule": "one plan, run once; no retry; failed samples stay",
                "host": {
                    "sw_vers": tool_version(["/usr/bin/sw_vers"]),
                    "machine": platform.machine(),
                    "kernel": platform.release(),
                },
                "docker": tool_version(["docker", "version", "--format", "{{json .}}"]),
                "image": image,
                "run_tag": tag,
                "commit": tool_version(["/usr/bin/git", "-C", str(REPO), "rev-parse", "HEAD"]),
                "tools": {"cargo": tool_version(["cargo", "--version"]), "python": sys.version},
                "lockfiles": {
                    str(path.relative_to(REPO)): sha256(path) for path in LOCKFILES if path.exists()
                },
                "sources": sources(),
                "executables": {
                    "cli": {"path": str(CLI), "sha256": sha256(CLI)},
                    "app": {"path": str(APP), "sha256": sha256(APP)},
                },
                "bounds": {
                    "guest_timeout": GUEST_TIMEOUT,
                    "ordinary_timeout": ORDINARY_TIMEOUT,
                    "host_margin": HOST_MARGIN,
                    "grace": GRACE,
                    "kill_wait": KILL_WAIT,
                    "ready_bound": READY_BOUND,
                    "stop_bound": STOP_BOUND,
                    "observe_bound": OBSERVE_BOUND,
                    "cleanup_bound": CLEANUP_BOUND,
                    "disposal_bound": DISPOSAL_BOUND,
                    "agent_port": AGENT_PORT,
                },
                "cases": {
                    seam: [
                        asdict(case) | {"stdout": case.stdout.hex(), "stderr": case.stderr.hex()}
                        for case in cases(seam, tag)
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
        summary["leftovers"] = audit(tag)
        write_json(out / "summary.json", summary)
    print(f"verdict {summary.get('verdict')} run {summary.get('run')}", flush=True)
    if summary.get("verdict") is not True or summary["leftovers"].get("found"):
        raise SystemExit(1)


if __name__ == "__main__":
    main()
