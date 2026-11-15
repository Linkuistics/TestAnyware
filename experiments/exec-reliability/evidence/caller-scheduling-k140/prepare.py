"""Freeze all diagnostic sources and one optional named ownership regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "query-bypass": (
        "schedule(Turn(advance_query, readiness=query_readiness))",
        "run_turns(Turn(advance_query, readiness=query_readiness))",
    ),
    "retirement-bypass": (
        "schedule(Turn(advance_retirement, safety=True, readiness=retirement_readiness))",
        "run_turns(Turn(advance_retirement, safety=True, readiness=retirement_readiness))",
    ),
    "close-bypass": (
        "schedule(Turn(self.advance_close, safety=True))",
        "run_turns(Turn(self.advance_close, safety=True))",
    ),
    "capture-close-bypass": ("worker.close(drive=drive)", "worker.close()"),
}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("destination", type=Path)
    parser.add_argument("--mutant", choices=MUTATIONS)
    args = parser.parse_args()
    source, destination = args.source.resolve(), args.destination.resolve()
    destination.mkdir(parents=True, exist_ok=False)
    for path in [*sorted(source.glob("*.py")), source / "pyproject.toml", source / "uv.lock"]:
        shutil.copy2(path, destination / path.name)
    taskfile = source / "Taskfile.yml"
    if not taskfile.exists():
        taskfile = source.parents[1] / "Taskfile.yml"
    shutil.copy2(taskfile, destination / "Taskfile.yml")
    if "test_caller_driver_owns_close" in (destination / "test_turns.py").read_text():
        (destination / "caller-controls.marker").touch()
    if args.mutant:
        path = destination / "worker_capture.py"
        text = path.read_text()
        before, after = MUTATIONS[args.mutant]
        if text.count(before) != 1:
            raise ValueError(f"mutation {args.mutant}: expected exactly one source site")
        path.write_text(text.replace(before, after))
    (destination / "preparation.json").write_text(
        json.dumps({"source": str(source), "mutation": args.mutant}, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
