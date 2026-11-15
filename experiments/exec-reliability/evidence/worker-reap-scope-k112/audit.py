"""Verify retained reap-scope conditions without running or altering their subjects."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
EXPECTED = {
    "green": (109, 0),
    "full": (755, 0),
    "reset-close": (1, 1),
}


def main() -> None:
    results: dict[str, object] = {}
    for name, (total, failures) in EXPECTED.items():
        folder = HERE / name
        verdict = json.loads((folder / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        for key, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256((REPO / key).read_bytes()).hexdigest() == digest, key
        checks = verdict["checks"]
        assert all(item["reaped"] and not item["timed_out"] for item in checks)
        assert verdict["ok"] is (failures == 0)
        cases = list(ET.parse(folder / "pytest.xml").getroot().iter("testcase"))
        failed = [item.attrib["name"] for item in cases if item.find("failure") is not None]
        assert len(cases) == total and len(failed) == failures
        assert all(item.find("error") is None and item.find("skipped") is None for item in cases)
        audits: list[str] = []
        for pattern in ("first-reap-*.json", "shutdown-*.json", "persistent-close-audit-*.json"):
            for path in sorted((folder / "runs").glob(pattern)):
                data = json.loads(path.read_text())
                audit = data.get("audit", data)
                assert audit["reaped"] is True, path
                audits.append(str(path.relative_to(HERE)))
        assert len(list((folder / "runs").glob("first-reap-*.json"))) == (
            0 if name == "reset-close" else 42
        )
        polled = list((folder / "runs").glob("persistent-close-expired-polled-*.json"))
        assert len(polled) == 1
        receipt = json.loads(polled[0].read_text())
        runtime = receipt["runtime"]
        assert receipt["exit_observed_without_reap"]
        assert receipt["polls_before_close"] > 0
        assert receipt["last_poll_observed"] < runtime["owner"]["signals"][0]["monotonic"]
        assert runtime["owner"]["signals"][0]["signal"] == "SIGTERM"
        assert len(runtime["owner"]["signals"]) == 1
        if name == "reset-close":
            assert failed == ["test_persistent_worker_close_has_one_fixed_grace[expired-polled]"]
            assert runtime["owner"]["reaped"]
        else:
            assert not runtime["owner"]["reaped"] and runtime["residual_remedy"]
            assert runtime["owner"]["reap_attempts"] == receipt["polls_before_close"]
        assert runtime["shutdown"]["overrun"]
        results[name] = {
            "tests": total,
            "failures": failed,
            "checks": checks,
            "verified_artifacts": len(verdict["artifact_sha256"]),
            "exact_child_audits": audits,
        }
    green = HERE / "green-inputs"
    subjects = [*sorted(green.glob("*.py")), green / "pyproject.toml", green / "uv.lock"]
    for path in subjects:
        assert path.read_bytes() == (HERE.parents[1] / path.name).read_bytes(), path
    assert (green / "Taskfile.yml").read_bytes() == (REPO / "Taskfile.yml").read_bytes()
    mutations: dict[str, object] = {}
    for name in ("reset-close",):
        differences: list[dict[str, str | int]] = []
        for path in subjects:
            before = path.read_text().splitlines()
            after = (HERE / f"{name}-inputs" / path.name).read_text().splitlines()
            assert len(before) == len(after)
            differences.extend(
                {"file": path.name, "line": index, "before": old, "after": new}
                for index, (old, new) in enumerate(zip(before, after, strict=True), 1)
                if old != new
            )
        assert len(differences) == 1 and differences[0]["file"] == "worker_capture.py"
        mutations[name] = differences
    prior = json.loads((HERE / "prior-after.json").read_text())
    assert prior["ok"] and not prior["changed"]
    assert prior["snapshot"] == json.loads((HERE / "prior-before.json").read_text())
    result = {"conditions": results, "mutations": mutations, "prior_paths": prior["count"]}
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(f"Verified {len(results)} conditions and {prior['count']} unchanged prior paths")


if __name__ == "__main__":
    main()
