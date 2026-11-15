"""Audit k136's frozen controls, one-site mutants and per-case suite outcomes."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    plan = json.loads((here / "comparison-plan.json").read_text())
    outcomes: dict[str, dict[str, str]] = {}
    summary: dict[str, object] = {}
    fixtures = 0
    for name in plan["conditions"]:
        condition = here / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["subjects_before"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            archived = here / "instruments-initial" / subject.name
            assert digest(subject) == expected or (
                subject.parent == here and archived.exists() and digest(archived) == expected
            ), (name, path)
        for path, expected in verdict["artifact_sha256"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            assert digest(subject) == expected, (name, path)
        for check in verdict["checks"]:
            assert check["reaped"] and check["fixtures_ok"] and not check["timed_out"], name
            fixtures += len(check["fixtures"])
            if check["name"] != "pytest":
                assert check["returncode"] == 0, (name, check["name"])
        items = {
            f"{case.get('classname')}::{case.get('name')}": (
                "failure"
                if case.find("failure") is not None
                else "error"
                if case.find("error") is not None
                else "skipped"
                if case.find("skipped") is not None
                else "pass"
            )
            for case in ET.parse(condition / "pytest.xml").findall(".//testcase")
        }
        outcomes[name] = items
        nonpass = {key: value for key, value in items.items() if value != "pass"}
        summary[name] = {"ok": verdict["ok"], "cases": len(items), "nonpass": nonpass}
        if name in plan["expected_failure_conditions"]:
            expected = plan["expected_failure_conditions"][name]
            assert not verdict["ok"] and len(nonpass) == expected, name
            assert set(nonpass.values()) == {"failure"}, name
        elif name != "parent-full":
            assert verdict["ok"] and not nonpass, name

    controls = outcomes["candidate-controls-2"]
    assert len(controls) == 7 and set(controls.values()) == {"pass"}
    assert controls == outcomes["candidate-controls-3"]
    assert set(controls) == set(outcomes["red-controls-final"])
    assert all(
        key in controls and "readiness_failure_preserves_safety_retirement" in key
        for key, value in outcomes["red-controls-final"].items()
        if value == "failure"
    )
    full = outcomes["candidate-full-1"]
    assert len(full) == plan["candidate_expected_cases"] and set(full.values()) == {"pass"}
    assert full == outcomes["candidate-full-2"]
    assert all(full[key] == value for key, value in controls.items())
    parent = outcomes["parent-full"]
    assert set(parent) == set(full)
    comparisons = {key: [parent[key], full[key]] for key in full}
    for name, failures in plan["mutant_expected_failures"].items():
        items = outcomes[f"mutant-{name}"]
        assert set(items) == set(controls)
        assert len([value for value in items.values() if value == "failure"]) == failures
        filename, before, after = plan["mutations"][name]
        for path in (here / "inputs-final").iterdir():
            if not path.is_file() or path.name == "preparation.json":
                continue
            expected = path.read_bytes()
            if path.name == filename:
                assert expected.count(before.encode()) == 1
                expected = expected.replace(before.encode(), after.encode())
            assert (here / f"inputs-mutant-{name}" / path.name).read_bytes() == expected, name
    runtime = here.parents[1]
    for path in [
        *runtime.glob("*.py"),
        runtime / "pyproject.toml",
        runtime / "uv.lock",
        repo / "Taskfile.yml",
    ]:
        assert digest(path) == digest(here / "inputs-final" / path.name), path
    changed = sorted(
        path.name
        for path in (here / "inputs-red-final").iterdir()
        if path.is_file()
        and path.name != "preparation.json"
        and digest(path) != digest(here / "inputs-final" / path.name)
    )
    assert changed == ["turns.py"], changed
    preservation = json.loads((here / "prior-after.json").read_text())
    assert preservation["ok"] and not preservation["changed"]
    result = {
        "integrity_verified": True,
        "aggregate_accepted": False,
        "conditions": summary,
        "per_item_parent_candidate": comparisons,
        "preserved_paths": preservation["count"],
        "fixture_absence_audits": fixtures,
        "historical_failures": json.loads((here / "observed-deviations.json").read_text()),
    }
    with (here / "condition-audit.json").open("x") as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
