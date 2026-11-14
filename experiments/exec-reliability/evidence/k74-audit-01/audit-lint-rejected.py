"""Audit k74 frozen conditions and dispose only exact independently reconciled mock resources."""

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
CONDITIONS = {
    "k74-red-01": (323, 8, [0, 0, 0, 1]),
    "k74-green-01": (323, 0, [0, 0, 0, 0]),
}
INPUTS = [
    *map(
        Path, json.loads((ROOT.parent / "k74-green-01/verdict.json").read_text())["subjects_after"]
    ),
    Path(__file__),
    ROOT / "witness-manifest.json",
    *sorted((ROOT / "scratch-witnesses").iterdir()),
    *[Path(item["original"]) for item in json.loads((ROOT / "witness-manifest.json").read_text())],
    Path(sys.executable),
    Path("/bin/ps"),
    Path("/usr/bin/true"),
    *sorted(path for path in (ROOT.parent / "k74-inputs-red-01").iterdir() if path.is_file()),
    *sorted(
        path for name in CONDITIONS for path in (ROOT.parent / name).rglob("*") if path.is_file()
    ),
    *sorted(path for path in (ROOT.parent / "k74-inputs-green-01").iterdir() if path.is_file()),
    ROOT.parent / "k74-inputs-01/prior-evidence.json",
]


def hashes():
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in INPUTS}


def inspect(pids):
    return subprocess.run(
        ["/bin/ps", "-p", ",".join(map(str, pids)), "-o", "pid=,ppid=,pgid=,stat=,command="],
        capture_output=True,
        timeout=5,
    )


def absent(check):
    return check.returncode == 1 and check.stdout == check.stderr == b""


def main():
    before = hashes()
    (ROOT / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
    pids, excluded, scratch = set(), set(), set()
    integrity, identity_checks = [], []
    empty_witnesses = []
    planned_markers = {}

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

    for name, (count, failure_count, codes) in CONDITIONS.items():
        condition = ROOT.parent / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        assert [check["returncode"] for check in verdict["checks"]] == codes
        assert all(check["reaped"] and not check["timed_out"] for check in verdict["checks"])
        for path, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest, path
        cases = ET.parse(condition / "pytest.xml").findall(".//testcase")
        failures = [case.attrib["name"] for case in cases if case.find("failure") is not None]
        assert len(cases) == count and len(failures) == failure_count
        assert not any(
            case.find("error") is not None or case.find("skipped") is not None for case in cases
        )
        if name == "k74-red-01":
            assert set(failures) == {
                *(
                    "test_review_fork_failure_requires_clean_channel_end[" + mode + "]"
                    for mode in (
                        "control-tail",
                        "short-header",
                        "short-frame",
                        "oversized",
                        "open-tail",
                    )
                ),
                "test_review_fork_failure_preserves_errno_on_publication_error[timeout]",
                "test_review_fork_failure_preserves_errno_on_publication_error[broken-pipe]",
                "test_review_distinct_created_identities_remain_obligations",
            }
        if name in CONDITIONS:
            frozen_dir = "k74-inputs-red-01" if name == "k74-red-01" else "k74-inputs-green-01"
            for path, digest in verdict["subjects_after"].items():
                original = Path(path)
                if name == "k74-green-01":
                    assert hashlib.sha256(original.read_bytes()).hexdigest() == digest
                frozen = ROOT.parent / frozen_dir / original.name
                if original.suffix == ".py" or original.name in {
                    "Taskfile.yml",
                    "pyproject.toml",
                    "uv.lock",
                }:
                    assert hashlib.sha256(frozen.read_bytes()).hexdigest() == digest
        integrity.append({"condition": name, "cases": count, "failures": failures, "codes": codes})
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
            if path.name == "plan.json":
                for resource in value.get("resources", []):
                    argv = resource["acquire"]["argv"]
                    identity = resource["identity"]
                    assert Path(argv[1]).name == "peer.py" and argv[2:4] == ["acquire", identity]
                    assert value["run_id"] in identity
                    planned_markers[str(Path(argv[4]) / identity)] = {
                        "plan": str(path),
                        "identity": identity,
                        "run_id": value["run_id"],
                    }
            collect(value)
            if name != "k74-green-01" or not path.name.startswith(
                (
                    "worker-created-result-loss-",
                    "worker-created-reap-",
                    "worker-created-pre-gate-worker-exit-",
                    "worker-created-conflict-",
                )
            ):
                continue
            worker = value["worker"]
            identity = worker["helper_identity"]
            assert worker["reaped"]
            if identity:
                assert identity["pid"] != worker["pid"]
                assert identity["ppid"] == identity["pgid"] == worker["pid"]
                assert worker["dispatch_monotonic"] <= identity["created_monotonic"]
                assert identity["created_monotonic"] <= identity["deadline_monotonic"]
                assert not worker["unknown_helper_creation"]
            resolved = path.name.startswith("worker-created-reap-")
            assert worker["helper_obligation_retained"] == (bool(identity) and not resolved)
            if resolved:
                assert value["reaped"] and value["pid"] == identity["pid"]
                assert value["created_monotonic"] == identity["created_monotonic"]
            identity_checks.append(
                {
                    "receipt": str(path),
                    "identity": identity,
                    "retained": worker["helper_obligation_retained"],
                }
            )
    assert len(identity_checks) == 14
    fork_checks = []
    final = ROOT.parent / "k74-green-01/runs"
    for path in sorted(final.glob("worker-*fork-failure-*.json")):
        value = json.loads(path.read_text())
        worker = value["worker"]
        assert value["error"] and worker["reaped"]
        assert value["pid"] is None
        known = any(mode in path.name for mode in ("failure-then-created", "created-then-failure"))
        confirmed = path.name.startswith(
            ("worker-fork-failure-fork-error-", "worker-fork-failure-lost-result-")
        )
        assert worker["unknown_helper_creation"] == (not known and not confirmed)
        assert worker["helper_obligation_retained"] == known
        if confirmed:
            assert worker["fork_failure"]["errno"] == value["error_errno"] == errno.EAGAIN
            assert not value["residual_remedy"]
        fork_checks.append(
            {"receipt": str(path), "confirmed_no_child": confirmed, "synthetic_known": known}
        )
    assert len(fork_checks) == 23
    # Compare every named case, not merely the suite totals.
    outcomes = {}
    for name in CONDITIONS:
        outcomes[name] = {
            (case.attrib["classname"], case.attrib["name"]): "failed"
            if case.find("failure") is not None
            else "passed"
            for case in ET.parse(ROOT.parent / name / "pytest.xml").findall(".//testcase")
        }
    assert outcomes["k74-red-01"].keys() == outcomes["k74-green-01"].keys()
    changed = [
        {"class": key[0], "case": key[1], "before": old, "after": outcomes["k74-green-01"][key]}
        for key, old in outcomes["k74-red-01"].items()
        if old != outcomes["k74-green-01"][key]
    ]
    assert len(changed) == 8 and all(item["after"] == "passed" for item in changed)
    (ROOT / "case-comparison.json").write_text(json.dumps(changed, indent=2) + "\n")
    repair_checks = []
    for path in sorted(final.glob("worker-*.json")):
        if not path.name.startswith(
            (
                "worker-fork-failure-channel-",
                "worker-fork-failure-callback-",
                "worker-distinct-created-identities-",
                "worker-parent-late-fork-failure-",
                "worker-fork-failure-worker-",
            )
        ):
            continue
        value = json.loads(path.read_text())
        worker = value["worker"]
        assert value["error"]
        if "-channel-" in path.name:
            assert worker["unknown_helper_creation"] and value["residual_remedy"]
            assert worker["fork_failure"]["errno"] == errno.EAGAIN and worker["reaped"]
        elif "-callback-" in path.name:
            assert value["error_errno"] == errno.EAGAIN and value["pid"] is None
            assert "injected helper fork refusal" in value["error"]
            assert "injected publication" in value["error"]
            assert worker["fork_failure"] is None and worker["reaped"]
        elif "distinct-created-identities" in path.name:
            assert {item["pid"] for item in worker["helper_identities"]} == {987654321, 987654322}
            assert worker["helper_obligation_retained"] and value["residual_remedy"]
        elif "parent-late" in path.name:
            assert worker["fork_failure"] is None
            assert worker["unknown_helper_creation"] and value["residual_remedy"]
        else:
            assert value["error_errno"] == errno.EAGAIN and not worker["unknown_helper_creation"]
            if "unclean-exit" in path.name:
                assert worker["reaped"] and worker["returncode"] == 23
            else:
                assert not worker["reaped"] and value["residual_remedy"]
        repair_checks.append({"receipt": str(path), "worker": worker})
    assert len(repair_checks) == 12
    # Copied scratch witnesses match exact worker PID or original query deadline.
    witnesses = json.loads((ROOT / "witness-manifest.json").read_text())
    assert len(witnesses) == 6
    for item in witnesses:
        assert hashlib.sha256(Path(item["original"]).read_bytes()).hexdigest() == item["sha256"]
        assert hashlib.sha256(Path(item["copy"]).read_bytes()).hexdigest() == item["sha256"]
        witness = json.loads(Path(item["copy"]).read_text())
        receipt = json.loads(Path(item["receipt"]).read_text())
        if "unreaped" in item["receipt"]:
            assert witness["pid"] == receipt["worker"]["pid"] and witness["reaped"] is True
        else:
            assert witness["deadline_monotonic"] == receipt["worker"]["query_deadline_monotonic"]
            assert receipt["worker"]["dispatch_monotonic"] <= witness["failed_monotonic"]
            assert witness["failed_monotonic"] < witness["deadline_monotonic"]
            assert witness["errno"] == errno.EAGAIN

    prior = json.loads((ROOT.parent / "k74-inputs-01/prior-evidence.json").read_text())
    for path, digest in prior.items():
        assert hashlib.sha256(Path(path).read_bytes()).hexdigest() == digest

    # Validate the OS-query oracle before relying on a no-row observation.
    ended = subprocess.Popen(["/usr/bin/true"])
    assert ended.wait(timeout=1) == 0
    controls = {
        "live": inspect([os.getpid()]),
        "missing": inspect([ended.pid]),
        "mixed": inspect([os.getpid(), ended.pid]),
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
    assert absent(check), "Retained exact PID still present; no mock resource disposal"
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
        markers = list(directory.glob("diag-*")) if directory.is_dir() else []
        removed = []
        for marker in markers:
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
    after = hashes()
    result = {
        "integrity": integrity,
        "identity_checks": identity_checks,
        "fork_checks": fork_checks,
        "repair_checks": repair_checks,
        "case_comparison": changed,
        "prior_evidence_unchanged": len(prior),
        "subjects_before": before,
        "subjects_after": after,
        "processes": processes,
        "oracle_controls": control_receipts,
        "resources": resources,
        "empty_witnesses": empty_witnesses,
        "limits": "Later observations of retained exact PIDs only; exact plan-owned mock marker "
        "disposal follows a clean process observation. Unpublished "
        "helpers in failed/interrupted captures remain unknown. Both RED and GREEN source copies are retained. Scratch files are "
        "retained except the exactly matched mock markers; no signaling. "
        "These observations do not repair runtime "
        "receipts or acknowledge helper reaps. No guest, privilege or acceptance claim.",
        "ok": before == after and absent(check) and all(item["absent_after"] for item in resources),
    }
    (ROOT / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(
        json.dumps(
            {
                "ok": result["ok"],
                "pids": len(pids),
                "identity_checks": len(identity_checks),
                "fork_checks": len(fork_checks),
                "prior_evidence": len(prior),
                "scratch": len(scratch),
            }
        )
    )
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
