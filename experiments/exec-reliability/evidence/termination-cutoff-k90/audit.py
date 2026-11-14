"""Verify retained identities, exact failing cases and unchanged prior evidence."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cases(name: str) -> tuple[set[str], set[str]]:
    nodes = ET.parse(ROOT / name / "pytest.xml").findall(".//testcase")
    identities = {f"{node.attrib['classname']}::{node.attrib['name']}" for node in nodes}
    failures = {
        f"{node.attrib['classname']}::{node.attrib['name']}"
        for node in nodes
        if node.find("failure") is not None or node.find("error") is not None
    }
    assert len(identities) == len(nodes), name
    return identities, failures


def main() -> None:
    prior = json.loads((ROOT / "prior-evidence.json").read_text())
    assert all(digest(Path(path)) == expected for path, expected in prior.items())
    results = {}
    for name in ("red-1", "green-1", "green-2", "outer-deadline-1", "early-receipt-1", "full-1"):
        verdict = json.loads((ROOT / name / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["subjects_before"].items():
            source = Path(path)
            if name == "red-1" and source == ROOT.parents[3] / "Taskfile.yml":
                source = ROOT / "red-inputs/Taskfile.yml"
            if name == "green-1" and source == ROOT / "run_condition.py":
                source = ROOT / "run_condition-green-1.py"
            assert digest(source) == expected, (name, path)
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(Path(path)) == expected, (name, path)
        assert all(item["reaped"] and not item["timed_out"] for item in verdict["checks"]), name
        identities, failures = cases(name)
        results[name] = {"total": len(identities), "failures": sorted(failures)}
    selected, clean = cases("green-2")
    assert len(selected) == 20 and not clean
    red = cases("red-1")[1]
    cutoff = cases("outer-deadline-1")[1]
    timestamp = cases("early-receipt-1")[1]
    assert (
        cases("red-1")[0] == cases("outer-deadline-1")[0] == cases("early-receipt-1")[0] == selected
    )
    assert len(cutoff) == 6 and len(timestamp) == 2
    assert not cutoff & timestamp and red == cutoff | timestamp
    assert all("receipt_records_admission_sample" in case for case in timestamp)
    assert all(
        "refuses_late_signals" in case or "checks_observer_return" in case for case in cutoff
    )
    assert cases("green-1")[0] == selected and not cases("green-1")[1]
    assert len(cases("full-1")[0]) == 454 and not cases("full-1")[1]
    mode_red, mode_failed = cases("mode-red-1")
    mode_green, mode_clean = cases("mode-green-1")
    assert len(mode_red) == 3 and mode_red == mode_green == mode_failed and not mode_clean
    # These mode-only runs predate this auditor: their JUnit and raw streams are
    # retained, but they did not sample test-source hashes before dispatch.
    report = {
        "ok": True,
        "prior_files_unchanged": len(prior),
        "conditions": results,
        "mode_red": sorted(mode_failed),
        "mode_green": len(mode_green),
        "mode_limit": "No pre-dispatch test-source hash sample for the two mode-only runs",
    }
    target = ROOT / "audit.json"
    with target.open("x") as output:
        output.write(json.dumps(report, indent=2) + "\n")
    print(f"Verified {len(prior)} prior files and six measured conditions")


if __name__ == "__main__":
    main()
