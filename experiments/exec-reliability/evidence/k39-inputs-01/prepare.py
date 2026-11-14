"""Freeze k39 inputs and replay the three reap controls on the previous controller."""

import hashlib
import json
import os
import subprocess
import sys
import xml.etree.ElementTree as ET
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
    for condition in ("previous", "current"):
        snapshot = HERE / condition
        snapshot.mkdir()
        for source in SOURCES:
            relative = source.relative_to(PROJECT)
            target = snapshot / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(
                old
                if condition == "previous" and relative.as_posix().endswith("/controller.py")
                else source.read_bytes()
            )
    snapshot_hashes = hashes(
        sorted(
            path
            for condition in ("previous", "current")
            for path in (HERE / condition).rglob("*")
            if path.is_file()
        )
    )
    output = HERE.parent / "k39-previous-01"
    output.mkdir()
    argv = (
        "mise",
        "exec",
        "--",
        "task",
        "diagnostic:test",
        "--",
        "-vv",
        "-k",
        "nonblocking_reap",
        f"--junitxml={output / 'pytest.xml'}",
    )
    with (
        (output / "pytest.stdout").open("xb") as stdout,
        (output / "pytest.stderr").open("xb") as stderr,
    ):
        check = subprocess.run(
            argv,
            cwd=HERE / "previous",
            env={**os.environ, "DIAGNOSTIC_EVIDENCE": str(output / "runs")},
            stdout=stdout,
            stderr=stderr,
            timeout=30,
        )
    cases = ET.parse(output / "pytest.xml").findall(".//testcase")
    assert check.returncode != 0 and len(cases) == 3
    assert all(case.find("failure") is not None for case in cases)
    after = hashes(SUBJECTS)
    assert before == after
    manifest = {
        "baseline_revision": baseline,
        "subjects_before": before,
        "subjects_after": after,
        "snapshot_sha256": snapshot_hashes,
        "change": "Previous controller only; tests and supporting inputs match current.",
    }
    (HERE / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    verdict = {
        "argv": argv,
        "returncode": check.returncode,
        "expected_failures": [case.attrib["name"] for case in cases],
        "artifact_sha256": hashes(sorted(path for path in output.rglob("*") if path.is_file())),
        "ok": False,
        "control_replay_ok": True,
    }
    (output / "verdict.json").write_text(json.dumps(verdict, indent=2) + "\n")
    print("Previous controller: all three new controls fail as expected.")


if __name__ == "__main__":
    main()
