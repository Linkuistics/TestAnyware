"""Audit retained condition outcomes, immutable subjects and item comparisons."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    summary: dict[str, object] = {}
    items: dict[str, dict[str, str]] = {}
    for condition in sorted(here.iterdir()):
        if not (condition / "verdict.json").is_file():
            continue
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], condition
        for path, expected in verdict["artifact_sha256"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            assert digest(subject) == expected, (condition, path)
        assert all(
            check["reaped"] and check["fixtures_ok"] and not check["timed_out"]
            for check in verdict["checks"]
        ), condition
        tests = (
            ET.parse(condition / "pytest.xml").findall(".//testcase")
            if (condition / "pytest.xml").exists()
            else []
        )
        outcomes = {
            f"{case.get('classname')}::{case.get('name')}": (
                "failure"
                if case.find("failure") is not None
                else "error"
                if case.find("error") is not None
                else "skipped"
                if case.find("skipped") is not None
                else "pass"
            )
            for case in tests
        }
        items[condition.name] = outcomes
        summary[condition.name] = {
            "ok": verdict["ok"],
            "cases": len(tests),
            "nonpass": {key: value for key, value in outcomes.items() if value != "pass"},
        }
        if condition.name.startswith("mutant-"):
            assert not verdict["ok"] and "failure" in outcomes.values(), condition
        if condition.name in {
            "controls-verified",
            "candidate-full-1",
            "candidate-full-2",
            "static",
        }:
            assert verdict["ok"], condition
    comparisons = {}
    for number in (1, 2):
        parent = items[f"parent-full-{number}"]
        candidate = items[f"candidate-full-{number}"]
        comparisons[str(number)] = {
            "common": len(parent.keys() & candidate.keys()),
            "parent_only": sorted(parent.keys() - candidate.keys()),
            "candidate_only": sorted(candidate.keys() - parent.keys()),
            "changed": {
                key: [parent[key], candidate[key]]
                for key in parent.keys() & candidate.keys()
                if parent[key] != candidate[key]
            },
        }
    runtime = here.parents[1]
    for source in [
        *runtime.glob("*.py"),
        runtime / "pyproject.toml",
        runtime / "uv.lock",
        repo / "Taskfile.yml",
    ]:
        assert digest(source) == digest(here / "inputs-verified" / source.name), source
    preservation = json.loads((here / "prior-after.json").read_text())
    assert preservation["ok"]
    with (here / "condition-audit.json").open("x") as stream:
        json.dump(
            {
                "conditions": summary,
                "comparisons": comparisons,
                "preserved_paths": preservation["count"],
            },
            stream,
            indent=2,
        )
    print(json.dumps({"conditions": summary, "comparisons": comparisons}))


if __name__ == "__main__":
    main()
