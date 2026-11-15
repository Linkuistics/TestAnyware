"""Check immutable run receipts and child audits without repeating the experiments."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
EXPECTED = {
    "red": (42, 42),
    "green-controls": (151, 0),
    "green-full": (797, 1),
    "self-check": (797, 0),
    "review-controls": (238, 0),
}


def main() -> None:
    subjects: dict[Path, str] = {}

    def read(path: Path) -> bytes:
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        assert subjects.setdefault(path, digest) == digest, path
        return data

    read(Path(__file__).resolve())
    read(REPO / "Taskfile.yml")
    results: dict[str, object] = {}
    per_case: dict[str, dict[str, bool]] = {}
    for name, expectation in EXPECTED.items():
        folder = HERE / name
        verdict = json.loads(read(folder / "verdict.json"))
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for key, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256(read(REPO / key)).hexdigest() == digest, key
        checks = verdict["checks"]
        assert all(item["reaped"] and not item["timed_out"] for item in checks), name
        cases = list(ET.fromstring(read(folder / "pytest.xml")).iter("testcase"))
        assert all(item.find("error") is None and item.find("skipped") is None for item in cases)
        failed = [item.attrib["name"] for item in cases if item.find("failure") is not None]
        assert (len(cases), len(failed)) == expectation, name
        assert verdict["ok"] is (not failed), name
        if name == "red":
            assert all("incremental-" in item for item in failed)
            assert all(
                "has no attribute 'begin_close'" in (item.findtext("failure") or "")
                for item in cases
            )
        if name == "green-full":
            assert failed == ["test_review_group_kill_precedes_inspection[ignore-term]"]
        per_case[name] = {
            f"{item.attrib['classname']}::{item.attrib['name']}": item.find("failure") is None
            for item in cases
        }
        assert len(per_case[name]) == len(cases)
        audits: list[str] = []
        for pattern in ("first-reap-*.json", "shutdown-*.json", "persistent-close-audit-*.json"):
            for path in sorted((folder / "runs").glob(pattern)):
                data = json.loads(read(path))
                assert data.get("audit", data)["reaped"] is True, path
                audits.append(str(path.relative_to(HERE)))
        if name in ("green-controls", "green-full", "self-check"):
            assert len(list((folder / "runs").glob("shutdown-incremental-*.json"))) == 42
            assert len(list((folder / "runs").glob("shutdown-blocking-*.json"))) == 42
        results[name] = {
            "tests": len(cases),
            "failures": failed,
            "verified_artifacts": len(verdict["artifact_sha256"]),
            "exact_child_audits": audits,
        }
    before = per_case["green-full"]
    after = per_case["self-check"]
    assert before.keys() == after.keys()
    differences = [key for key in before if before[key] != after[key]]
    assert differences == [
        "test_controller::test_review_group_kill_precedes_inspection[ignore-term]"
    ]
    green = HERE / "green-inputs"
    for path in [*sorted(green.glob("*.py")), green / "pyproject.toml", green / "uv.lock"]:
        assert read(path) == read(HERE.parents[1] / path.name), path
    prior = json.loads(read(HERE / "prior-after.json"))
    assert prior["ok"] and not prior["changed"]
    assert prior["snapshot"] == json.loads(read(HERE / "prior-before.json"))
    final = {path: hashlib.sha256(path.read_bytes()).hexdigest() for path in subjects}
    assert subjects == final
    result = {
        "conditions": results,
        "changed_case_outcomes": differences,
        "prior_paths": prior["count"],
        "subjects_before": {str(p.relative_to(REPO)): h for p, h in subjects.items()},
        "subjects_after": {str(p.relative_to(REPO)): h for p, h in final.items()},
    }
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(f"Verified {len(results)} conditions; {prior['count']} prior paths unchanged")


if __name__ == "__main__":
    main()
