"""Verify the retained k66 measurements without modifying their subjects."""

import hashlib
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


root = Path(__file__).resolve().parent
repo = root.parents[3]
evidence = root.parent
script_before = digest(Path(__file__))
inputs = [Path(__file__), root / "prior-evidence-before.json", Path(sys.executable)]
inputs.extend(
    repo / path
    for path in (
        "Taskfile.yml",
        "experiments/exec-reliability/pyproject.toml",
        "experiments/exec-reliability/uv.lock",
    )
)
for name in ("red01", "green01", "red01-sources", "green01-sources"):
    inputs.extend(path for path in (root / name).rglob("*") if path.is_file())
inputs_before = {str(path): digest(path) for path in inputs}
(root / "audit-inputs-before.json").write_text(json.dumps(inputs_before, indent=2) + "\n")
prior = json.loads((root / "prior-evidence-before.json").read_text())
after = {
    str(path.relative_to(repo)): digest(path)
    for path in sorted(evidence.rglob("*"))
    if path.is_file() and not path.is_relative_to(root)
}
(root / "prior-evidence-after.json").write_text(json.dumps(after, indent=2) + "\n")
assert prior == after, "earlier evidence membership or bytes changed"
results = {}
case_sets = []
for run, failures in (("red01", 2), ("green01", 0)):
    directory = root / run
    verdict = json.loads((directory / "verdict.json").read_text())
    assert verdict["subjects_before"] == verdict["subjects_after"]
    assert len(verdict["subjects_before"]) == 14
    for path, expected in verdict["artifact_sha256"].items():
        assert digest(Path(path)) == expected, path
    snapshots = root / f"{run}-sources"
    copied = 0
    for path, expected in verdict["subjects_before"].items():
        snapshot = snapshots / Path(path).name
        if snapshot.exists():
            assert digest(snapshot) == expected, str(snapshot)
            copied += 1
    assert copied == 12
    checks = verdict["checks"]
    assert [c["returncode"] for c in checks] == [0, 0, 0, int(bool(failures))]
    assert all(c["reaped"] and not c["timed_out"] for c in checks)
    cases = ET.parse(directory / "pytest.xml").findall(".//testcase")
    identities = [(c.get("classname"), c.get("name")) for c in cases]
    assert len(cases) == len(set(identities)) == 259
    assert not any(c.find("error") is not None or c.find("skipped") is not None for c in cases)
    failed = [c.get("name") for c in cases if c.find("failure") is not None]
    expected_failures = {
        "test_review_worker_group_kill_reaches_live_raw_helper",
        "test_review_raw_helper_resets_before_exec",
    }
    assert set(failed) == (expected_failures if failures else set())
    case_sets.append(set(identities))
    witnesses = list((directory / "runs").glob("worker-group-kill-real-helper-*.json"))
    assert len(witnesses) == 1
    witness = json.loads(witnesses[0].read_text())
    helper = witness["helper"]
    assert witness["worker_reaped"] and witness["worker_returncode"] == -9
    assert helper["ppid"] == witness["worker_pid"]
    if failures:
        assert helper["pid"] == helper["pgid"] == helper["sid"]
        assert witness["helper_exit_event"] is None
        assert witness["fixture_cleanup_exit_pids"] == [helper["pid"]]
    else:
        assert helper["pgid"] == helper["sid"] == witness["worker_pid"]
        event = witness["helper_exit_event"]
        assert event["pid"] == helper["pid"] and event["fflags"] & 0x80000000
    results[run] = {
        "tests": len(cases),
        "failed_cases": failed,
        "source_snapshots_verified": copied,
        "subject_hashes_stable": True,
        "artifact_hashes_verified": len(verdict["artifact_sha256"]),
        "helper_outcome": witness,
    }
assert case_sets[0] == case_sets[1]
inputs_after = {str(path): digest(path) for path in inputs}
(root / "audit-inputs-after.json").write_text(json.dumps(inputs_after, indent=2) + "\n")
assert inputs_before == inputs_after, "audit inputs changed during verification"
assert digest(Path(__file__)) == script_before
result = {
    "ok": True,
    "prior_evidence_files_unchanged": len(prior),
    "same_individual_case_inventory": True,
    "audit_inputs_stable": len(inputs_before),
    "runs": results,
    "audit_script_sha256_before": script_before,
    "audit_script_sha256_after": digest(Path(__file__)),
    "limits": "Helper kernel exit is observed independently; helper reap remains unavailable.",
}
(root / "verification.json").write_text(json.dumps(result, indent=2) + "\n")
print(json.dumps({"ok": True, "prior_files_unchanged": len(prior), "runs": list(results)}))
