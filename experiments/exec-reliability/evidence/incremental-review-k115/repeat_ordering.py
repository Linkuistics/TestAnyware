"""Retain ten isolated ordering/ignore-term samples at each declared revision."""

import hashlib
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
DIAGNOSTIC = HERE.parents[1]
REVISIONS = {"parent": "1e8cbf4d", "k110": "cbd2aace"}


def main() -> None:
    paths = [*sorted(DIAGNOSTIC.glob("*.py")), DIAGNOSTIC / "pyproject.toml"]
    paths.extend((DIAGNOSTIC / "uv.lock", REPO / "Taskfile.yml"))
    inputs = {}
    for label, revision in REVISIONS.items():
        destination = HERE / f"ordering-{label}-inputs"
        destination.mkdir(exist_ok=False)
        hashes = {}
        for path in paths:
            data = subprocess.run(
                ["jj", "file", "show", "-r", revision, str(path.relative_to(REPO))],
                cwd=REPO,
                capture_output=True,
                check=True,
            ).stdout
            (destination / path.name).write_bytes(data)
            hashes[path.name] = hashlib.sha256(data).hexdigest()
        inputs[label] = {"revision": revision, "sha256": hashes}
    # Declare the fixed count and both byte sets before any sample runs.
    with (HERE / "ordering-declaration.json").open("x") as stream:
        json.dump({"samples_per_revision": 10, "inputs": inputs}, stream, indent=2)
        stream.write("\n")
    outcomes: dict[str, list[int]] = {}
    for label in REVISIONS:
        codes: list[int] = []
        for sample in range(1, 11):
            output = HERE / f"ordering-{label}-{sample:02}"
            result = subprocess.run(
                [
                    sys.executable,
                    str(HERE.parent / "worker-reap-scope-k112/run_condition.py"),
                    str(HERE / f"ordering-{label}-inputs"),
                    str(output),
                    "controls",
                    "--select",
                    "test_review_group_kill_precedes_inspection and ignore-term",
                ],
                check=False,
            )
            codes.append(result.returncode)
        outcomes[label] = codes
    with (HERE / "ordering-outcomes.json").open("x") as stream:
        json.dump(outcomes, stream, indent=2)
        stream.write("\n")


if __name__ == "__main__":
    main()
