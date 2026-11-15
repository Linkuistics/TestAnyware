"""Verify frozen readiness conditions and compare individual suite outcomes."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

from prepare import MUTATIONS

MUTANTS = (
    "ignore-ready",
    "priority",
    "wait-past-deadline",
    "query-registration",
    "retirement-registration",
    "command-reader",
)


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    conditions = (
        "red-parent-controls",
        "candidate-controls",
        "parent-full-1",
        "candidate-full-1",
        "parent-full-2",
        "candidate-full-2",
        "static",
        "static-final",
        "candidate-controls-final",
        "parent-final-1",
        "candidate-final-1",
        "parent-final-2",
        "candidate-final-2",
        *(f"mutant-{name}" for name in MUTANTS),
        *(f"mutant-final-{name}" for name in MUTANTS),
    )
    outcomes: dict[str, dict[str, str]] = {}
    summary: dict[str, object] = {}
    rejections: dict[str, object] = {}
    final_rejections: dict[str, object] = {}
    for name in conditions:
        condition = here / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["subjects_before"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            archived = here / "instruments-at-measurement" / subject.name
            assert digest(subject) == expected or (
                archived.exists() and digest(archived) == expected
            ), (name, path)
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
        elif not verdict["ok"] or nonpass:
            rejections[name] = summary[name]
            if "final" in name:
                final_rejections[name] = summary[name]
    new_cases = {
        key for key, value in outcomes["red-parent-controls"].items() if value == "failure"
    }
    assert len(new_cases) == 9
    assert len(outcomes["red-parent-controls"]) == 19
    comparisons = {}
    for phase in ("full", "final"):
        for number in (1, 2):
            parent = outcomes[f"parent-{phase}-{number}"]
            candidate = outcomes[f"candidate-{phase}-{number}"]
            added = set(candidate) - set(parent)
            assert parent and set(parent) <= set(candidate)
            assert added == new_cases
            assert all(candidate[key] == "pass" for key in added)
            comparisons[f"{phase}-{number}"] = {
                key: [parent.get(key, "absent"), candidate[key]] for key in candidate
            }
            controls = outcomes[
                "candidate-controls-final" if phase == "final" else "candidate-controls"
            ]
            assert len(controls) == 106
            assert all(candidate[key] == value == "pass" for key, value in controls.items())
    runtime = here.parents[1]
    frozen = here / "inputs-candidate-final"
    for source in [
        *runtime.glob("*.py"),
        runtime / "pyproject.toml",
        runtime / "uv.lock",
        repo / "Taskfile.yml",
    ]:
        assert digest(source) == digest(frozen / source.name), source
    parent = here / "inputs-parent"
    changed = sorted(
        path.name
        for path in frozen.iterdir()
        if path.is_file()
        and path.name != "preparation.json"
        and digest(path) != digest(parent / path.name)
    )
    assert changed == ["test_turns.py", "turns.py", "worker_capture.py"], changed
    for source in (here / "parent-source").glob("*.py"):
        assert digest(source) == digest(parent / source.name), source
    for label in ("inputs-candidate", "inputs-candidate-final"):
        for name in MUTANTS:
            filename, before, after = MUTATIONS[name]
            mutant = here / (
                f"inputs-mutant-final-{name}"
                if label.endswith("final")
                else f"inputs-mutant-{name}"
            )
            for path in (here / label).glob("*.py"):
                expected = path.read_text()
                if path.name == filename:
                    assert expected.count(before) == 1
                    expected = expected.replace(before, after)
                assert (mutant / path.name).read_text() == expected, (label, name, path)
    preservation = json.loads((here / "prior-after.json").read_text())
    assert preservation["ok"] and not preservation["changed"]
    with (here / "condition-audit.json").open("x") as stream:
        json.dump(
            {
                "conditions": summary,
                "preserved_paths": preservation["count"],
                "per_item_comparisons": comparisons,
                "aggregate_accepted": False,
                "integrity_verified": True,
                "comparison_gate_passed": not rejections,
                "comparison_gate_rejections": rejections,
                "final_comparison_gate_passed": not final_rejections,
                "final_comparison_gate_rejections": final_rejections,
                "historical_failures": json.loads((here / "observed-deviations.json").read_text())[
                    "historical"
                ],
            },
            stream,
            indent=2,
        )
    print(json.dumps(summary, indent=2))
    if rejections:
        raise SystemExit("comparison gate rejected; all failures retained in condition-audit.json")


if __name__ == "__main__":
    main()
