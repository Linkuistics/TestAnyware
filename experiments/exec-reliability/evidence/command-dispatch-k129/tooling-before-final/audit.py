"""Verify retained hashes and compare individual parent/candidate test outcomes."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    names = (
        "red-parent-controls-v3",
        "candidate-controls-v2",
        "parent-full-1",
        "candidate-full-1",
        "parent-full-2",
        "candidate-full-2",
        "mutant-idle-spin",
        "mutant-cutoff-write",
        "static",
    )
    outcomes: dict[str, dict[str, str]] = {}
    summary: dict[str, object] = {}
    for name in names:
        condition = here / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["artifact_sha256"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            assert digest(subject) == expected, (name, path)
        assert all(
            check["reaped"] and check["fixtures_ok"] and not check["timed_out"]
            for check in verdict["checks"]
        ), name
        tests = (
            ET.parse(condition / "pytest.xml").findall(".//testcase")
            if (condition / "pytest.xml").exists()
            else []
        )
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
            for case in tests
        }
        outcomes[name] = items
        nonpass = {key: value for key, value in items.items() if value != "pass"}
        summary[name] = {"ok": verdict["ok"], "cases": len(items), "nonpass": nonpass}
        if name.startswith(("red-", "mutant-")):
            assert not verdict["ok"] and nonpass, name
            assert set(nonpass.values()) == {"failure"}, name
        elif not name.startswith("parent-full"):
            assert verdict["ok"] and not nonpass, name
    expected = {
        f"test_turns::test_command_dispatch_turns_yield_and_stop[{mode}]"
        for mode in ("ready", "short", "blocked", "broken", "cutoff", "reap-cutoff")
    }
    assert set(outcomes["red-parent-controls-v3"]) == expected
    assert set(outcomes["red-parent-controls-v3"].values()) == {"failure"}
    assert (
        outcomes["mutant-idle-spin"][
            "test_turns::test_command_dispatch_turns_yield_and_stop[blocked]"
        ]
        == "failure"
    )
    assert (
        outcomes["mutant-cutoff-write"][
            "test_turns::test_command_dispatch_turns_yield_and_stop[reap-cutoff]"
        ]
        == "failure"
    )
    for number in (1, 2):
        parent = outcomes[f"parent-full-{number}"]
        candidate = outcomes[f"candidate-full-{number}"]
        assert parent and set(parent) | expected == set(candidate)
        assert all(candidate[key] == "pass" for key in parent)
        for key, value in outcomes["candidate-controls-v2"].items():
            assert candidate[key] == value
    assert outcomes["parent-full-1"] == outcomes["parent-full-2"]
    assert outcomes["candidate-full-1"] == outcomes["candidate-full-2"]
    runtime = here.parents[1]
    candidate = here / "inputs-candidate-final"
    for source in [
        *runtime.glob("*.py"),
        runtime / "pyproject.toml",
        runtime / "uv.lock",
        repo / "Taskfile.yml",
    ]:
        assert digest(source) == digest(candidate / source.name), source
    parent = here / "inputs-parent-final"
    changed = [
        path.name
        for path in candidate.iterdir()
        if path.is_file()
        and path.name != "preparation.json"
        and digest(path) != digest(parent / path.name)
    ]
    assert changed == ["worker_capture.py"], changed
    preservation = json.loads((here / "prior-after.json").read_text())
    assert preservation["ok"] and not preservation["changed"]
    with (here / "condition-audit.json").open("x") as stream:
        json.dump(
            {"conditions": summary, "preserved_paths": preservation["count"]}, stream, indent=2
        )
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
