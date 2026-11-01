"""Audit this producer's finite local conditions; never signal a process."""

import hashlib
import json
import os
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVIDENCE = ROOT.parent
CONDITIONS = (
    "k20-red-01",
    "k20-green-01",
    "k20-green-02",
    "k20-green-03",
    "k20-final-01",
    "k20-final-02",
)
SUBJECTS = [
    Path(__file__),
    Path("/bin/ps"),
    *sorted(path for name in CONDITIONS for path in (EVIDENCE / name).rglob("*") if path.is_file()),
]


def hashes():
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in SUBJECTS}


before = hashes()
(ROOT / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
pids = set()
scratch = set()
resources = []
known_children = []


def collect(value):
    if isinstance(value, dict):
        for key, item in value.items():
            if key in {"pid", "runner_pid"} and isinstance(item, int):
                pids.add(item)
            collect(item)
    elif isinstance(value, list):
        for item in value:
            collect(item)
    elif isinstance(value, str):
        match = re.search(r"(/[^\n]*?/pytest-of-antony/pytest-\d+/test_[^/\n]+)", value)
        if match:
            scratch.add(Path(match[1]))


for path in SUBJECTS:
    if path.name in {"fork.stdout", "scope.stdout", "stubborn.stdout"}:
        child = int(path.read_bytes().strip())
        pids.add(child)
        known_children.append({"receipt": str(path), "pid": child})
    if path.suffix != ".json":
        continue
    value = json.loads(path.read_text())
    collect(value)
    if not isinstance(value, dict) or path.name != "plan.json":
        continue
    for resource in value.get("resources", []):
        argv = resource["acquire"]["argv"]
        identity = resource["identity"]
        assert value["run_id"] in identity
        assert Path(argv[1]).name == "peer.py" and argv[2:4] == ["acquire", identity]
        marker = Path(argv[4]) / identity
        assert "pytest-of-antony" in marker.parts and not marker.is_symlink()
        present = marker.exists()
        if present:
            assert marker.read_bytes() == b"owned"
            marker.unlink()
        resources.append(
            {
                "plan": str(path),
                "identity": identity,
                "path": str(marker),
                "present_before": present,
                "absent_after": not marker.exists(),
            }
        )

argv = [
    "/bin/ps",
    "-p",
    ",".join(map(str, sorted(pids))),
    "-o",
    "pid=,ppid=,pgid=,stat=,lstart=,command=",
]
inspection = subprocess.run(argv, capture_output=True, timeout=5)
(ROOT / "processes.stdout").write_bytes(inspection.stdout)
(ROOT / "processes.stderr").write_bytes(inspection.stderr)
absent = inspection.returncode == 1 and inspection.stdout == b"" and inspection.stderr == b""
# The same oracle must reject a genuine live process and a ps usage failure.
live = subprocess.run(
    ["/bin/ps", "-p", str(os.getpid()), "-o", "pid="], capture_output=True, timeout=1
)
usage = subprocess.run(["/bin/ps", "--not-a-valid-option"], capture_output=True, timeout=1)
controls = {
    "live": {
        "returncode": live.returncode,
        "stdout": live.stdout.decode(),
        "stderr": live.stderr.decode(),
    },
    "usage": {
        "returncode": usage.returncode,
        "stdout": usage.stdout.decode(),
        "stderr": usage.stderr.decode(),
    },
}
assert live.returncode == 0 and live.stdout.strip().isdigit() and not live.stderr
assert usage.returncode != 0 and usage.stderr
scratch_results = []
for directory in sorted(scratch):
    assert not directory.is_symlink()
    files = []
    if directory.exists():
        # Remove only the finite known fixture outputs, never recurse through
        # an unknown file or a symlink. Unexpected contents leave a residual.
        for path in sorted(directory.iterdir()):
            if (
                path.is_file()
                and not path.is_symlink()
                and (
                    path.name
                    in {"unrelated", "input", "not-executable", "release", "ready", "nested.py"}
                    or re.fullmatch(r"diag-[0-9a-f]{32}-(vm|second)", path.name)
                )
            ):
                files.append(
                    {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                )
                path.unlink()
        if not any(directory.iterdir()):
            directory.rmdir()
    scratch_results.append(
        {"path": str(directory), "removed_files": files, "absent_after": not directory.exists()}
    )
after = hashes()
result = {
    "conditions": CONDITIONS,
    "subjects_before": before,
    "subjects_after": after,
    "resources": resources,
    "known_children": known_children,
    "recorded_pids": sorted(pids),
    "process_argv": argv,
    "process_exit": inspection.returncode,
    "processes_absent": absent,
    "absence_oracle_controls": controls,
    "scratch_directories": scratch_results,
    "ok": before == after
    and absent
    and all(item["absent_after"] for item in resources)
    and all(item["absent_after"] for item in scratch_results),
}
(ROOT / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(
    json.dumps(
        {
            "ok": result["ok"],
            "pids": len(pids),
            "resources": len(resources),
            "known_children": len(known_children),
            "scratch_directories": len(scratch),
        }
    )
)
