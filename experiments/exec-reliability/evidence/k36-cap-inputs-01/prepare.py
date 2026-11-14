"""Freeze identical review-control trees except for the PID-only deadline cap."""

import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parents[3]
SOURCE = PROJECT / "experiments/exec-reliability"
OLD = "min(time.monotonic(), term_deadline) + plan.kill_wait"
NEW = "time.monotonic() + plan.kill_wait"


def main() -> None:
    inputs = [PROJECT / "Taskfile.yml", *sorted(SOURCE.glob("*.py"))]
    inputs += [SOURCE / "pyproject.toml", SOURCE / "uv.lock", Path(__file__)]
    before = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
    snapshots = {}
    for condition in ("normal", "removed-cap"):
        destination = ROOT / condition
        destination.mkdir()
        entries = {}
        for source in inputs[:-1]:
            relative = source.relative_to(PROJECT)
            target = destination / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            raw = source.read_bytes()
            if condition == "removed-cap" and source.name == "controller.py":
                assert raw.count(OLD.encode()) == 1
                raw = raw.replace(OLD.encode(), NEW.encode())
            target.write_bytes(raw)
            entries[str(relative)] = hashlib.sha256(raw).hexdigest()
        # Reuse the already selected, locked environment; do not install a new one.
        (destination / "experiments/exec-reliability/.venv").symlink_to(SOURCE / ".venv")
        snapshots[condition] = entries
    differing = [
        key
        for key in snapshots["normal"]
        if snapshots["normal"][key] != snapshots["removed-cap"][key]
    ]
    assert differing == ["experiments/exec-reliability/controller.py"]
    after = {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in inputs}
    assert before == after
    (ROOT / "manifest.json").write_text(
        json.dumps(
            {
                "subjects_before": before,
                "subjects_after": after,
                "snapshots": snapshots,
                "mutation": {"before": OLD, "after": NEW, "differing_paths": differing},
            },
            indent=2,
        )
        + "\n"
    )


if __name__ == "__main__":
    main()
