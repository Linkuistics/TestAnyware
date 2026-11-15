"""Compare retained items and supplement incomplete absence audits without signals."""

import hashlib
import json
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent
SAMPLES = (
    "red",
    "red2",
    "red3",
    "candidate-controls",
    "candidate-controls-py312",
    "parent-full",
    "candidate-full",
    "parent-final",
    "candidate-final",
    "no-first-reap",
    "repeat-reap",
    "no-residual",
    "false-residual",
    "expired-signal",
)


def items(path: Path) -> dict[str, str]:
    result: dict[str, str] = {}
    for case in ET.parse(path).iter("testcase"):
        key = case.attrib["classname"] + "::" + case.attrib["name"]
        if key in result:
            raise ValueError(f"duplicate item: {key}")
        result[key] = (
            "failed"
            if case.find("failure") is not None or case.find("error") is not None
            else "skipped"
            if case.find("skipped") is not None
            else "passed"
        )
    return result


def main() -> None:
    subjects = [Path(__file__)]
    for name in SAMPLES:
        directory = ROOT / name
        subjects.extend(path for path in directory.iterdir() if path.is_file())
        subjects.extend((directory / "runs").glob("owned-termination-*.json"))
        subjects.extend((directory / "runs").glob("persistent-expired-independent-*.json"))
    before = {
        str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in subjects
    }
    samples: dict[str, object] = {}
    pending: set[int] = set()
    for name in SAMPLES:
        directory = ROOT / name
        tests = items(directory / "pytest.xml")
        verdict = directory / "verdict.json"
        value = json.loads(verdict.read_text()) if verdict.exists() else None
        samples[name] = {
            "items": tests,
            "counts": {
                status: list(tests.values()).count(status)
                for status in ("passed", "failed", "skipped")
            },
            "stable": value["subjects_before"] == value["subjects_after"] if value else None,
            "verdict": value["ok"] if value else None,
        }
        if value:
            for check in value["checks"]:
                for fixture in check["fixtures"]:
                    if not fixture["absent"]:
                        pending.add(fixture["pid"])
        else:
            for journal in (directory / "runs").glob("owned-termination-*.json"):
                pending.add(json.loads(journal.read_text())["identity"]["pid"])
            for journal in (directory / "runs").glob("persistent-expired-independent-*.json"):
                pending.add(json.loads(journal.read_text())["owner"]["pid"])

    audits: list[dict[str, object]] = []
    for pid in sorted(pending):
        try:
            current = subprocess.run(
                ["/bin/ps", "-p", str(pid), "-o", "pid=,pgid=,stat=,lstart=,command="],
                capture_output=True,
                text=True,
                timeout=1,
            )
            audits.append(
                {
                    "pid": pid,
                    "returncode": current.returncode,
                    "stdout": current.stdout,
                    "stderr": current.stderr,
                    "absent": current.returncode == 1 and not current.stdout and not current.stderr,
                    "signals": [],
                }
            )
        except subprocess.TimeoutExpired:
            audits.append({"pid": pid, "absent": False, "timeout": True, "signals": []})
    parent, candidate = (
        items(ROOT / "parent-final/pytest.xml"),
        items(ROOT / "candidate-final/pytest.xml"),
    )
    common = parent.keys() & candidate.keys()
    changed = {
        key: [parent[key], candidate[key]]
        for key in sorted(common)
        if parent[key] != candidate[key]
    }
    after = {
        str(path.relative_to(ROOT)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in subjects
    }
    report = {
        "samples": samples,
        "common_items": len(common),
        "changed_common_items": changed,
        "removed": {key: parent[key] for key in parent.keys() - candidate.keys()},
        "added": {key: candidate[key] for key in candidate.keys() - parent.keys()},
        "supplementary_absence": audits,
        "all_supplementary_absent": all(item["absent"] for item in audits),
        "subjects_before": before,
        "subjects_after": after,
        "stable": before == after,
        "limits": "Supplementary observations never replace failed or incomplete original samples.",
    }
    output = sys.argv[1] if len(sys.argv) > 1 else "comparison.json"
    with (ROOT / output).open("x") as stream:
        json.dump(report, stream, indent=2)
        stream.write("\n")
    if before != after or changed or not all(item["absent"] for item in audits):
        raise SystemExit("comparison or supplementary audit rejected; retain original samples")
    print(f"{len(common)} unchanged common items; {len(audits)} supplementary absences")


if __name__ == "__main__":
    main()
