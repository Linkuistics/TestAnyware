"""Run one frozen k77 mutation/control condition under a fixed checker budget."""

import hashlib
import json
import os
import signal
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CONDITION = os.environ["K77_CONDITION"]
SOURCE = ROOT / CONDITION / "experiments/exec-reliability"
OUTPUT = ROOT.parent / CONDITION
SELECTIONS = {
    "k77-red-02": "late_created_identity_is_cleanup_only and overflow and not malformed",
    "k77-invalid-mutant-02": "late_created_identity_rejects_invalid_fields",
    "k77-scope-mutant-02": "late_created_identity_is_cleanup_only and select",
}


def hashes(paths):
    return {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in paths}


def main():
    OUTPUT.mkdir(exist_ok=False)
    subjects = sorted(p for p in SOURCE.rglob("*") if p.is_file())
    subjects += [Path(__file__), Path(sys.executable), Path("/bin/ps"), Path("/usr/bin/true")]
    before = hashes(subjects)
    argv = [
        sys.executable,
        "-m",
        "pytest",
        "-vv",
        "test_worker_capture.py",
        "-k",
        SELECTIONS[CONDITION],
        f"--junitxml={OUTPUT / 'pytest.xml'}",
    ]
    (OUTPUT / "declaration.json").write_text(
        json.dumps(
            {"subjects_before": before, "argv": argv, "budget": 60, "term": 2, "kill": 2},
            indent=2,
        )
        + "\n"
    )
    environment = {**os.environ, "DIAGNOSTIC_EVIDENCE": str(OUTPUT / "runs")}
    start = time.monotonic()
    timed_out = False
    with (OUTPUT / "stdout").open("xb") as stdout, (OUTPUT / "stderr").open("xb") as stderr:
        child = subprocess.Popen(
            argv, cwd=SOURCE, env=environment, stdout=stdout, stderr=stderr, start_new_session=True
        )
        try:
            code = child.wait(timeout=60)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(child.pid, signal.SIGTERM)
            try:
                code = child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                code = child.wait(timeout=2)
    after = hashes(subjects)
    verdict = {
        "subjects_before": before,
        "subjects_after": after,
        "unchanged": before == after,
        "pid": child.pid,
        "returncode": code,
        "reaped": True,
        "timed_out": timed_out,
        "elapsed": time.monotonic() - start,
        "artifacts": hashes(sorted(p for p in OUTPUT.rglob("*") if p.is_file())),
    }
    (OUTPUT / "verdict.json").write_text(json.dumps(verdict, indent=2) + "\n")
    print(json.dumps({k: verdict[k] for k in ("returncode", "unchanged", "timed_out", "elapsed")}))
    assert before == after and not timed_out and code == 1


if __name__ == "__main__":
    main()
