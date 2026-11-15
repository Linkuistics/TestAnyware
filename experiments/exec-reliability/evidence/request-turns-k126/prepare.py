"""Freeze diagnostic inputs, optionally with one named authority regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "query-safety": (
        "worker_capture.py",
        "run_turns(Turn(advance_query))",
        "run_turns(Turn(advance_query, safety=True))",
    ),
    "retirement-ordinary": (
        "worker_capture.py",
        "run_turns(Turn(advance_retirement, safety=True))",
        "run_turns(Turn(advance_retirement))",
    ),
    "blocking-query": (
        "worker_capture.py",
        "select.select(list(streams), [], [], 0)",
        "select.select(list(streams), [], [], 0.005)",
    ),
    "uncapped-result": (
        "worker_capture.py",
        "min(TURN_CAP // 2, HEADER.size + result_size - len(buffers[1]) + 1)",
        "min(FRAME_CAP, HEADER.size + result_size - len(buffers[1]) + 1)",
    ),
    "terminal-read": (
        "worker_capture.py",
        "if complete or time.monotonic() >= query_deadline:",
        "if time.monotonic() >= query_deadline:",
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
