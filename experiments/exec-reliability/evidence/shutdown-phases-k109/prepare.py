"""Freeze diagnostic inputs, optionally with one named shutdown regression."""

import argparse
import json
import shutil
from pathlib import Path

MUTATIONS = {
    "drop-reserve": (
        "worker_capture.py",
        "old_end, phase_start + grace, max(self.shutdown_started, old_end - reap_budget)",
        "old_end, phase_start + grace",
    ),
    "ignore-prior-term": (
        "worker_capture.py",
        'phase_start = min(phase_start, cast(float, attempt["monotonic"]))',
        "phase_start = phase_start",
    ),
    "fresh-grace": (
        "worker_capture.py",
        "old_end, phase_start + grace, max(self.shutdown_started, old_end - reap_budget)",
        "old_end, self.shutdown_started + grace, max(self.shutdown_started, old_end - reap_budget)",
    ),
    "grace-offset": (
        "worker_capture.py",
        "old_end, phase_start + grace, max(self.shutdown_started, old_end - reap_budget)",
        "old_end, phase_start + grace + 0.003, max(self.shutdown_started, old_end - reap_budget)",
    ),
    "kill-offset": (
        "worker_capture.py",
        "if not kill_sent and kill_at <= now < end:\n"
        "                    attempts = len(owner.signals)",
        "if not kill_sent and kill_at <= now < end:\n"
        "                    time.sleep(0.003)\n"
        "                    attempts = len(owner.signals)",
    ),
    "hide-grace-failure": (
        "worker_capture.py",
        "if grace_finished > kill_at and prior_kill is None:",
        "if False:",
    ),
    "hide-grace-late": (
        "worker_capture.py",
        '"late" if finish > cutoff else "at_deadline" if finish == cutoff else "on_time"',
        '"on_time"',
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
