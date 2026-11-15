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
        "parent-final-1",
        "candidate-final-1",
        "parent-final-2",
        "candidate-final-2",
        "candidate-identity-controls",
        "mutant-idle-spin",
        "mutant-cutoff-write",
        "static",
    )
    historical = {
        "test_shutdown::test_declared_shutdown_real_child_phases[blocking-2-deferred-reap]",
        "test_worker_capture::test_persistent_worker_expired_request_is_sticky",
    }
    archived = (
        "red-parent-controls",
        "red-parent-controls-v2",
        "red-reap-cutoff",
        "candidate-controls",
        "parent-full-1",
        "candidate-full-1",
        "parent-full-2",
        "candidate-full-2",
    )
    outcomes: dict[str, dict[str, str]] = {}
    summary: dict[str, object] = {}
    rejected: dict[str, dict[str, str]] = {}
    for name in (*names, *archived):
        condition = here / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["artifact_sha256"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            assert digest(subject) == expected, (name, path)
        assert all(
            check["reaped"]
            and check["fixtures_ok"]
            and (not check["timed_out"] or name == "red-parent-controls")
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
        if name in archived:
            continue
        if name.startswith(("red-", "mutant-")):
            assert not verdict["ok"] and nonpass, name
            assert set(nonpass.values()) == {"failure"}, name
        elif "-final-" in name:
            unexpected = {key: value for key, value in nonpass.items() if key not in historical}
            if unexpected:
                rejected[name] = unexpected
            assert all(
                check["returncode"] == 0 for check in verdict["checks"] if check["name"] != "pytest"
            ), name
        else:
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
        parent = outcomes[f"parent-final-{number}"]
        candidate = outcomes[f"candidate-final-{number}"]
        assert parent and set(parent) | expected == set(candidate)
        unexpected = {
            key: candidate[key]
            for key in parent
            if key not in historical and candidate[key] != "pass"
        }
        if unexpected:
            rejected[f"candidate-final-{number}"] = unexpected
        for key, value in outcomes["candidate-controls-v2"].items():
            assert candidate[key] == value
    comparisons = {
        str(number): {
            key: [
                outcomes[f"parent-final-{number}"][key],
                outcomes[f"candidate-final-{number}"][key],
            ]
            for key in outcomes[f"parent-final-{number}"]
            if outcomes[f"parent-final-{number}"][key] != outcomes[f"candidate-final-{number}"][key]
        }
        for number in (1, 2)
    }
    runtime = here.parents[1]
    candidate = here / "inputs-candidate-verified"
    for source in [
        *runtime.glob("*.py"),
        runtime / "pyproject.toml",
        runtime / "uv.lock",
        repo / "Taskfile.yml",
    ]:
        assert digest(source) == digest(candidate / source.name), source
    parent = here / "inputs-parent-verified"
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
            {
                "conditions": summary,
                "preserved_paths": preservation["count"],
                "per_item_comparison_differences": comparisons,
                "aggregate_accepted": False,
                "integrity_verified": True,
                "comparison_gate_passed": not rejected,
                "comparison_gate_rejections": rejected,
                "historical_failures_remain_open": sorted(historical),
            },
            stream,
            indent=2,
        )
    print(json.dumps(summary, indent=2))
    if rejected:
        raise SystemExit("comparison gate rejected; retained condition-audit.json records failures")


if __name__ == "__main__":
    main()
