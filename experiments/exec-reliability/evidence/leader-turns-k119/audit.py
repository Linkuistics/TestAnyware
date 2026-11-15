"""Audit retained reap conditions, subject identities and exact child cleanup."""

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
    for name, cases, failures in (
        ("red", 4, 4),
        ("green-controls", 22, 0),
        ("late-poll", 4, 1),
        ("sleep-in-turn", 4, 3),
        ("full", 860, 0),
    ):
        condition = here / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["ok"] is (failures == 0), name
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["subjects_before"].items():
            subject = Path(path) if Path(path).is_absolute() else repo / path
            if subject == here / "audit.py" and name != "full":
                subject = here / ("audit.red.py.txt" if name == "red" else "audit.focused.py.txt")
            assert digest(subject) == expected, (name, path)
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(repo / path) == expected, (name, path)
        assert all(
            item["reaped"] and item["fixtures_ok"] and not item["timed_out"]
            for item in verdict["checks"]
        ), name
        checks = verdict["checks"]
        assert checks[0]["returncode"] == (1 if failures else 0), name
        assert all(item["returncode"] == 0 for item in checks[1:]), name
        tests = ET.parse(condition / "pytest.xml").findall(".//testcase")
        assert len(tests) == cases, (name, len(tests))
        failed = [test.attrib["name"] for test in tests if test.find("failure") is not None]
        assert len(failed) == failures, (name, failed)
        assert all(test.find("error") is None and test.find("skipped") is None for test in tests)
        if failures:
            assert all(
                "test_leader_reap_turn_yields_and_stops_at_fixed_cutoff" in test for test in failed
            )
            if name == "late-poll":
                assert failed == [
                    "test_leader_reap_turn_yields_and_stops_at_fixed_cutoff[withheld]"
                ]
        children = sorted((condition / "runs").glob("leader-turn-*.json"))
        assert len(children) == 4, (name, len(children))
        for child in children:
            assert json.loads(child.read_text())["after"]["state"] == "absent", child
        summary[name] = {"cases": cases, "failed": failed, "leader_turn_children": len(children)}
    final = here / "inputs-final"
    runtime = here.parents[1]
    for source in [
        *runtime.glob("*.py"),
        runtime / "pyproject.toml",
        runtime / "uv.lock",
        repo / "Taskfile.yml",
    ]:
        assert digest(source) == digest(final / source.name), source
    preservation = json.loads((here / "prior-after.json").read_text())
    assert preservation["ok"] is True
    summary["preserved_paths"] = preservation["count"]
    with (here / "condition-audit.json").open("x") as stream:
        json.dump(summary, stream, indent=2)
    print(json.dumps(summary))


if __name__ == "__main__":
    main()
