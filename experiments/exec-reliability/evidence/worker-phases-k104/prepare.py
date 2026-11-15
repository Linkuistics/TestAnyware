"""Freeze diagnostic inputs, optionally with one named shutdown regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "renew-cutoff": (
        "worker_capture.py",
        "min(old_end, kill_at + reap_budget)",
        "kill_at + reap_budget",
    ),
    "healthy-term": ("worker_capture.py", 'term_sent = mode == "orderly"', "term_sent = False"),
    "no-overrun": (
        "worker_capture.py",
        "overrun = self.shutdown_finished > end",
        "overrun = False",
    ),
    "failure-hardcode": (
        "worker_capture.py",
        "grace = self.failure_bounds.term_grace",
        "grace = 2",
    ),
    "duplicate-term": (
        "worker_capture.py",
        "term_sent = True\n                    phase_start",
        "term_sent = False\n                    phase_start",
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
