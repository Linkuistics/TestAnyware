"""Freeze diagnostic inputs, optionally with one named authority regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "no-fallback": [
        "turns.py",
        "except (OSError, ValueError):\n                    time.sleep(delay)",
        "except (OSError, ValueError):\n                    raise",
    ],
    "stream-eof": [
        "worker_capture.py",
        "del streams[fd]",
        "pass  # Mutant retains a closed stream interest.",
    ],
    "command-empty": [
        "worker_capture.py",
        "def query_readiness() -> tuple[tuple[int, ...], tuple[int, ...]]:\n"
        "            if command:",
        "def query_readiness() -> tuple[tuple[int, ...], tuple[int, ...]]:\n"
        "            if worker is not None:",
    ],
    "cleanup-eof": [
        "worker_capture.py",
        "(control_read,) if cleanup_open and control_read is not None else ()",
        "(control_read,) if control_read is not None else ()",
    ],
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--mutant", choices=MUTATIONS)
    parser.add_argument("--worker-source", type=Path)
    parser.add_argument("--turns-source", type=Path)
    parser.add_argument("--tests-source", type=Path)
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
    if args.worker_source:
        shutil.copy2(args.worker_source, destination / "worker_capture.py")
    if args.turns_source:
        shutil.copy2(args.turns_source, destination / "turns.py")
    if args.tests_source:
        shutil.copy2(args.tests_source, destination / "test_turns.py")
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
        json.dumps(
            {
                "source": str(source),
                "mutation": args.mutant,
                "worker_source": str(args.worker_source) if args.worker_source else None,
                "turns_source": str(args.turns_source) if args.turns_source else None,
                "tests_source": str(args.tests_source) if args.tests_source else None,
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
