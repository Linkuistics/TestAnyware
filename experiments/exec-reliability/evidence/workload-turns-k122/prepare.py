"""Freeze diagnostic inputs, optionally with one named authority regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "late-entry": ("controller.py", "if now >= self.deadline:", "if False:"),
    "late-result": ("controller.py", "if observed >= self.deadline:", "if False:"),
    "sleep-in-turn": (
        "controller.py",
        "return min(self.deadline, observed + 0.005)",
        "time.sleep(0.001)\n        return min(self.deadline, observed + 0.005)",
    ),
    "repoll-in-turn": (
        "controller.py",
        "exited = _exited(self.pid)",
        "exited = _exited(self.pid)\n            exited = _exited(self.pid)",
    ),
    "retry-terminal": (
        "controller.py",
        "if self.finished:\n            return None\n        now = time.monotonic()",
        "if False:\n            return None\n        now = time.monotonic()",
    ),
    "extend-wake": (
        "controller.py",
        "return min(self.deadline, observed + 0.005)",
        "return observed + 0.005",
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
        start = content.index("class _WorkloadWait:")
        end = content.index("def _wait(", start)
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
