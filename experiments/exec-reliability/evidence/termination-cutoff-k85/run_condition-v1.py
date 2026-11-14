"""Run frozen full suites or deadline controls under independent checker bounds."""

import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(sys.argv[1]).resolve()


def hashes(paths: list[Path]) -> dict[str, str]:
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}


def main() -> None:
    output = Path(sys.argv[2]).resolve()
    output.mkdir(parents=True, exist_ok=False)
    python = sys.executable
    sources = sorted(HERE.glob("*.py"))
    full = sys.argv[3:] == ["full"]
    commands = [
        (
            "pytest",
            (
                python,
                "-m",
                "pytest",
                "-vv",
                *(("-k", "termination_cutoff") if not full else ()),
                f"--junitxml={output / 'pytest.xml'}",
            ),
            120 if full else 20,
        )
    ]
    subjects = [
        *sources,
        HERE / "pyproject.toml",
        HERE / "uv.lock",
        Path(__file__).resolve().parents[4] / "Taskfile.yml",
        Path(__file__),
        Path(__file__).with_name("prior-evidence.json"),
        Path("/bin/sh"),
        Path("/bin/sleep"),
        Path("/usr/bin/false"),
        Path("/usr/bin/true"),
        Path(python),
        Path("/bin/ps"),
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
