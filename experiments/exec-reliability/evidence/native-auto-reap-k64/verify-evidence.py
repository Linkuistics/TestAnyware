"""Audit frozen k64 samples and exact recorded identities outside their workers."""

import hashlib
import json
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def write(path, value):
    with path.open("x") as stream:
        stream.write(json.dumps(value, indent=2) + "\n")


root = Path(__file__).resolve().parent
repo = root.parents[3]
audit = root / "audit01"
audit.mkdir(exist_ok=False)
inputs = [Path(__file__), root / "plan.json", root / "prior-evidence.json", Path(sys.executable)]
inputs += [Path("/bin/ps"), repo / "Taskfile.yml"]
for name in ("red01", "green01", "green02"):
    for directory in (root / name, root / f"sources-{name}"):
        inputs += [path for path in directory.rglob("*") if path.is_file()]
green = json.loads((root / "green02/verdict.json").read_text())
inputs += [Path(path) for path in green["subjects_before"]]
before = {str(path): digest(path) for path in inputs}
write(audit / "inputs-before.json", before)
prior = json.loads((root / "prior-evidence.json").read_text())
now = {
    str(path.relative_to(root.parent)): digest(path)
    for path in sorted(root.parent.rglob("*"))
    if path.is_file() and not path.is_relative_to(root)
}
write(audit / "prior-evidence-after.json", now)
assert prior == now, "earlier evidence membership or bytes changed"
red_failures = {
    f"test_review_native_auto_reap_rejected_before_creation[{boundary}-{flags}]"
    for boundary in ("acquisition", "worker", "helper")
    for flags in (32, 40)
}
red_failures |= {
    f"test_review_worker_transports_real_native_auto_reap_refusal[{flags}]" for flags in (32, 40)
}
red_failures |= {
    "test_worker_failure_consistency[admission-ready-auto-reap-inconsistent worker admission]",
    "test_worker_failure_consistency[native-auto-reap-refusal-None]",
}
inventories = []
results = {}
identities = []
for name, failures in (
    ("red01", red_failures),
    ("green01", {"test_review_admission_rejects_native_sigchld_hidden_from_python[ignore]"}),
    ("green02", set()),
):
    directory = root / name
    verdict = json.loads((directory / "verdict.json").read_text())
    assert verdict["subjects_before"] == verdict["subjects_after"]
    assert len(verdict["subjects_before"]) == 14
    copied = 0
    for path, expected in verdict["subjects_before"].items():
        snapshot = root / f"sources-{name}" / Path(path).name
        if snapshot.exists():
            assert digest(snapshot) == expected, str(snapshot)
            copied += 1
    assert copied == 12
    for path, expected in verdict["artifact_sha256"].items():
        assert digest(Path(path)) == expected, path
    checks = verdict["checks"]
    assert [c["name"] for c in checks] == ["lint", "format", "types", "pytest"]
    assert [c["returncode"] for c in checks] == [0, 0, 0, int(bool(failures))]
    assert all(c["reaped"] and not c["timed_out"] for c in checks)
    assert all(c["elapsed"] < (60 if c["name"] == "pytest" else 30) for c in checks)
    for check in checks:
        identities.append({"run": name, "role": check["name"], "pid": check["pid"]})
    cases = ET.parse(directory / "pytest.xml").findall(".//testcase")
    inventory = {(c.get("classname"), c.get("name")) for c in cases}
    assert len(cases) == len(inventory) == 269
    assert not any(c.find("error") is not None or c.find("skipped") is not None for c in cases)
    failed = {c.get("name") for c in cases if c.find("failure") is not None}
    assert failed == failures, (name, failed)
    inventories.append(inventory)
    direct = sorted((directory / "runs").glob("native-auto-reap-*.json"))
    assert len(direct) == (2 if name == "red01" else 6)
    for path in direct:
        receipt = json.loads(path.read_text())
        assert receipt["pid"] is None and receipt["created_monotonic"] is None
        assert receipt["stdout"] == receipt["stderr"] == ""
        assert receipt["finished_monotonic"] <= receipt["deadline_monotonic"]
        worker = receipt["worker"]
        if worker is not None:
            assert worker["reaped"] and not worker["ownership_lost"]
            assert worker["reap_errno"] is None
            if name != "red01":
                assert worker["returncode"] == 0 and worker["signals"] == []
                assert worker["ready"] and worker["intent"]
                assert not worker["unknown_helper_creation"]
            else:
                assert worker["unknown_helper_creation"]
            identities.append({"run": name, "role": path.name, "pid": worker["pid"]})
        if name != "red01":
            admission = receipt["admission"]
            assert admission["native_sigchld"] == "SA_NOCLDWAIT"
            assert admission["sigchld"] == "SIG_DFL"
            assert admission["ok"] is False and admission["reason"] == "sigchld"
    acquisitions = []
    for path in (directory / "runs").glob("*/result.json"):
        result = json.loads(path.read_text())
        admission = result.get("fork_admission", {})
        if admission.get("native_sigchld") != "SA_NOCLDWAIT":
            continue
        assert result["calls"] == result["resources"] == []
        assert result["unreached"] == ["acquire", "workload"]
        assert result["subjects_before"] == result["subjects_after"]
        assert not list(path.parent.glob("intent-*.json"))
        acquisitions.append(str(path.relative_to(root)))
    assert len(acquisitions) == (0 if name == "red01" else 2)
    results[name] = {
        "tests": len(cases),
        "failed_cases": sorted(failed),
        "source_copies_verified": copied,
        "subject_hashes_stable": True,
        "artifacts_verified": len(verdict["artifact_sha256"]),
        "checks": checks,
        "capture_receipts": [str(path.relative_to(root)) for path in direct],
        "preacquisition_refusals": acquisitions,
    }
assert inventories[0] == inventories[1] == inventories[2]
for path, expected in green["subjects_before"].items():
    assert digest(Path(path)) == expected, path
pids = sorted({item["pid"] for item in identities})
argv = ["/bin/ps", "-p", ",".join(map(str, pids)), "-o", "pid=,ppid=,pgid=,stat=,command="]
observation = subprocess.run(argv, capture_output=True, timeout=2, check=False)
write(
    audit / "identity-audit.json",
    {
        "subjects": identities,
        "argv": argv,
        "returncode": observation.returncode,
        "stdout": observation.stdout.decode(),
        "stderr": observation.stderr.decode(),
        "signal_authority": "none; exact recorded PIDs queried only",
    },
)
assert observation.returncode == 1 and observation.stdout == observation.stderr == b""
after = {str(path): digest(path) for path in inputs}
write(audit / "inputs-after.json", after)
assert before == after, "audit inputs changed"
write(
    audit / "verification.json",
    {
        "ok": True,
        "runs": results,
        "prior_files_unchanged": len(prior),
        "same_individual_case_inventory": True,
        "audit_inputs_stable": len(before),
        "exact_recorded_pids_absent": pids,
        "limits": (
            "RED's safety guard prevents helper creation; interrupted RED workers retain unknown "
            "creation in their receipts. GREEN exercises actual native state and actual worker "
            "transport but creates no helper. PID absence now is an independent check, not helper "
            "reap acknowledgement, signal authority, or aggregate scheduler acceptance."
        ),
    },
)
print(json.dumps({"ok": True, "prior_files": len(prior), "identities": len(identities)}))
