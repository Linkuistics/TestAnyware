"""Check retained interval controls, real ownership receipts and prior evidence bytes."""

import hashlib
import json
import os
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def cases(path: Path) -> dict[str, str]:
    result = {}
    for case in ET.parse(path).iter("testcase"):
        key = f"{case.attrib['classname']}::{case.attrib['name']}"
        assert key not in result, key
        result[key] = (
            "failed"
            if case.find("failure") is not None or case.find("error") is not None
            else "skipped"
            if case.find("skipped") is not None
            else "passed"
        )
    return result


def main() -> None:
    output = BASE / "audit-01"
    output.mkdir(exist_ok=False)
    verdicts = {
        name: json.loads((BASE / name / "verdict.json").read_text())
        for name in ("red-02", "green-01", "full-01")
    }
    subjects = {
        Path(path)
        for verdict in verdicts.values()
        for path in (*verdict["artifact_sha256"], *verdict["subjects_after"])
    }
    subjects.update(BASE / name / "verdict.json" for name in verdicts)
    subjects.update([Path(__file__), BASE / "prior-evidence.json"])
    subjects.update((ROOT / "experiments/exec-reliability").glob("*.py"))
    before = {str(path): digest(path) for path in sorted(subjects)}
    (output / "declaration.json").write_text(json.dumps({"subjects_before": before}, indent=2))
    for name, verdict in verdicts.items():
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["subjects_after"].items():
            assert digest(Path(path)) == expected, path
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(Path(path)) == expected, path
        assert all(item["reaped"] and not item["timed_out"] for item in verdict["checks"])
        assert verdict["ok"] is (name != "red-02"), name
    red = cases(BASE / "red-02/pytest.xml")
    green = cases(BASE / "green-01/pytest.xml")
    full = cases(BASE / "full-01/pytest.xml")
    assert len(red) == len(green) == 10 and red.keys() == green.keys()
    assert set(red.values()) == {"failed"}
    failures = list(ET.parse(BASE / "red-02/pytest.xml").iter("failure"))
    assert len(failures) == 10
    assert all("KeyError: 'probe_started_monotonic'" in item.attrib["message"] for item in failures)
    assert set(green.values()) == {"passed"}
    assert len(full) == 424 and set(full.values()) == {"passed"}
    assert all(full[key] == value for key, value in green.items())
    for path in (BASE / "inputs-green").iterdir():
        if path.is_file():
            assert digest(path) == digest(ROOT / "experiments/exec-reliability" / path.name)
    receipts = {}
    for name in ("green-01", "full-01"):
        raw_paths = sorted((BASE / name / "runs").glob("worker-group-snapshot-[0-9]*.json"))
        audit_paths = sorted((BASE / name / "runs").glob("worker-group-snapshot-audit-*.json"))
        assert len(raw_paths) == len(audit_paths) == 1
        raw = json.loads(raw_paths[0].read_text())
        audit = json.loads(audit_paths[0].read_text())
        helper = raw["helper"]
        worker = helper["worker"]
        query = worker["published_times"]
        assert raw["error"] is None and raw["state"] == "live"
        assert raw["probe_outcome"] == "permitted" and raw["probe_errno"] is None
        assert helper["reaped"] and worker["reaped"]
        assert audit == {"pid": raw["fixture_pid"], "returncode": -9, "reaped": True}
        assert (
            raw["started_monotonic"]
            <= worker["dispatch_monotonic"]
            <= query["started_monotonic"]
            <= query["finished_monotonic"]
            <= worker["receipt_monotonic"]
            <= raw["probe_started_monotonic"]
            <= raw["probe_finished_monotonic"]
            <= raw["finished_monotonic"]
        )
        receipts[name] = {"observation": str(raw_paths[0]), "fixture_audit": str(audit_paths[0])}
    prior = json.loads((BASE / "prior-evidence.json").read_text())
    after_prior = {}
    for name in prior:
        path = ROOT / name
        after_prior[name] = "symlink:" + os.readlink(path) if path.is_symlink() else digest(path)
    differences = [name for name in prior if prior[name] != after_prior[name]]
    after = {str(path): digest(path) for path in sorted(subjects)}
    result = {
        "ok": before == after and not differences,
        "subjects_before": before,
        "subjects_after": after,
        "red_cases": red,
        "green_cases": green,
        "full_cases": full,
        "real_receipts": receipts,
        "prior_entries_checked": len(prior),
        "prior_differences": differences,
        "prior_before_map_sha256": hashlib.sha256(
            json.dumps(prior, sort_keys=True).encode()
        ).hexdigest(),
        "prior_after_map_sha256": hashlib.sha256(
            json.dumps(after_prior, sort_keys=True).encode()
        ).hexdigest(),
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"audit ok={result['ok']}; {len(prior)} prior entries; evidence {output}")
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
