"""Retain the checker verdict separately from the controller's raw outcomes."""

import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
PYTEST_BUDGET = 300


def hashes(paths: list[Path]) -> dict[str, str]:
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}


def main() -> None:
    output = Path(sys.argv[1]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    python = sys.executable
    controls = sys.argv[2:] == ["--controls-only"]
    sources = sorted(HERE.glob("*.py"))
    source_args = tuple(map(str, sources))
    commands: list[tuple[str, tuple[str, ...], int]] = (
        []
        if controls
        else [
            ("lint", (python, "-m", "ruff", "check", *source_args), 30),
            ("format", (python, "-m", "ruff", "format", "--check", *source_args), 30),
            ("types", (python, "-m", "pyright"), 30),
        ]
    )
    commands.append(
        (
            "pytest",
            (
                python,
                "-m",
                "pytest",
                "-vv",
                f"--junitxml={output / 'pytest.xml'}",
                *(("-k", "review_") if controls else ()),
            ),
            # The suite took 124 s and 127 s in k19 and 126 s in k223 on the
            # owner's host, so the earlier 120 s budget could not pass. This
            # bound only keeps the runner finite. Each test keeps its own bounds.
            PYTEST_BUDGET,
        )
    )
    subjects = [
        *sources,
        HERE / "pyproject.toml",
        HERE / "uv.lock",
        HERE.parents[1] / "Taskfile.yml",
        Path(python),
        Path("/bin/ps"),
        Path("/bin/sh"),
        Path("/bin/sleep"),
        Path("/usr/bin/true"),
        Path("/usr/bin/false"),
    ]
    before = hashes(subjects)
    declaration = {
        "runner_pid": os.getpid(),
        "commands": commands,
        "subjects_before": before,
        "bounds": "TERM 2 seconds then KILL/reap 2 seconds per checker timeout",
    }
    (output / "declaration.json").write_text(json.dumps(declaration, indent=2) + "\n")
    receipts: list[dict[str, object]] = []
    environment = {**os.environ, "DIAGNOSTIC_EVIDENCE": str(output / "runs")}
    for name, argv, budget in commands:
        start = time.monotonic()
        timed_out = False
        code: int | None = None
        reaped = False
        with (
            (output / f"{name}.stdout").open("xb") as stdout,
            (output / f"{name}.stderr").open("xb") as stderr,
        ):
            process = subprocess.Popen(
                argv,
                cwd=HERE,
                env=environment,
                stdout=stdout,
                stderr=stderr,
                start_new_session=True,
            )
            try:
                code = process.wait(timeout=budget)
            except subprocess.TimeoutExpired:
                timed_out = True
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    code = process.wait(timeout=2)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    try:
                        code = process.wait(timeout=2)
                    except subprocess.TimeoutExpired:
                        pass
            reaped = code is not None
        receipts.append(
            {
                "name": name,
                "argv": argv,
                "pid": process.pid,
                "pgid": process.pid,
                "returncode": code,
                "timed_out": timed_out,
                "reaped": reaped,
                "residual_remedy": None
                if reaped
                else (
                    "Independently inspect recorded checker PID/PGID and start identity; "
                    "terminate/reap only the revalidated owned checker. "
                    "Later checkers were not run."
                ),
                "elapsed": time.monotonic() - start,
            }
        )
        print(f"{name}: exit {code}; evidence {output}", flush=True)
        if not reaped:
            break
    after = hashes(subjects)
    artifacts = hashes(sorted(path for path in output.rglob("*") if path.is_file()))
    ok = before == after and all(
        item["returncode"] == 0 and item["timed_out"] is False and item["reaped"] is True
        for item in receipts
    )
    verdict = {
        "ok": ok,
        "checks": receipts,
        "subjects_before": before,
        "subjects_after": after,
        "artifact_sha256": artifacts,
    }
    (output / "verdict.json").write_text(json.dumps(verdict, indent=2) + "\n")
    raise SystemExit(0 if ok else 1)


if __name__ == "__main__":
    main()
