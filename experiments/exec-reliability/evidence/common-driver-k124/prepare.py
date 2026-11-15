"""Freeze diagnostic inputs, optionally with one named authority regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "no-safety": ("turns.py", "not item[2].safety, item[0], item[1]", "False, item[0], item[1]"),
    "latest-wake": (
        "turns.py",
        "not item[2].safety, item[0], item[1]",
        "not item[2].safety, -item[0], item[1]",
    ),
    "no-rotation": (
        "turns.py",
        "pending.append((wake, sequence, item[2]))",
        "pending.append((wake, item[1], item[2]))",
    ),
    "uncapped-sleep": (
        "turns.py",
        "min(0.005, wake - time.monotonic())",
        "wake - time.monotonic()",
    ),
    "suppress-error": (
        "turns.py",
        "wake = item[2].advance()",
        (
            "try:\n            wake = item[2].advance()\n"
            "        except OSError:\n            wake = None"
        ),
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
