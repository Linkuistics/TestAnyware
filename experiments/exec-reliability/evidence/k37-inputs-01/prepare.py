"""Freeze the previous controller with the current table controls for a RED replay."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
PROJECT = HERE.parents[3]
SOURCES = [
    PROJECT / "Taskfile.yml",
    *sorted((PROJECT / "experiments/exec-reliability").glob("*.py")),
    PROJECT / "experiments/exec-reliability/pyproject.toml",
    PROJECT / "experiments/exec-reliability/uv.lock",
]
SUBJECTS = [Path(__file__), Path(sys.executable), *SOURCES]


def hashes(paths):
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in paths}


def main():
    before = hashes(SUBJECTS)
    baseline = subprocess.check_output(
        ["jj", "--no-pager", "log", "-r", "@-", "--no-graph", "-T", "commit_id"],
        cwd=PROJECT,
        timeout=5,
        text=True,
    ).strip()
    old = subprocess.check_output(
        [
            "jj",
            "--no-pager",
            "file",
            "show",
            "-r",
            baseline,
            "experiments/exec-reliability/controller.py",
        ],
        cwd=PROJECT,
        timeout=5,
    )
    snapshot = HERE / "previous"
    snapshot.mkdir()
    for source in SOURCES:
        relative = source.relative_to(PROJECT)
        target = snapshot / relative
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(
            old if relative.as_posix().endswith("/controller.py") else source.read_bytes()
        )
    after = hashes(SUBJECTS)
    assert before == after
    manifest = {
        "baseline_revision": baseline,
        "subjects_before": before,
        "subjects_after": after,
        "snapshot_sha256": hashes(sorted(path for path in snapshot.rglob("*") if path.is_file())),
        "change": "Previous controller only; all tests and other inputs match the current condition.",
    }
    (HERE / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")


if __name__ == "__main__":
    main()
