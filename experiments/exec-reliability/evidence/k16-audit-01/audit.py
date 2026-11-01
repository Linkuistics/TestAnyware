"""Audit only retained local fake peers; never signal a process or touch a VM."""

import hashlib
import json
import subprocess
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVIDENCE = ROOT.parent
NAMES = (
    "self-check",
    "self-check-final",
    "k16-red-01",
    "k16-red-02",
    "k16-red-03",
    "k16-green-01",
    "k16-green-02",
)
subjects = [
    Path(__file__),
    *(path for name in NAMES for path in (EVIDENCE / name).rglob("*") if path.is_file()),
]


def hashes():
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in subjects}


before = hashes()
(ROOT / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
resources = []
pids = set()
uncertain = []
for path in subjects:
    if path.name == "fork.stdout" and path.read_bytes().strip().isdigit():
        pids.add(int(path.read_bytes().strip()))
    if path.suffix != ".json":
        continue
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        continue
    if path.name == "result.json":
        uncertain.extend(
            {"receipt": str(path), **item}
            for item in value.get("resources", [])
            if item["state"] == "uncertain"
        )
    for receipt in [value, *value.get("calls", [])]:
        if isinstance(receipt, dict) and isinstance(receipt.get("pid"), int):
            pids.add(receipt["pid"])
    if path.name != "plan.json" or "resources" not in value:
        continue
    for resource in value["resources"]:
        argv = resource["acquire"]["argv"]
        identity = resource["identity"]
        assert value["run_id"] in identity
        assert Path(argv[1]).name == "peer.py" and argv[2:4] == ["acquire", identity]
        marker = Path(argv[4]) / identity
        assert marker.is_relative_to(Path(tempfile.gettempdir()).resolve())
        assert "pytest-of-antony" in marker.parts
        assert not marker.is_symlink()
        present = marker.exists()
        if present:
            assert marker.read_bytes() == b"owned"
            marker.unlink()
        resources.append(
            {
                "plan": str(path),
                "identity": identity,
                "marker": str(marker),
                "present_before": present,
                "absent_after": not marker.exists(),
            }
        )
argv = ["/bin/ps", "-p", ",".join(map(str, sorted(pids))), "-o", "pid=,ppid=,pgid=,command="]
inspection = subprocess.run(argv, capture_output=True, timeout=5, check=False)
(ROOT / "processes.stdout").write_bytes(inspection.stdout)
(ROOT / "processes.stderr").write_bytes(inspection.stderr)
processes_absent = (
    inspection.returncode == 1 and inspection.stdout == b"" and inspection.stderr == b""
)
after = hashes()
result = {
    "subjects_before": before,
    "subjects_after": after,
    "resources": resources,
    "uncertain_entries": uncertain,
    "process_argv": argv,
    "process_exit": inspection.returncode,
    "recorded_pids": sorted(pids),
    "processes_absent": processes_absent,
    "ok": before == after and processes_absent and all(item["absent_after"] for item in resources),
}
(ROOT / "result.json").write_text(json.dumps(result, indent=2) + "\n")
print(
    json.dumps(
        {
            "ok": result["ok"],
            "plans_with_resources": len(resources),
            "uncertain_entries": len(uncertain),
            "pids": len(pids),
            "removed_markers": [item["marker"] for item in resources if item["present_before"]],
        }
    )
)
