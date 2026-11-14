"""Audit retained first-reap cases, exact-child cleanup and immutable evidence bytes."""

import argparse
import errno
import hashlib
import json
import os
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import cast


def record(value: object) -> dict[str, object]:
    assert isinstance(value, dict)
    return cast(dict[str, object], value)


def load(path: Path) -> dict[str, object]:
    return record(json.loads(path.read_bytes()))


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    archives = [root / "run_condition_original.py", root / "red-inputs" / "Taskfile.yml"]
    subjects = [Path(__file__), root / "prior-evidence.json", *archives]
    prior = load(root / "prior-evidence.json")
    subjects.extend(Path(name) for name in prior)
    for sample in ("red-1", "green-1", "full-1", "green-2"):
        verdict_path = root / sample / "verdict.json"
        subjects.append(verdict_path)
        frozen = load(verdict_path)
        subjects.extend(Path(name) for name in record(frozen["artifact_sha256"]))
        subjects.extend(Path(name) for name in record(frozen["subjects_before"]))
    before = {str(path): digest(path) for path in subjects}
    counts: dict[str, object] = {}
    expected = {
        f"test_leader_reap_first_attempt_at_deadline[{outcome}-{deadline}]"
        for outcome in ("ready", "withheld", "missing")
        for deadline in ("equality", "expired")
    }
    expected.add("test_review_nonblocking_reap_retains_failed_owner[expired]")
    control_identities = None
    for sample in ("red-1", "green-1", "full-1", "green-2"):
        directory = root / sample
        verdict = load(directory / "verdict.json")
        assert verdict["subjects_before"] == verdict["subjects_after"]
        for name, sha in record(verdict["subjects_before"]).items():
            assert digest(Path(name)) == sha or any(digest(path) == sha for path in archives), name
        assert verdict["ok"] is (sample != "red-1")
        checks = cast(list[object], verdict["checks"])
        for value in checks:
            check = record(value)
            assert check["timed_out"] is False and check["reaped"] is True
            assert check["returncode"] == (1 if sample == "red-1" else 0)
        for name, sha in record(verdict["artifact_sha256"]).items():
            assert digest(Path(name)) == sha, name
        cases = ET.parse(directory / "pytest.xml").findall(".//testcase")
        identities = {(case.attrib["classname"], case.attrib["name"]) for case in cases}
        assert len(identities) == len(cases)
        if sample == "red-1":
            control_identities = identities
        elif sample == "full-1":
            assert control_identities is not None and control_identities <= identities
        else:
            assert identities == control_identities
        failures = {
            case.attrib["name"]
            for case in cases
            if case.find("failure") is not None or case.find("error") is not None
        }
        assert failures == (expected if sample == "red-1" else set())
        assert len(cases) == (460 if sample == "full-1" else 16)
        assert all(case.find("skipped") is None for case in cases)
        audits = sorted((directory / "runs").glob("leader-reap-*.json"))
        assert len(audits) == 6
        combinations: set[tuple[object, object]] = set()
        pids: set[object] = set()
        for path in audits:
            item = load(path)
            combinations.add((item["deadline"], item["outcome"]))
            assert item["pid"] not in pids
            pids.add(item["pid"])
            assert record(item["before"])["state"] == "zombie"
            assert record(item["after"])["state"] == "absent"
            receipt = record(item["receipt"])
            assert receipt["reap_deadline_monotonic"] == item["deadline"]
            assert receipt["authority_closed_monotonic"] == 100.0
            assert receipt["within_process_bounds"] is False
            assert item["attempts"] == ([] if sample == "red-1" else [[item["pid"], os.WNOHANG]])
            ready = sample != "red-1" and item["outcome"] == "ready"
            assert receipt["reaped"] is ready
            if ready:
                assert receipt["returncode"] == 0
                assert cast(float, receipt["reaped_monotonic"]) > cast(float, item["deadline"])
                assert "residual_leader_pid" not in receipt
            else:
                assert receipt["residual_leader_pid"] == item["pid"]
                if sample != "red-1" and item["outcome"] == "missing":
                    assert receipt["reap_errno"] == errno.ECHILD
        assert combinations == {
            (deadline, outcome)
            for deadline in (99.0, 100.0)
            for outcome in ("ready", "withheld", "missing")
        }
        counts[sample] = {"cases": len(cases), "failures": sorted(failures), "child_audits": 6}
    for name, sha in prior.items():
        assert digest(Path(name)) == sha, name
    after = {str(path): digest(path) for path in subjects}
    assert before == after
    result = {
        "ok": True,
        "samples": counts,
        "prior_files_unchanged": len(prior),
        "inventory_source": "prior-evidence.json and each sample verdict artifact_sha256",
        "inventory_files": len(before),
        "subjects_before_sha256": hashlib.sha256(
            json.dumps(before, sort_keys=True).encode()
        ).hexdigest(),
        "subjects_after_sha256": hashlib.sha256(
            json.dumps(after, sort_keys=True).encode()
        ).hexdigest(),
        "limitations": (
            "Recorded observations only; no current process probe or aggregate scheduler claim."
        ),
    }
    with args.output.open("x") as stream:
        stream.write(json.dumps(result, indent=2) + "\n")


if __name__ == "__main__":
    main()
