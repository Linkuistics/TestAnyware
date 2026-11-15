"""Run frozen full suites or fork-failure controls under independent checker bounds."""

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
    return {
        path.resolve().relative_to(REPO).as_posix()
        if path.resolve().is_relative_to(REPO)
        else str(path): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in paths
    }


def fixture_audits(output: Path, checker_pid: int) -> list[dict[str, object]]:
    """Revalidate the recorded exact fixture before any timeout cleanup signal."""
    audits: list[dict[str, object]] = []
    for path in sorted((output / "runs").glob("owned-termination-*.json")):
        owned = json.loads(path.read_text())
        assert owned["checker_pid"] == checker_pid
        identity = owned["identity"]
        pid = identity["pid"]

        def inspect(pid: int = pid) -> subprocess.CompletedProcess[str]:
            return subprocess.run(
                ["/bin/ps", "-p", str(pid), "-o", "pid=,pgid=,stat=,lstart=,command="],
                capture_output=True,
                text=True,
                timeout=1,
            )

        current = inspect()
        initial = current.stdout
        signaled = False
        if current.returncode == 0 and not current.stderr:
            before = identity["stdout"].split(maxsplit=3)
            after = current.stdout.split(maxsplit=3)
            # PID, PGID, start time and unique fixture command must all agree.
            if len(before) == len(after) == 4 and before[:2] == after[:2] and before[3] == after[3]:
                try:
                    os.kill(pid, signal.SIGKILL)
                    signaled = True
                except ProcessLookupError:
                    pass
                deadline = time.monotonic() + 2
                while time.monotonic() < deadline:
                    current = inspect()
                    if current.returncode == 1 and not current.stdout and not current.stderr:
                        break
                    time.sleep(0.01)
        absent = current.returncode == 1 and not current.stdout and not current.stderr
        audits.append(
            {
                "journal": str(path),
                "pid": pid,
                "initial": initial,
                "signaled": signaled,
                "absent": absent,
                "after": current.stdout,
                "stderr": current.stderr,
                "remedy": None
                if absent
                else "Revalidate retained exact fixture identity before cleanup",
            }
        )
    return audits


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("inputs", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("mode", choices=("controls", "full"))
    parser.add_argument("--select", help="Override the controls pytest expression")
    parser.add_argument("--pytest-budget", type=float, help="Fault-injection checker budget")
    args = parser.parse_args()
    here = args.inputs.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    python = sys.executable
    sources = sorted(here.glob("*.py"))
    full = args.mode == "full"
    commands: list[tuple[str, tuple[str, ...], float]] = [
        (
            "pytest",
            (
                python,
                "-m",
                "pytest",
                "-vv",
                *(
                    (
                        "-k",
                        args.select or ("fork_failure or fork_eof"),
                    )
                    if not full
                    else ()
                ),
                f"--junitxml={output / 'pytest.xml'}",
            ),
            args.pytest_budget if args.pytest_budget is not None else (120 if full else 60),
        )
    ]
    if full:
        commands.extend(
            [
                (
                    "ruff",
                    (
                        python,
                        "-m",
                        "ruff",
                        "check",
                        *map(str, sources),
                        str(Path(__file__).resolve()),
                        str(Path(__file__).with_name("prepare.py")),
                        str(Path(__file__).with_name("preserve.py")),
                        str(Path(__file__).with_name("audit.py")),
                    ),
                    30,
                ),
                (
                    "format",
                    (
                        python,
                        "-m",
                        "ruff",
                        "format",
                        "--check",
                        *map(str, sources),
                        str(Path(__file__).resolve()),
                        str(Path(__file__).with_name("prepare.py")),
                        str(Path(__file__).with_name("preserve.py")),
                        str(Path(__file__).with_name("audit.py")),
                    ),
                    30,
                ),
                (
                    "pyright",
                    (
                        python,
                        "-m",
                        "pyright",
                        "controller.py",
                        "inspection.py",
                        "worker_capture.py",
                        "peer.py",
                        "preflight.py",
                        "self_check.py",
                        str(Path(__file__).resolve()),
                        str(Path(__file__).with_name("prepare.py")),
                        str(Path(__file__).with_name("preserve.py")),
                        str(Path(__file__).with_name("audit.py")),
                    ),
                    30,
                ),
            ]
        )
    subjects = [
        *sources,
        here / "pyproject.toml",
        here / "uv.lock",
        *((here / "Taskfile.yml",) if (here / "Taskfile.yml").exists() else ()),
        Path(__file__).resolve().parents[4] / "Taskfile.yml",
        Path(__file__),
        Path(__file__).with_name("prepare.py"),
        Path(__file__).with_name("preserve.py"),
        Path(__file__).with_name("audit.py"),
        Path(__file__).resolve().parents[4] / "docs/specs/diagnostic-process-inspection.md",
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
        fixtures = fixture_audits(output, process.pid) if name == "pytest" else []
        receipts.append(
            {
                "fixtures": fixtures,
                "fixtures_ok": all(fixture["absent"] is True for fixture in fixtures),
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
        item["returncode"] == 0
        and item["timed_out"] is False
        and item["reaped"] is True
        and item["fixtures_ok"] is True
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
