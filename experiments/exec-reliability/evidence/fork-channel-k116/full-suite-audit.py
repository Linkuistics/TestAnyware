"""Audit exact regressions, source comparisons, bounds and retained child outcomes."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
RESULT_FAILURE = "test_fork_failure_lost_result_eof_order[result]"


def main() -> None:
    results = {}
    source_sets: dict[str, dict[str, str]] = {}
    conditions: tuple[tuple[str, set[str], int | None], ...] = (
        ("red-order", {RESULT_FAILURE}, 2),
        ("green-controls", set(), None),
        ("omit-cleanup-eof", {RESULT_FAILURE}, 2),
        ("parent-full-1", {RESULT_FAILURE}, 854),
        ("candidate-full-1", set(), 854),
        ("parent-full-2", {RESULT_FAILURE}, 854),
        ("candidate-full-2", set(), 854),
    )
    for name, expected, total in conditions:
        folder = HERE / name
        verdict = json.loads((folder / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for key, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256((REPO / key).read_bytes()).hexdigest() == digest, key
        cases = list(ET.parse(folder / "pytest.xml").getroot().iter("testcase"))
        assert not any(c.find("error") is not None or c.find("skipped") is not None for c in cases)
        failed = {c.attrib["name"] for c in cases if c.find("failure") is not None}
        assert failed == expected, (name, failed)
        assert total is None or len(cases) == total, (name, len(cases))
        checks = verdict["checks"]
        assert all(c["reaped"] and not c["timed_out"] and c["fixtures_ok"] for c in checks)
        assert checks[0]["returncode"] == (1 if expected else 0)
        assert all(c["returncode"] == 0 for c in checks[1:]), name
        assert verdict["ok"] is (not expected), name
        if "full" in name:
            assert len(checks) == 4
            assert len(checks[0]["fixtures"]) == 2
            assert all(f["absent"] for f in checks[0]["fixtures"])
            source_sets[name] = {
                Path(key).name: digest
                for key, digest in verdict["subjects_before"].items()
                if str(HERE.relative_to(REPO)) + "/inputs-" in key
            }
        orders = sorted((folder / "runs").glob("worker-fork-eof-*.json"))
        assert len(orders) == 2, (name, orders)
        for path in orders:
            receipt = json.loads(path.read_text())
            worker = receipt["worker"]
            assert worker["reaped"] and worker["returncode"] == 70, path
            assert worker["fork_failure"]["errno"] == 35, path
            assert receipt["pid"] is None and worker["helper_identity"] is None, path
            assert not worker["helper_obligation_retained"], path
            unknown = bool(expected) and "-result-" in path.name
            assert worker["unknown_helper_creation"] is unknown, path
            assert bool(receipt["residual_remedy"]) is unknown, path
            assert receipt["error"] is not None, path
            assert receipt["error_errno"] == (None if unknown else 35), path
        results[name] = {"cases": len(cases), "failed": sorted(failed)}
    for index in (1, 2):
        parent = source_sets[f"parent-full-{index}"]
        candidate = source_sets[f"candidate-full-{index}"]
        assert parent.keys() == candidate.keys()
        changed = [key for key in parent if parent[key] != candidate[key]]
        assert changed == ["worker_capture.py"], changed
    assert source_sets["parent-full-1"] == source_sets["parent-full-2"]
    assert source_sets["candidate-full-1"] == source_sets["candidate-full-2"]
    parent_text = (HERE / "inputs-parent/worker_capture.py").read_text()
    candidate_text = (HERE / "inputs-candidate/worker_capture.py").read_text()
    addition = "                                if not chunk:\n"
    addition += "                                    closed_channels.add(0)\n"
    assert candidate_text.count(addition) == 1
    assert candidate_text.replace(addition, "") == parent_text
    prior = json.loads((HERE / "prior-after.json").read_text())
    assert prior["ok"] and not prior["changed"]
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump(
            {"ok": True, "conditions": results, "prior_count": prior["count"]}, stream, indent=2
        )
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
