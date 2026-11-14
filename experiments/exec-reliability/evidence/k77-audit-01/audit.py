"""Reconcile the k77 regression, deliberate mutants and exact fixture resources."""

import hashlib
import json
import os
import re
import signal
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent
EVIDENCE = ROOT.parent
REPO = EVIDENCE.parents[2]
CONDITIONS = {
    "k77-red-01": (9, 6),
    "k77-invalid-mutant-01": (7, 7),
    "k77-scope-mutant-01": (6, 6),
    "k77-green-01": (350, 1),
    "k77-green-02": (None, None),
    "k77-red-02": (9, 6),
    "k77-invalid-mutant-02": (7, 7),
    "k77-scope-mutant-02": (6, 6),
    "k77-green-03": (350, 0),
}
PRIOR = EVIDENCE / "k77-inputs-01/prior-evidence.json"
INPUTS = {
    Path(__file__),
    PRIOR,
    Path(sys.executable),
    Path("/bin/ps"),
    Path("/usr/bin/true"),
    *(
        path
        for name in (*CONDITIONS, "k77-inputs-01")
        for path in (EVIDENCE / name).rglob("*")
        if path.is_file() and "__pycache__" not in path.parts and ".pytest_cache" not in path.parts
    ),
    *map(Path, json.loads((EVIDENCE / "k77-green-03/verdict.json").read_text())["subjects_after"]),
}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def hashes():
    return {str(path): digest(path) for path in sorted(INPUTS)}


def inspect(pids):
    return subprocess.run(
        ["/bin/ps", "-p", ",".join(map(str, pids)), "-o", "pid=,ppid=,pgid=,stat=,command="],
        capture_output=True,
        timeout=5,
    )


def absent(check):
    return check.returncode == 1 and check.stdout == check.stderr == b""


def verify_prior():
    prior = json.loads(PRIOR.read_text())
    for path, expected in prior.items():
        assert digest(REPO / path) == expected, path
    return len(prior)


def main():
    before = hashes()
    (ROOT / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
    prior_count = verify_prior()
    pids, excluded, scratch = set(), set(), set()
    planned_markers, outcomes = {}, {}
    integrity, late_checks, empty_witnesses = [], [], []

    def collect(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if (
                    key
                    in {
                        "pid",
                        "ppid",
                        "pgid",
                        "runner_pid",
                        "child",
                        "fixture_pid",
                        "controller_pid",
                        "residual_leader_pid",
                    }
                    and type(item) is int
                ):
                    if item <= 0 or item in {123456789, 987654321, 987654322}:
                        excluded.add(item)
                    else:
                        pids.add(item)
                collect(item)
        elif isinstance(value, list):
            for item in value:
                collect(item)
        elif isinstance(value, str):
            for match in re.finditer(
                r"/[^\s\"'<>]+/pytest-of-antony/pytest-\d+/test_[^\s\"'<>/)]+", value
            ):
                scratch.add(Path(match.group()))

    for name, (count, failed) in CONDITIONS.items():
        condition = EVIDENCE / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        if "checks" in verdict:
            codes = [check["returncode"] for check in verdict["checks"]]
            expected_codes = (
                [1, 0, 0, 1]
                if name == "k77-green-01"
                else [0, 0, 0, -15]
                if name == "k77-green-02"
                else [0, 0, 0, 0]
            )
            assert codes == expected_codes
            assert all(check["reaped"] for check in verdict["checks"])
            assert [check["timed_out"] for check in verdict["checks"]] == [
                False,
                False,
                False,
                name == "k77-green-02",
            ]
            artifact_hashes = verdict["artifact_sha256"]
        else:
            codes = [verdict["returncode"]]
            assert codes == [1] and verdict["reaped"] and not verdict["timed_out"]
            artifact_hashes = verdict["artifacts"]
        for path, expected in artifact_hashes.items():
            assert digest(Path(path)) == expected, path
        for path, expected in verdict["subjects_after"].items():
            original = Path(path)
            if original.suffix == ".py" or original.name in {
                "pyproject.toml",
                "uv.lock",
                "Taskfile.yml",
            }:
                if "checks" in verdict:
                    frozen = EVIDENCE / "k77-inputs-01" / name / original.relative_to(REPO)
                    assert digest(frozen) == expected, str(frozen)
                else:
                    assert digest(original) == expected
            if name == "k77-green-03":
                assert digest(original) == expected
        if name == "k77-green-02":
            assert not (condition / "pytest.xml").exists()
            transcript = (condition / "pytest.stdout").read_text()
            integrity.append(
                {
                    "condition": name,
                    "codes": codes,
                    "completed_passes": transcript.count(" PASSED ["),
                    "incomplete": True,
                }
            )
            collect(transcript)
        else:
            cases = ET.parse(condition / "pytest.xml").findall(".//testcase")
            assert len(cases) == count
            assert not any(
                case.find("error") is not None or case.find("skipped") is not None for case in cases
            )
            results = {
                (case.attrib["classname"], case.attrib["name"]): case.find("failure") is None
                for case in cases
            }
            assert len(results) == count
            failures = [key[1] for key, passed in results.items() if not passed]
            assert len(failures) == failed
            if name.startswith("k77-red-"):
                expected_failures = {
                    f"test_late_created_identity_is_cleanup_only[{publication}-{delay}]"
                    for publication in ("valid-overflow", "distinct-overflow")
                    for delay in ("read", "select", "shutdown")
                }
            elif name.startswith("k77-invalid-mutant-"):
                expected_failures = {
                    f"test_late_created_identity_rejects_invalid_fields[{publication}]"
                    for publication in (
                        "token",
                        "boolean-pid",
                        "stale",
                        "future",
                        "deadline",
                        "extra",
                        "partial",
                    )
                }
            elif name.startswith("k77-scope-mutant-"):
                expected_failures = {
                    f"test_late_created_identity_is_cleanup_only[{publication}-select]"
                    for publication in (
                        "valid",
                        "distinct",
                        "malformed",
                        "overflow",
                        "valid-overflow",
                        "distinct-overflow",
                    )
                }
            elif name == "k77-green-01":
                expected_failures = {
                    "test_late_created_identity_is_cleanup_only[valid-overflow-select]"
                }
            else:
                expected_failures = set()
            assert set(failures) == expected_failures
            for case in cases:
                failure = case.find("failure")
                if failure is not None:
                    text = failure.text or ""
                    expected_assertion = (
                        'assert worker["signal_scope"] == "pid"'
                        if name.startswith("k77-scope-mutant-")
                        else "assert delayed"
                        if name == "k77-green-01"
                        else 'assert {item["pid"] for item in identities} == expected'
                    )
                    assert expected_assertion in text

            outcomes[name] = results
            integrity.append(
                {"condition": name, "cases": count, "failures": failures, "codes": codes}
            )
        if (condition / "pytest.xml").exists():
            collect((condition / "pytest.xml").read_text())
        for path in sorted(condition.rglob("*")):
            if not path.is_file():
                continue
            if path.suffix == ".stdout" and path.read_bytes().strip().isdigit():
                pids.add(int(path.read_bytes()))
            if path.suffix != ".json":
                continue
            raw = path.read_bytes()
            if not raw:
                assert re.fullmatch(
                    r"worker-publication-(nested|duplicate)-readiness-\d+.json", path.name
                ), str(path)
                empty_witnesses.append(str(path))
                continue
            value = json.loads(raw)
            collect(value)
            if path.name == "plan.json":
                for resource in value.get("resources", []):
                    argv, identity = resource["acquire"]["argv"], resource["identity"]
                    assert Path(argv[1]).name == "peer.py" and argv[2:4] == ["acquire", identity]
                    assert value["run_id"] in identity
                    planned_markers[str(Path(argv[4]) / identity)] = {
                        "plan": str(path),
                        "identity": identity,
                        "run_id": value["run_id"],
                    }

    green = outcomes["k77-green-03"]
    changed = []
    for name, results in outcomes.items():
        assert results.keys() <= green.keys()
        for key, passed in results.items():
            assert green[key]
            if not passed:
                changed.append([name, *key])
    assert len(changed) == 39
    final = EVIDENCE / "k77-green-03/runs"
    for delay in ("read", "select", "shutdown", "prebuffer"):
        publications = [
            "valid",
            "distinct",
            "malformed",
            "overflow",
            "valid-overflow",
            "distinct-overflow",
        ]
        if delay == "prebuffer":
            publications = ["valid", "distinct"]
        if delay == "shutdown":
            publications += [
                "token",
                "boolean-pid",
                "stale",
                "future",
                "deadline",
                "extra",
                "partial",
            ]
        for publication in publications:
            receipts = [
                p
                for p in final.glob(f"worker-late-created-{delay}-{publication}-*.json")
                if re.fullmatch(
                    re.escape(f"worker-late-created-{delay}-{publication}-") + r"\d+\.json", p.name
                )
            ]
            witnesses = [
                p
                for p in final.glob(f"late-created-{delay}-{publication}-*.json")
                if re.fullmatch(
                    re.escape(f"late-created-{delay}-{publication}-") + r"\d+\.json", p.name
                )
            ]
            assert len(receipts) == len(witnesses) == 1
            receipt = json.loads(receipts[0].read_text())
            witness = json.loads(witnesses[0].read_text())
            worker, identity = receipt["worker"], witness["identity"]
            assert receipt["error"] and receipt["residual_remedy"]
            assert receipt["pid"] is None and not receipt["reaped"]
            assert worker["reaped"] and worker["fork_failure"] is None
            assert worker["receipt_monotonic"] is None and worker["published_times"] is None
            assert worker["shutdown_deadline_monotonic"] == receipt["deadline_monotonic"]
            deadline = worker["query_deadline_monotonic"]
            assert deadline == receipt["deadline_monotonic"] - 0.05 == witness["query_deadline"]
            assert receipt["finished_monotonic"] <= receipt["deadline_monotonic"] + 0.015
            assert worker["dispatch_monotonic"] <= identity["created_monotonic"] < deadline
            assert identity["ppid"] == identity["pgid"] == worker["pid"]
            expected = (
                {identity["pid"]}
                if publication in {"valid", "distinct", "valid-overflow", "distinct-overflow"}
                else set()
            )
            if publication in {"distinct", "distinct-overflow"}:
                expected.add(987654322)
            assert {item["pid"] for item in worker["helper_identities"]} == expected
            assert worker["helper_obligation_retained"] == bool(expected)
            assert worker["unknown_helper_creation"] == (not expected)
            if delay == "select":
                assert not worker["ready"] and worker["signal_scope"] == "pid"
            if delay == "shutdown":
                assert witness["after_term"] and witness["published_monotonic"] >= deadline
            if delay == "prebuffer":
                assert witness["pipe_before_term"] and witness["term_disposition"] == "SIG_DFL"
                assert worker["returncode"] == -signal.SIGTERM
            if publication in {"valid-overflow", "distinct-overflow"}:
                record = json.loads(
                    witnesses[0].with_name(witnesses[0].stem + "-read.json").read_text()
                )
                chunk = bytes.fromhex(record["hex"])
                assert len(chunk) == record["bytes"] <= 4097
                assert b'"created_monotonic"' in chunk and chunk.endswith(b"x" * 100)
                if publication == "distinct-overflow":
                    assert b"987654322" in chunk
            late_checks.append(
                {
                    "receipt": str(receipts[0]),
                    "witness": str(witnesses[0]),
                    "helper_pids": sorted(expected),
                }
            )
    assert len(late_checks) == 27

    probe = subprocess.Popen(["/usr/bin/true"])
    probe.wait(timeout=5)
    controls = {
        "live": inspect([os.getpid()]),
        "absent": inspect([probe.pid]),
        "mixed": inspect([os.getpid(), probe.pid]),
        "invalid": inspect(["invalid"]),
    }
    control_receipts = {
        name: {
            "returncode": check.returncode,
            "stdout": check.stdout.decode(),
            "stderr": check.stderr.decode(),
            "absent": absent(check),
        }
        for name, check in controls.items()
    }
    (ROOT / "controls.json").write_text(json.dumps(control_receipts, indent=2) + "\n")
    assert [absent(check) for check in controls.values()] == [False, True, False, False]
    assert controls["invalid"].stderr
    assert pids and all(type(pid) is int and pid > 0 for pid in pids)
    check = inspect(sorted(pids))
    (ROOT / "processes.stdout").write_bytes(check.stdout)
    (ROOT / "processes.stderr").write_bytes(check.stderr)
    processes = {
        "pids": sorted(pids),
        "excluded_simulated": sorted(excluded),
        "returncode": check.returncode,
        "absent": absent(check),
    }
    (ROOT / "processes.json").write_text(json.dumps(processes, indent=2) + "\n")
    assert absent(check), "Retained exact fixture PID still present; no mock disposal"
    resources = []
    for session in {directory.parent for directory in scratch}:
        if session.is_dir() and not session.is_symlink():
            scratch.update(
                path
                for path in session.iterdir()
                if path.name.startswith("test_") and path.is_dir() and not path.is_symlink()
            )
    for directory in sorted(scratch):
        assert not directory.is_symlink()
        removed = []
        for marker in directory.glob("diag-*") if directory.is_dir() else []:
            assert str(marker) in planned_markers
            assert not marker.is_symlink() and not directory.parent.is_symlink()
            raw = marker.read_bytes()
            assert raw == b"owned"
            removed.append(
                {
                    **planned_markers[str(marker)],
                    "path": str(marker),
                    "bytes_hex": raw.hex(),
                    "sha256": hashlib.sha256(raw).hexdigest(),
                }
            )
            marker.unlink()
            assert not marker.exists()
        resources.append(
            {
                "directory": str(directory),
                "removed": removed,
                "absent_after": not list(directory.glob("diag-*")),
            }
        )
    assert verify_prior() == prior_count
    after = hashes()
    result = {
        "integrity": integrity,
        "late_checks": late_checks,
        "case_comparison": changed,
        "prior_evidence_unchanged": prior_count,
        "subjects_before": before,
        "subjects_after": after,
        "processes": processes,
        "oracle_controls": control_receipts,
        "resources": resources,
        "empty_witnesses": empty_witnesses,
        "limits": "Later exact-PID absence does not establish historical reap or resolve unknown "
        "unpublished helpers. Synthetic identities excluded. Failed captures remain "
        "failed; this slice does not establish the persistent worker/shared scheduler. "
        "Only exact plan-owned mock marker bytes are disposed after clean observation.",
    }
    (ROOT / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    assert before == after
    print(
        f"Audited {len(before)} subjects; {prior_count} prior files; "
        f"{len(late_checks)} late controls; {len(pids)} exact fixture PIDs absent; "
        f"{len(resources)} scratch directories"
    )


if __name__ == "__main__":
    main()
