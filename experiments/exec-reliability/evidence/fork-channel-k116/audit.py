"""Audit exact regressions, source comparisons, bounds and retained child outcomes."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
RESULT_FAILURE = "test_fork_failure_lost_result_eof_order[result]"
SHUTDOWN_FAILURE = "test_declared_shutdown_real_child_phases[blocking-2-deferred-reap]"


def main() -> None:
    results = {}
    source_sets: dict[str, dict[str, str]] = {}
    case_sets: dict[str, set[tuple[str, str]]] = {}
    shutdown_audits: dict[str, object] = {}
    conditions: tuple[tuple[str, set[str], int | None], ...] = (
        ("red-order", {RESULT_FAILURE}, 2),
        ("green-controls", set(), 37),
        ("omit-cleanup-eof", {RESULT_FAILURE}, 2),
        ("parent-full-1", {RESULT_FAILURE, SHUTDOWN_FAILURE}, 854),
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
        declaration = json.loads((folder / "declaration.json").read_text())
        assert declaration["subjects_before"] == verdict["subjects_before"]
        for check, (_, _, budget) in zip(checks, declaration["commands"], strict=True):
            assert check["elapsed"] < budget, (name, check["name"])
        if "full" in name:
            case_sets[name] = {(c.attrib["classname"], c.attrib["name"]) for c in cases}
            assert len(case_sets[name]) == 854
            shutdown = sorted((folder / "runs").glob("shutdown-*.json"))
            assert len(shutdown) == 84, name
            identities: set[tuple[str, int, str]] = set()
            late: list[dict[str, object]] = []
            for path in shutdown:
                fixture = json.loads(path.read_text())
                audit = fixture["audit"]
                assert audit["reaped"] and audit["source"] == "original exact-child WNOHANG"
                identities.add((audit["mode"], audit["sample"], audit["driver"]))
                end = fixture["runtime"]["shutdown"]["kill_deadline_monotonic"]
                overruns = [t - end for t in fixture["probes"] if t >= end]
                if overruns and audit["mode"] != "expired":
                    late.append({"receipt": path.name, "seconds": overruns, "audit": audit})
            assert len(identities) == 84
            assert len(late) == (1 if name == "parent-full-1" else 0), (name, late)
            shutdown_audits[name] = {"reaped": len(shutdown), "late_probes": late}
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
    assert all(cases == case_sets["parent-full-1"] for cases in case_sets.values())
    assert source_sets["parent-full-1"] == source_sets["parent-full-2"]
    assert source_sets["candidate-full-1"] == source_sets["candidate-full-2"]
    parent_text = (HERE / "inputs-parent/worker_capture.py").read_text()
    candidate_text = (HERE / "inputs-candidate/worker_capture.py").read_text()
    addition = "                                if not chunk:\n"
    addition += "                                    closed_channels.add(0)\n"
    assert candidate_text.count(addition) == 1
    assert candidate_text.replace(addition, "") == parent_text
    # The shipping runtime and tests must still match the measured candidate.
    for key, digest in source_sets["candidate-full-2"].items():
        current = (
            REPO / "Taskfile.yml"
            if key == "Taskfile.yml"
            else REPO / "experiments/exec-reliability" / key
        )
        assert hashlib.sha256(current.read_bytes()).hexdigest() == digest, current
    measured = HERE / "full-suite-audit.py"
    audit_key = str((HERE / "audit.py").relative_to(REPO))
    for name in case_sets:
        verdict = json.loads((HERE / name / "verdict.json").read_text())
        assert (
            hashlib.sha256(measured.read_bytes()).hexdigest()
            == verdict["subjects_before"][audit_key]
        )
    prior = json.loads((HERE / "prior-after.json").read_text())
    assert prior["ok"] and not prior["changed"]
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump(
            {
                "ok": True,
                "conditions": results,
                "shutdown_audits": shutdown_audits,
                "prior_count": prior["count"],
                "audit_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            },
            stream,
            indent=2,
        )
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
