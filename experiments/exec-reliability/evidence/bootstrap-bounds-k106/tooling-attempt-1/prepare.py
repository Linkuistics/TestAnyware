"""Freeze diagnostic inputs, optionally with one named bootstrap regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "readiness-half": (
        "worker_capture.py",
        "deadline = self.startup_started + self.bootstrap.readiness",
        "deadline = self.startup_started + 0.5",
    ),
    "late-accept": ("worker_capture.py", "if self.readiness_finished >= deadline:", "if False:"),
    "equality-accept": (
        "worker_capture.py",
        "if self.readiness_finished >= deadline:",
        "if self.readiness_finished > deadline:",
    ),
    "overwrite-sample": (
        "worker_capture.py",
        "if self.readiness_finished is None:\n"
        "                self.readiness_finished = time.monotonic()",
        "if True:\n                self.readiness_finished = time.monotonic()",
    ),
    "overrun-true": (
        "worker_capture.py",
        "overrun = self.shutdown_finished > end",
        "overrun = True",
    ),
    "overrun-false": (
        "worker_capture.py",
        "overrun = self.shutdown_finished > end",
        "overrun = False",
    ),
    "outcome-reaped": (
        "worker_capture.py",
        'else "reaped"\n                if owner.reaped',
        'else "residual_ownership"\n                if owner.reaped',
    ),
    "outcome-residual": ("worker_capture.py", 'else "residual_ownership",', 'else "reaped",'),
    "outcome-not-created": ("worker_capture.py", 'outcome="not_created"', 'outcome="reaped"'),
    "readiness-outcome": (
        "worker_capture.py",
        'self.readiness_outcome = "expired" if isinstance(error, TimeoutError) else "failed"',
        'self.readiness_outcome = "failed"',
    ),
    "readiness-overrun": (
        "worker_capture.py",
        "self.readiness_finished > self.startup_deadline",
        "False",
    ),
    "readiness-lateness": (
        "worker_capture.py",
        "max(0.0, self.readiness_finished - self.startup_deadline)",
        "0.0",
    ),
    "shutdown-lateness": (
        "worker_capture.py",
        "lateness_seconds=max(0.0, self.shutdown_finished - end)",
        "lateness_seconds=0.0",
    ),
    "shutdown-expired": (
        "worker_capture.py",
        "deadline_exhausted=self.shutdown_finished >= end",
        "deadline_exhausted=False",
    ),
    "early-kill": (
        "worker_capture.py",
        "term_at + (self.bootstrap.term_grace if failed_bootstrap else 2)",
        "term_at + (self.bootstrap.term_grace - 0.005 if failed_bootstrap else 2)",
    ),
    "late-kill": (
        "worker_capture.py",
        "term_at + (self.bootstrap.term_grace if failed_bootstrap else 2)",
        "term_at + (self.bootstrap.term_grace + 0.04 if failed_bootstrap else 2)",
    ),
    "kill-reap-small": (
        "worker_capture.py",
        "(self.bootstrap.term_grace + self.bootstrap.kill_reap if failed_bootstrap else 4)",
        "(self.bootstrap.term_grace + self.bootstrap.kill_reap + 0.02 if failed_bootstrap else 4)",
    ),
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--mutant", choices=MUTATIONS)
    args = parser.parse_args()
    source, destination = args.source.resolve(), args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    paths = [*sorted(source.glob("*.py")), source / "pyproject.toml", source / "uv.lock"]
    for path in paths:
        shutil.copy2(path, destination / path.name)
    taskfile = source / "Taskfile.yml"
    if not taskfile.exists():
        taskfile = source.parents[1] / "Taskfile.yml"
    shutil.copy2(taskfile, destination / "Taskfile.yml")
    if args.mutant:
        filename, before, after = MUTATIONS[args.mutant]
        path = destination / filename
        content = path.read_text()
        if content.count(before) != 1:
            raise ValueError(f"mutation {args.mutant}: expected exactly one source site")
        path.write_text(content.replace(before, after))
    (destination / "preparation.json").write_text(
        json.dumps({"source": str(source), "mutation": args.mutant}, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
