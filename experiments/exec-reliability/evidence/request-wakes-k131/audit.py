"""Audit frozen wake controls, mutation failures and exact fixture cleanup."""

import difflib
import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    here = Path(__file__).resolve().parent
    repo = here.parents[3]
    summary = {}
    items: dict[str, dict[str, str]] = {}
    expected = {
        "red-idle",
        "red-stale",
        "controls-verified",
        "mutant-ready",
        "mutant-retirement",
        "candidate-full",
        "static",
    }
    for name in sorted(expected):
        condition = here / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected_hash in verdict["artifact_sha256"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            assert digest(subject) == expected_hash, (name, path)
        assert all(
            check["reaped"] and check["fixtures_ok"] and not check["timed_out"]
            for check in verdict["checks"]
        ), name
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
        items[name] = outcomes
        nonpass = {key: value for key, value in outcomes.items() if value != "pass"}
        summary[name] = {"ok": verdict["ok"], "cases": len(tests), "nonpass": nonpass}
        if name.startswith(("red-", "mutant-")):
            assert not verdict["ok"] and nonpass, name
            assert set(nonpass.values()) == {"failure"}, name
            for lost in ("False", "True"):
                key = (
                    "test_turns::test_request_turns_wakes_avoid_idle_spin_and_stale_retirement"
                    f"[{lost}]"
                )
                assert outcomes[key] == "failure", (name, key)
            failures = [case.find("failure") for case in tests]
            assert all(
                "assert " in (failure.get("message", ""))
                for failure in failures
                if failure is not None
            ), name
        else:
            assert verdict["ok"] and not nonpass, name
    assert items["controls-verified"]
    assert all(
        items["candidate-full"].get(key) == value
        for key, value in items["controls-verified"].items()
    )
    candidate = here / "inputs-verified"
    runtime = here.parents[1]
    for source in [
        *runtime.glob("*.py"),
        runtime / "pyproject.toml",
        runtime / "uv.lock",
        repo / "Taskfile.yml",
    ]:
        assert digest(source) == digest(candidate / source.name), source
    differences: dict[str, list[str]] = {}
    for name in ("idle-spin", "ready-delay", "stale-kill", "slow-retirement"):
        frozen = here / f"inputs-{name}"
        changed: list[str] = []
        for source in candidate.iterdir():
            if source.is_file() and source.name != "preparation.json":
                if digest(source) != digest(frozen / source.name):
                    changed.append(source.name)
        assert changed == ["worker_capture.py"], (name, changed)
        differences[name] = list(
            difflib.unified_diff(
                (candidate / "worker_capture.py").read_text().splitlines(),
                (frozen / "worker_capture.py").read_text().splitlines(),
            )
        )
        assert sum(line.startswith("@@") for line in differences[name]) == 1, name
    preservation = json.loads((here / "prior-after.json").read_text())
    assert preservation["ok"] and not preservation["changed"]
    with (here / "condition-audit.json").open("x") as stream:
        json.dump(
            {
                "conditions": summary,
                "mutation_diffs": differences,
                "preserved_paths": preservation["count"],
            },
            stream,
            indent=2,
        )
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
