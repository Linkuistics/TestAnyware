"""Audit the finite review controls, their source deltas and exact-child receipts."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
OPEN_FAILURE = "test_fork_failure_empty_control_channel[open]"


def main() -> None:
    plan = json.loads((HERE / "comparison-plan.json").read_text())
    results = {}
    for condition in plan["conditions"]:
        name = condition["name"]
        folder = HERE / name
        verdict = json.loads((folder / "verdict.json").read_text())
        declaration = json.loads((folder / "declaration.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        assert declaration["subjects_before"] == verdict["subjects_before"], name
        for key, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256((REPO / key).read_bytes()).hexdigest() == digest, key
        cases = list(ET.parse(folder / "pytest.xml").getroot().iter("testcase"))
        assert not any(c.find("error") is not None or c.find("skipped") is not None for c in cases)
        failed = {c.attrib["name"] for c in cases if c.find("failure") is not None}
        assert failed == set(condition["failures"]), (name, failed)
        assert len(cases) == condition["cases"], (name, len(cases))
        checks = verdict["checks"]
        assert len(checks) == (4 if condition["mode"] == "full" else 1), name
        assert all(c["reaped"] and not c["timed_out"] and c["fixtures_ok"] for c in checks), name
        assert checks[0]["returncode"] == (1 if failed else 0), name
        assert all(c["returncode"] == 0 for c in checks[1:]), name
        assert verdict["ok"] is (not failed), name
        for check, (_, _, budget) in zip(checks, declaration["commands"], strict=True):
            assert check["elapsed"] < budget, (name, check["name"])
        fixtures = sorted((folder / "runs").glob("empty-control-*.json"))
        assert len(fixtures) == (0 if name.startswith("old-") else 2), name
        for path in fixtures:
            fixture = json.loads(path.read_text())
            assert fixture["holder_reaped"] is True and fixture["holder_returncode"] == 0, path
            assert fixture["control_eagain"] > 0, path
            assert fixture["result_eof"] is True and fixture["failure_seen"] is True, path
            if fixture["mode"] == "open":
                assert fixture["holder_alive_after_capture"] is True, path
                assert fixture["control_eof_monotonic"] is None, path
            else:
                assert fixture["query_cutoff"] <= fixture["release_monotonic"], path
                assert fixture["query_cutoff"] <= fixture["control_eof_monotonic"], path
                assert fixture["control_eof_monotonic"] < fixture["deadline"], path
        receipts = sorted((folder / "runs").glob("worker-empty-control-*.json"))
        assert len(receipts) == len(fixtures), name
        for path in receipts:
            receipt = json.loads(path.read_text())
            worker = receipt["worker"]
            assert worker["reaped"] and worker["returncode"] == 70, path
            assert worker["fork_failure"]["errno"] == 35, path
            assert receipt["pid"] is None and worker["helper_identity"] is None, path
            assert not worker["helper_obligation_retained"] and not worker["creation_conflict"], (
                path
            )
            unknown = "-open-" in path.name and OPEN_FAILURE not in failed
            assert worker["unknown_helper_creation"] is unknown, path
            assert bool(receipt["residual_remedy"]) is unknown, path
            assert receipt["error"] is not None, path
            assert receipt["error_errno"] == (None if unknown else 35), path
        if condition["mode"] == "full":
            shutdown = sorted((folder / "runs").glob("shutdown-*.json"))
            assert len(shutdown) == 84, name
            for path in shutdown:
                fixture = json.loads(path.read_text())
                assert fixture["audit"]["reaped"], path
                assert fixture["audit"]["source"] == "original exact-child WNOHANG", path
            assert len(checks[0]["fixtures"]) == 2, name
        results[name] = {"cases": len(cases), "failures": sorted(failed)}

    baseline = HERE / "inputs-old"
    candidate = HERE / "inputs-candidate"
    assert (baseline / "worker_capture.py").read_bytes() == (
        candidate / "worker_capture.py"
    ).read_bytes()
    for key in ("result-eof-only", "eagain-is-eof", "omit-eager-conflict"):
        # Exact declared single-site mutation; all other source/config bytes agree.
        import prepare

        filename, before, after = prepare.MUTATIONS[key]
        for stem, source in (("old", baseline), ("new", candidate)):
            mutant = HERE / f"inputs-{stem}-{key}"
            if not mutant.exists():
                assert key == "omit-eager-conflict" and stem == "old"
                continue
            for path in source.glob("*"):
                if path.suffix == ".py" or path.name in {
                    "uv.lock",
                    "pyproject.toml",
                    "Taskfile.yml",
                }:
                    expected = path.read_text()
                    if path.name == filename:
                        assert expected.count(before) == 1
                        expected = expected.replace(before, after)
                    assert (mutant / path.name).read_text() == expected, mutant / path.name
    for path in candidate.glob("*.py"):
        assert path.read_bytes() == (HERE.parents[1] / path.name).read_bytes(), path
    assert (candidate / "Taskfile.yml").read_bytes() == (REPO / "Taskfile.yml").read_bytes()
    preserved = json.loads((HERE / "prior-after.json").read_text())
    assert preserved["ok"] and not preserved["changed"]
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump(
            {"ok": True, "results": results, "prior_paths": preserved["count"]}, stream, indent=2
        )
        stream.write("\n")
    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
