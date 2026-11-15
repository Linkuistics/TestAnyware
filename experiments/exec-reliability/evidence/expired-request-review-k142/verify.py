"""Verify retained per-case results, fixture audits and artifact digests."""

import hashlib
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = Path(__file__).resolve().parent
REPO = BASE.parents[3]
PREFIX = "test_persistent_worker_expired_request_is_sticky"
SCHEDULES = {"exited", "stopped", "natural", "ownership-lost"}
MUTANTS = {
    "no-first-reap": SCHEDULES,
    "repeat-reap": {"stopped", "natural"},
    "no-residual": {"stopped", "natural", "ownership-lost"},
    "false-residual": {"exited"},
    "expired-signal": {"stopped", "natural"},
    "no-receipt-residual": {"stopped", "natural", "ownership-lost"},
    "false-receipt-residual": {"exited"},
    "no-ownership-lost": {"ownership-lost"},
    "false-echild-reaped": {"ownership-lost"},
    "retain-lost-owner": {"ownership-lost"},
}
SUPPLEMENTAL = {
    "false-residual",
    "no-receipt-residual",
    "false-receipt-residual",
    "no-ownership-lost",
    "false-echild-reaped",
    "retain-lost-owner",
}


def cases(sample: str) -> dict[str, str]:
    result: dict[str, str] = {}
    for case in ET.parse(BASE / sample / "pytest.xml").iter("testcase"):
        name = case.attrib["classname"] + "::" + case.attrib["name"]
        assert name not in result, (sample, name)
        result[name] = next(
            (status for status in ("failure", "error", "skipped") if case.find(status) is not None),
            "passed",
        )
    return result


def receipt(sample: str) -> None:
    value = json.loads((BASE / sample / "verdict.json").read_text())
    assert value["subjects_before"] == value["subjects_after"], sample
    for name, expected in value["artifact_sha256"].items():
        path = REPO / name
        assert path.is_relative_to(BASE / sample), path
        assert hashlib.sha256(path.read_bytes()).hexdigest() == expected, path
    assert all(check["reaped"] and not check["timed_out"] for check in value["checks"]), sample


def main() -> None:
    samples = sorted(path.parent.name for path in BASE.glob("*/verdict.json"))
    for sample in samples:
        receipt(sample)
    before = json.loads((BASE / "baseline-false-residual/verdict.json").read_text())
    assert before["ok"] is True
    old_control = cases("baseline-false-residual")
    assert list(old_control.values()).count("passed") == 2
    assert list(old_control.values()).count("skipped") == 1
    parent = json.loads((BASE / "parent-full-py312/verdict.json").read_text())
    assert parent["ok"] is True
    candidate = json.loads((BASE / "candidate-full-py312/verdict.json").read_text())
    assert candidate["ok"] is False  # Keep the initial static-import failure visible.
    assert [(check["name"], check["returncode"]) for check in candidate["checks"]] == [
        ("pytest", 0),
        ("ruff", 0),
        ("format", 0),
        ("pyright", 1),
    ]
    static = json.loads((BASE / "candidate-static-py312/verdict.json").read_text())
    assert static["ok"] is True
    supplemental = json.loads((BASE / "candidate-controls-py315/verdict.json").read_text())
    assert supplemental["ok"] is True
    assert set(cases("candidate-controls-py315").values()) == {"passed"}
    assert len(cases("candidate-controls-py315")) == 4
    original, updated = cases("parent-full-py312"), cases("candidate-full-py312")
    common = {name for name in original if PREFIX not in name}
    assert common == {name for name in updated if PREFIX not in name}
    assert len(common) == 920
    assert all(original[name] == updated[name] == "passed" for name in common)
    assert len(updated) == 924 and set(updated.values()) == {"passed"}
    rejected: dict[str, list[str]] = {}
    for mutant, expected in MUTANTS.items():
        for version in ("py312", "py315") if mutant in SUPPLEMENTAL else ("py312",):
            sample = f"mutant-{mutant}-{version}"
            value = json.loads((BASE / sample / "verdict.json").read_text())
            assert value["ok"] is False and value["checks"][0]["returncode"] == 1
            result = cases(sample)
            assert len(result) == 4 and "error" not in result.values(), sample
            failed = {
                name.split("[")[1][:-1] for name, status in result.items() if status == "failure"
            }
            assert failed == expected, (sample, failed, expected)
            rejected[sample] = sorted(failed)
    for sample in ["candidate-full-py312", "candidate-controls-py315", *rejected]:
        value = json.loads((BASE / sample / "verdict.json").read_text())
        fixtures = value["checks"][0]["fixtures"]
        expired = [fixture for fixture in fixtures if "schedule" in fixture]
        assert {fixture["schedule"] for fixture in expired} == SCHEDULES, sample
        assert len(expired) == 4
        assert all(fixture["complete"] and fixture["absent"] for fixture in expired), sample
        assert value["checks"][0]["fixtures_ok"] is True, sample
    for path in (BASE / "baseline-inputs").glob("*.py"):
        if not path.name.startswith("test_"):
            assert path.read_bytes() == (BASE / "candidate-inputs" / path.name).read_bytes()
    for name in ("pyproject.toml", "uv.lock"):
        assert (BASE / "baseline-inputs" / name).read_bytes() == (
            BASE / "candidate-inputs" / name
        ).read_bytes()
    preservation = json.loads((BASE / "prior-after.json").read_text())
    assert preservation["ok"] is True and preservation["changed"] == []
    assert preservation["count"] == len(json.loads((BASE / "prior-before.json").read_text()))
    result = {
        "ok": True,
        "verified_samples": samples,
        "common_cases": len(common),
        "candidate_cases": len(updated),
        "mutant_rejections": rejected,
        "prior_paths_unchanged": preservation["count"],
    }
    with Path(sys.argv[1]).open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(json.dumps({key: value for key, value in result.items() if key != "verified_samples"}))


if __name__ == "__main__":
    main()
