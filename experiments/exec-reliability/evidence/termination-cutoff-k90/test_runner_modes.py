"""Unknown, missing or extra runner modes must fail before checker dispatch."""

import os
import subprocess
import sys
from pathlib import Path

import pytest


@pytest.mark.parametrize("modes", [[], ["ful"], ["full", "extra"]])
def test_runner_rejects_invalid_mode(tmp_path, modes):
    root = Path(os.environ.get("DIAGNOSTIC_EVIDENCE", tmp_path))
    output = root / ("invalid-" + "-".join(modes or ["missing"]))
    result = subprocess.run(
        [
            sys.executable,
            str(Path(__file__).with_name("run_condition.py")),
            str(Path(__file__).parents[1] / "termination-cutoff-k85/green-v3-inputs"),
            str(output),
            *modes,
        ],
        capture_output=True,
        timeout=20,
    )
    root.mkdir(parents=True, exist_ok=True)
    (root / (output.name + ".stdout")).write_bytes(result.stdout)
    (root / (output.name + ".stderr")).write_bytes(result.stderr)
    assert result.returncode == 2
    assert not output.exists()
