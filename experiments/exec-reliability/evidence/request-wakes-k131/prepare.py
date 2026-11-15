"""Freeze diagnostic inputs, optionally with one named authority regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "idle-spin": (
        "worker_capture.py",
        "time.monotonic() + (0 if ready else 0.005)",
        "time.monotonic()",
    ),
    "ready-delay": (
        "worker_capture.py",
        "time.monotonic() + (0 if ready else 0.005)",
        "time.monotonic() + 0.005",
    ),
    "stale-kill": (
        "worker_capture.py",
        'kill_at if not owner.reaped and owner.reap_errno is None else float("inf")',
        "kill_at",
    ),
    "slow-retirement": (
        "worker_capture.py",
        "return min(deadline, next_kill, time.monotonic() + 0.001)",
        "return min(deadline, next_kill, time.monotonic() + 0.005)",
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
        start = 0
        end = len(content)
        scope = content[start:end]
        if scope.count(before) != 1:
            raise ValueError(f"mutation {args.mutant}: expected exactly one source site")
        content = content[:start] + scope.replace(before, after) + content[end:]
        path.write_text(content)
    (destination / "preparation.json").write_text(
        json.dumps({"source": str(source), "mutation": args.mutant}, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
