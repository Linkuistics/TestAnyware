"""Freeze diagnostic inputs, optionally with one named bootstrap regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "plan-validation": ("controller.py", "    plan.bootstrap.validate()", "    pass"),
    "plan-wiring": (
        "controller.py",
        "Worker(run_id=plan.run_id, bootstrap=plan.bootstrap)",
        "Worker(run_id=plan.run_id)",
    ),
    "readiness": (
        "worker_capture.py",
        "deadline = self.startup_started + self.bootstrap.readiness",
        "deadline = self.startup_started + 1.0",
    ),
    "term": (
        "worker_capture.py",
        "term_at + (self.bootstrap.term_grace if failed_bootstrap else 2)",
        "term_at + 2",
    ),
    "kill-reap": (
        "worker_capture.py",
        "(self.bootstrap.term_grace + self.bootstrap.kill_reap if failed_bootstrap else 4)",
        "4",
    ),
    "overrun": (
        "worker_capture.py",
        "overrun = self.shutdown_finished > end",
        "overrun = False",
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
