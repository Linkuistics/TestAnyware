"""Freeze diagnostic inputs before a declared worker/teardown comparison."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "synchronous-retirement": (
        "defer_retirement=self.retirements.append,",
        "defer_retirement=None,",
    ),
    "skip-companion": ("self.worker.advance_close()", "None"),
    "skip-failure-retirement": (
        'self.worker.failure = self.worker.failure or f"local inspection failed after {call}"',
        "pass",
    ),
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--mutant", choices=MUTATIONS)
    parser.add_argument("--baseline", type=Path)
    args = parser.parse_args()
    source, destination = args.source.resolve(), args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    for path in [*sorted(source.glob("*.py")), source / "pyproject.toml", source / "uv.lock"]:
        shutil.copy2(path, destination / path.name)
    taskfile = source / "Taskfile.yml"
    if not taskfile.is_file():
        taskfile = source.parents[1] / "Taskfile.yml"
    shutil.copy2(taskfile, destination / "Taskfile.yml")
    if args.baseline:
        for name in ("controller.py", "worker_capture.py"):
            shutil.copy2(args.baseline / name, destination / name)
    if args.mutant:
        path = destination / "controller.py"
        text = path.read_text()
        before, after = MUTATIONS[args.mutant]
        if text.count(before) != 1:
            raise ValueError(f"mutation {args.mutant}: expected exactly one source site")
        path.write_text(text.replace(before, after))
    (destination / "preparation.json").write_text(
        json.dumps(
            {"source": str(source), "mutation": args.mutant, "baseline": str(args.baseline)},
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
