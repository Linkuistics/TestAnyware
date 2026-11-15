"""Audit retained startup frame conditions, source mutations and fixture absence."""

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
                archived = list(here.glob(f"instruments-*/{subject.name}"))
                assert digest(subject) == expected or (
                    subject.parent == here and any(digest(old) == expected for old in archived)
                ), (name, path)
        for check in verdict["checks"]:
            assert check["reaped"] and check["fixtures_ok"] and not check["timed_out"], name
            assert all(not fixture["signaled"] for fixture in check["fixtures"]), name
            fixtures += len(check["fixtures"])
            if check["name"] != "pytest":
                assert check["returncode"] == 0, (name, check["name"])
        cases = {
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
        outcomes[name] = cases
        nonpass = {key: result for key, result in cases.items() if result != "pass"}
        summary[name] = {"ok": verdict["ok"], "cases": len(cases), "nonpass": nonpass}
        if name in {"candidate-controls-1", "candidate-controls-2"}:
            assert len(nonpass) == 1 and all("nesting" in key for key in nonpass), name
        elif name.startswith("mutant-"):
            assert nonpass and set(nonpass.values()) == {"failure"}, name
            assert all("test_startup_frame_" in key for key in nonpass), name
        else:
            assert verdict["ok"] and not nonpass, name
    controls = outcomes["candidate-controls-3"]
    assert len(controls) == 14 and set(controls.values()) == {"pass"}
    assert controls == outcomes["candidate-controls-4"]
    assert outcomes["candidate-full-1"] == outcomes["candidate-full-2"]
    for name, (filename, before, after) in plan["mutations"].items():
        for path in (here / "inputs-candidate-v3").iterdir():
            if not path.is_file() or path.name == "preparation.json":
                continue
            expected = path.read_bytes()
            if path.name == filename:
                assert expected.count(before.encode()) == 1
                expected = expected.replace(before.encode(), after.encode())
            assert (here / f"inputs-mutant-{name}" / path.name).read_bytes() == expected, name
    runtime = here.parents[1]
    for path in [*runtime.glob("*.py"), runtime / "pyproject.toml", runtime / "uv.lock"]:
        assert path.read_bytes() == (here / "inputs-candidate-v3" / path.name).read_bytes(), path
    result = {"conditions": summary, "fixture_audits": fixtures, "outcomes": outcomes}
    with (here / "audit-summary.json").open("x") as stream:
        json.dump(result, stream, indent=2)
    print(json.dumps(summary, indent=2))
    print(f"{fixtures} exact fixture absence checks; frozen subjects and mutations verified")


if __name__ == "__main__":
    main()
