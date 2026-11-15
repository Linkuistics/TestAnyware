"""Audit frozen startup subjects, rejecting controls and per-item comparisons."""

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
        for group in ("subjects_before", "artifact_sha256"):
            for path, expected in verdict[group].items():
                subject = Path(path) if Path(path).is_absolute() else repo / path
                archived = here / "instruments-initial" / subject.name
                assert digest(subject) == expected or (
                    subject.parent == here and archived.exists() and digest(archived) == expected
                ), (name, path)
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
        if name == "red-controls" or name.startswith("parent-full"):
            assert len(nonpass) == 4 and set(nonpass.values()) == {"failure"}, name
            assert all("test_startup_turns" in key for key in nonpass), name
        elif name == "candidate-controls":
            assert len(nonpass) == 1 and set(nonpass.values()) == {"failure"}, name
            assert all("expired_callback_never_reads" in key for key in nonpass), name
        elif name.startswith("mutant-"):
            assert nonpass and set(nonpass.values()) == {"failure"}, name
        else:
            assert verdict["ok"] and not nonpass, name
    controls = outcomes["candidate-controls-2"]
    assert len(controls) == 7 and set(controls.values()) == {"pass"}
    full = outcomes["candidate-full-1"]
    assert full == outcomes["candidate-full-2"]
    comparisons = {
        key: {
            name: outcomes[name][key]
            for name in ("parent-full-1", "candidate-full-1", "parent-full-2", "candidate-full-2")
        }
        for key in full
    }
    assert all(set(outcomes[name]) == set(full) for name in ("parent-full-1", "parent-full-2"))
    for name, (filename, before, after) in plan["mutations"].items():
        for path in (here / "inputs-final-2").iterdir():
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
        assert digest(path) == digest(here / "inputs-final-2" / path.name), path
    changed = sorted(
        path.name
        for path in (here / "inputs-red").iterdir()
        if path.is_file()
        and path.name != "preparation.json"
        and digest(path) != digest(here / "inputs-final-2" / path.name)
    )
    assert changed == ["worker_capture.py"], changed
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
