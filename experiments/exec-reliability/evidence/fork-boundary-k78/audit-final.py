"""Reconcile frozen fork-boundary conditions and independently inspect exact fixture PIDs."""

import errno
import hashlib
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
PRIOR = ROOT / "prior-evidence.json"
CONDITIONS = {
    "red-01": (14, [1], "inputs-original"),
    "green-01": (14, [0], "inputs-patched"),
    "full-01": (415, [0, 0, 0, 1], "inputs-patched"),
    "red-02": (14, [1], "inputs-original-final"),
    "full-02": (415, [0, 0, 0, 0], "inputs-final"),
}
MUTATIONS = {
    "python-thread": "python_threads",
    "native-thread": "native_threads",
    "sigchld": "sigchld",
    "native-auto-reap": "sigchld",
    "observation-error": "native_observation_error",
    "preparation-deadline": None,
    "observation-deadline": None,
}


def expected_failures(condition):
    if condition in {"red-01", "red-02"}:
        prefix = "test_review_fork_boundary_rechecks_prepared_state"
        return {
            f"{prefix}[{mutation}-{reason}-{boundary}]": r"assert \[[0-9.]+\] == \[\]"
            for mutation, reason in MUTATIONS.items()
            for boundary in ("helper", "worker")
        }
    if condition == "full-01":
        case = "test_review_helper_late_creation_signals_pinned_child_without_new_allowance"
        reason = "TypeError: '>' not supported between instances of 'NoneType' and 'float'"
        return {case: reason}
    return {}


def verify_failures(condition, failures):
    expected = expected_failures(condition)
    assert failures.keys() == expected.keys(), (condition, failures.keys(), expected.keys())
    for case, reason in failures.items():
        assert re.search(expected[case], reason, re.DOTALL), (condition, case, reason)


INPUTS = {
    Path(__file__),
    PRIOR,
    Path(sys.executable),
    Path("/bin/ps"),
    Path("/usr/bin/true"),
    *(
        p
        for p in ROOT.rglob("*")
        if p.is_file()
        and "audit-final-01" not in p.relative_to(ROOT).parts
        and p.name != "report.md"
        and "__pycache__" not in p.parts
    ),
    *map(Path, json.loads((ROOT / "full-02/verdict.json").read_text())["subjects_after"]),
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
    output = ROOT / "audit-final-01"
    output.mkdir(exist_ok=False)
    before = hashes()
    (output / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
    prior_count = verify_prior()
    pids, excluded, scratch = set(), set(), set()
    planned_markers, outcomes = {}, {}
    integrity, empty_witnesses = [], []

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

    for name, (count, codes, snapshot) in CONDITIONS.items():
        condition = ROOT / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        assert [c["returncode"] for c in verdict["checks"]] == codes
        assert all(c["reaped"] and not c["timed_out"] for c in verdict["checks"])
        assert verdict["ok"] == (name in {"green-01", "full-02"})
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(Path(path)) == expected, path
        for path, expected in verdict["subjects_after"].items():
            original = Path(path)
            if (
                original.parent == ROOT / snapshot
                or original.parent == REPO / "experiments/exec-reliability"
            ):
                frozen = ROOT / snapshot / original.name
                assert digest(frozen) == expected, str(frozen)
            else:
                assert digest(original) == expected, path
            if name == "full-02":
                assert digest(original) == expected, path
        cases = ET.parse(condition / "pytest.xml").findall(".//testcase")
        assert len(cases) == count
        assert not any(c.find("error") is not None or c.find("skipped") is not None for c in cases)
        results = {
            (c.attrib["classname"], c.attrib["name"]): c.find("failure") is None for c in cases
        }
        assert len(results) == count
        failures = {
            c.attrib["name"]: c.find("failure").attrib["message"]
            for c in cases
            if c.find("failure") is not None
        }
        verify_failures(name, failures)
        outcomes[name] = results
        integrity.append(
            {"condition": name, "cases": count, "failure_reasons": failures, "codes": codes}
        )
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
                )
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

    changed = []
    green = outcomes["full-02"]
    for name, results in outcomes.items():
        assert results.keys() <= green.keys()
        for key, passed in results.items():
            assert green[key]
            if not passed:
                changed.append([name, *key])
    assert len(changed) == 29
    # Keep names/counts unchanged and corrupt only a reason: the same validator
    # must reject it. This checks the new instrument without editing evidence.
    original = next(item for item in integrity if item["condition"] == "red-01")
    mutant = dict(original["failure_reasons"])
    mutant[next(iter(mutant))] = "unrelated setup error"
    try:
        verify_failures("red-01", mutant)
    except AssertionError:
        wrong_reason_rejected = True
    else:
        wrong_reason_rejected = False
    assert wrong_reason_rejected
    boundary_controls = []
    for condition in CONDITIONS:
        receipts = sorted((ROOT / condition / "runs").glob("helper-boundary-*.json"))
        assert len(receipts) == 14
        seen = set()
        for path in receipts:
            value = json.loads(path.read_text())
            capture, obs = value["capture"], value["observations"]
            boundary = value["condition"].split("-", 2)[1]
            mutation = obs["mutation"]
            assert (boundary, mutation) not in seen
            seen.add((boundary, mutation))
            red = condition.startswith("red-")
            assert len(obs["fork_attempts"]) == int(red)
            assert len(obs["fork_error_publications"]) == int(red and boundary == "helper")
            assert obs["mask_restored"] and obs["fds_restored"]
            assert capture["pid"] is capture["worker"] is capture["created_monotonic"] is None
            assert capture["error"] and capture["stdout"] == capture["stderr"] == ""
            assert capture["error_errno"] == (errno.EAGAIN if red else None)
            reason = MUTATIONS[mutation]
            if not red and reason:
                assert capture["admission"]["ok"] is False
                assert capture["admission"]["reason"] == reason
            elif not red:
                assert capture["admission"]["ok"] is True
                assert capture["finished_monotonic"] >= capture["deadline_monotonic"]
            boundary_controls.append({"condition": condition, **value})
        assert seen == {(b, m) for b in ("worker", "helper") for m in MUTATIONS}

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
            "returncode": c.returncode,
            "stdout": c.stdout.decode(),
            "stderr": c.stderr.decode(),
            "absent": absent(c),
        }
        for name, c in controls.items()
    }
    (output / "controls.json").write_text(json.dumps(control_receipts, indent=2) + "\n")
    assert [absent(c) for c in controls.values()] == [False, True, False, False]
    assert controls["invalid"].stderr
    assert pids and all(type(pid) is int and pid > 0 for pid in pids)
    check = inspect(sorted(pids))
    (output / "processes.stdout").write_bytes(check.stdout)
    (output / "processes.stderr").write_bytes(check.stderr)
    processes = {
        "pids": sorted(pids),
        "excluded_simulated": sorted(excluded),
        "returncode": check.returncode,
        "absent": absent(check),
    }
    (output / "processes.json").write_text(json.dumps(processes, indent=2) + "\n")
    assert absent(check), "Retained exact fixture PID still present; no mock disposal"
    resources = []
    for session in {directory.parent for directory in scratch}:
        if session.is_dir() and not session.is_symlink():
            scratch.update(
                p
                for p in session.iterdir()
                if p.name.startswith("test_") and p.is_dir() and not p.is_symlink()
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
        "case_comparison": changed,
        "wrong_reason_rejected": wrong_reason_rejected,
        "boundary_controls": boundary_controls,
        "prior_evidence_unchanged": prior_count,
        "subjects_before": before,
        "subjects_after": after,
        "processes": processes,
        "oracle_controls": control_receipts,
        "resources": resources,
        "empty_witnesses": empty_witnesses,
        "limits": "Later exact-PID absence is not historical reap or proof of unpublished "
        "helper absence. Failed captures remain failed. The persistent worker/shared scheduler "
        "remains unimplemented. Only exact plan-owned mock markers disposed after observation. "
        "Only the five declared k78 conditions are included in this new census.",
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    assert before == after
    print(
        f"Audited {len(before)} subjects; {prior_count} prior files; {len(pids)} exact fixture "
        f"PIDs absent; {len(resources)} scratch directories; "
        "five conditions and failure reasons reconciled"
    )


if __name__ == "__main__":
    main()
