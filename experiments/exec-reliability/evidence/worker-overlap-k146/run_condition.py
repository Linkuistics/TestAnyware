"""Bound frozen diagnostic checks and retain every result, including failures."""

import argparse
import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

REPO = Path(__file__).resolve().parents[4]


def hashes(paths: list[Path]) -> dict[str, str]:
    return {str(path.resolve()): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("mode", choices=("controls", "full", "static"))
    parser.add_argument("--select", default="production_worker_retirement_overlaps_owned_teardown")
    args = parser.parse_args()
    here, output = args.inputs.resolve(), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    python = sys.executable
    sources = sorted(here.glob("*.py"))
    tooling = sorted(Path(__file__).parent.glob("*.py"))
    commands: list[tuple[str, tuple[str, ...], float]] = []
    if args.mode != "static":
        commands.append(
            (
                "pytest",
                (
                    python,
                    "-m",
                    "pytest",
                    "-vv",
                    *(("-k", args.select) if args.mode == "controls" else ()),
                    f"--junitxml={output / 'pytest.xml'}",
                ),
                120 if args.mode == "full" else 60,
            )
        )
    if args.mode in {"full", "static"}:
        for name, argv in (
            ("ruff", ("ruff", "check", *map(str, sources + tooling))),
            ("format", ("ruff", "format", "--check", *map(str, sources + tooling))),
            (
                "pyright",
                (
                    "pyright",
                    "controller.py",
                    "inspection.py",
                    "worker_capture.py",
                    "peer.py",
                    "preflight.py",
                    "self_check.py",
                    "turns.py",
                    *map(str, (path for path in tooling if not path.name.startswith("test_"))),
                ),
            ),
        ):
            commands.append((name, (python, "-m", *argv), 30))
    subjects = (
        sources
        + tooling
        + [
            here / "pyproject.toml",
            here / "uv.lock",
            here / "Taskfile.yml",
            REPO / "Taskfile.yml",
            REPO / "docs/specs/diagnostic-process-inspection.md",
            Path(__file__).with_name("comparison-plan.json"),
            Path(python),
            Path("/bin/sh"),
            Path("/bin/sleep"),
            Path("/usr/bin/true"),
            Path("/usr/bin/false"),
            Path("/bin/ps"),
            Path("/bin/dd"),
        ]
    )
    before = hashes(subjects)
    (output / "declaration.json").write_text(
        json.dumps(
            {
                "commands": commands,
                "subjects_before": before,
                "timeout_cleanup": "TERM 2 seconds, then KILL/reap 2 seconds",
            },
            indent=2,
        )
        + "\n"
    )
    receipts: list[dict[str, object]] = []
    environment = {**os.environ, "DIAGNOSTIC_EVIDENCE": str(output / "runs")}
    for name, argv, budget in commands:
        started = time.monotonic()
        timed_out = False
        code = None
        with (
            (output / f"{name}.stdout").open("xb") as stdout,
            (output / f"{name}.stderr").open("xb") as stderr,
        ):
            process = subprocess.Popen(
                argv,
                cwd=here,
                env=environment,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )
            try:
                code = process.wait(timeout=budget)
            except subprocess.TimeoutExpired:
                timed_out = True
                for sig in (signal.SIGTERM, signal.SIGKILL):
                    os.killpg(process.pid, sig)
                    try:
                        code = process.wait(timeout=2)
                        break
                    except subprocess.TimeoutExpired:
                        pass
        receipts.append(
            {
                "name": name,
                "pid": process.pid,
                "pgid": process.pid,
                "returncode": code,
                "reaped": code is not None,
                "timed_out": timed_out,
                "elapsed": time.monotonic() - started,
            }
        )
        print(f"{name}: exit {code}; evidence {output}", flush=True)
        if code is None:
            break
    after = hashes(subjects)
    ok = before == after and all(
        item["returncode"] == 0 and not item["timed_out"] and item["reaped"] for item in receipts
    )
    (output / "verdict.json").write_text(
        json.dumps(
            {
                "ok": ok,
                "checks": receipts,
                "subjects_before": before,
                "subjects_after": after,
                "artifact_sha256": hashes(
                    sorted(path for path in output.rglob("*") if path.is_file())
                ),
            },
            indent=2,
        )
        + "\n"
    )
    raise SystemExit(0 if ok else 1)


if __name__ == "__main__":
    main()
