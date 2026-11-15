"""Freeze diagnostic inputs, optionally with one named authority regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "result-eof-only": (
        "worker_capture.py",
        "closed_channels == {0, 1}",
        "1 in closed_channels",
    ),
    "eagain-is-eof": (
        "worker_capture.py",
        "                            except BlockingIOError:\n"
        "                                chunk = None\n",
        "                            except BlockingIOError:\n"
        "                                closed_channels.add(0)\n"
        "                                chunk = None\n",
    ),
    "omit-eager-conflict": (
        "worker_capture.py",
        "                                if chunk and fork_failure is not None:\n"
        "                                    creation_conflict = True\n",
        "",
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
        content = content.replace(before, after)
        path.write_text(content)
    (destination / "preparation.json").write_text(
        json.dumps({"source": str(source), "mutation": args.mutant}, indent=2) + "\n"
    )


if __name__ == "__main__":
    main()
