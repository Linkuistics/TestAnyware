"""Freeze all diagnostic sources and one optional named ownership regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "no-receipt-residual": (
        "if worker.owner is not None and not worker.owner.reaped:\n"
        "                receipt.residual_remedy",
        "if False:\n                receipt.residual_remedy",
    ),
    "false-receipt-residual": (
        "if worker.owner is not None and not worker.owner.reaped:\n"
        "                receipt.residual_remedy",
        "if worker.owner is not None:\n                receipt.residual_remedy",
    ),
    "no-ownership-lost": ("owner.ownership_lost = True", "owner.ownership_lost = False"),
    "false-echild-reaped": (
        "owner.ownership_lost = True",
        "owner.ownership_lost = True; owner.reaped = True",
    ),
    "retain-lost-owner": (
        "owner.ownership_lost = True\n            _owners.pop(owner.pid, None)",
        "owner.ownership_lost = True",
    ),
    "no-first-reap": (
        "if now >= owner.shutdown_deadline_monotonic and owner.reap_attempts:",
        "if now >= owner.shutdown_deadline_monotonic:",
    ),
    "repeat-reap": (
        "if now >= owner.shutdown_deadline_monotonic and owner.reap_attempts:",
        "if False:",
    ),
    "no-residual": ("and not self.owner.reaped\n", "and False\n"),
    "false-residual": ("and not self.owner.reaped\n", "and True\n"),
    "expired-signal": (
        "if now >= owner.shutdown_deadline_monotonic:\n        return\n    event:",
        "if False:\n        return\n    event:",
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
    for path in [*sorted(source.glob("*.py")), source / "pyproject.toml", source / "uv.lock"]:
        shutil.copy2(path, destination / path.name)
    taskfile = source / "Taskfile.yml"
    if not taskfile.exists():
        taskfile = source.parents[1] / "Taskfile.yml"
    shutil.copy2(taskfile, destination / "Taskfile.yml")
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
