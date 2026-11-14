"""Reconcile frozen framing conditions and independently inspect exact fixture PIDs."""

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
    "red-original-01": (82, [1], "inputs-original"),
    "red-minimum-01": (82, [1], "inputs-minimum-red"),
    "green-full-01": (393, [0, 0, 0, 1], "inputs-green"),
    "green-full-02": (393, [0, 0, 0, 0], "inputs-green-repaired"),
    "red-original-02": (90, [1], "inputs-original-final"),
    "green-full-03": (401, [0, 0, 0, 0], "inputs-green-final"),
}
MODES = (
    "oversized-header",
    "short-header",
    "short-body",
    "trailing",
    "overflow",
    "zero-header",
    "one-header",
    "fragmented-oversized-header",
)


def expected_failures(condition):
    expected = {}
    if condition in {"red-original-01", "red-original-02"}:
        for mode in MODES:
            expected[f"test_result_framing_error[{mode}]"] = (
                "worker channel cap exceeded"
                if mode == "overflow"
                else "worker query deadline exhausted"
            )
            expected[f"test_result_framing_read_budget[{mode}]"] = (
                "result read request exceeds framing budget"
            )
            if mode != "overflow":
                expected[f"test_result_framing_term[{mode}]"] = "late first TERM"
                expected[f"test_result_framing_finish[{mode}]"] = "late framing return"
        expected["test_review_worker_rejects_hostile_channel[oversized-frame]"] = (
            r"assert [0-9.]+ is None"
        )
        expected["test_review_worker_rejects_hostile_channel[partial-frame]"] = (
            "invalid worker frame length.*incomplete worker frame length"
        )
        if condition == "red-original-02":
            expected["test_result_framing_byte_count[overflow]"] = (
                "result byte count exceeds framing boundary"
            )
            expected["test_review_worker_rejects_hostile_channel[malformed]"] = (
                "Expecting property name.*invalid worker frame length"
            )
    elif condition == "red-minimum-01":
        for mode in ("zero-header", "one-header"):
            for property_, reason in (
                ("error", "worker query deadline exhausted"),
                ("term", "late first TERM"),
                ("finish", "late framing return"),
            ):
                expected[f"test_result_framing_{property_}[{mode}]"] = reason
    elif condition == "green-full-01":
        expected["test_review_worker_rejects_hostile_channel[malformed]"] = (
            r"None is not None or 'malformed'"
        )
    return expected


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
    *map(Path, json.loads((ROOT / "green-full-03/verdict.json").read_text())["subjects_after"]),
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
        assert verdict["ok"] == (name in {"green-full-02", "green-full-03"})
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
            if name == "green-full-03":
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
    green = outcomes["green-full-03"]
    for name, results in outcomes.items():
        assert results.keys() <= green.keys()
        for key, passed in results.items():
            assert green[key]
            if not passed:
                changed.append([name, *key])
    assert len(changed) == 73
    # Keep names/counts unchanged and corrupt only a reason: the same validator
    # must reject it. This checks the new instrument without editing evidence.
    original = next(item for item in integrity if item["condition"] == "red-original-01")
    mutant = dict(original["failure_reasons"])
    mutant["test_result_framing_read_budget[short-body]"] = "unrelated setup error"
    try:
        verify_failures("red-original-01", mutant)
    except AssertionError:
        wrong_reason_rejected = True
    else:
        wrong_reason_rejected = False
    assert wrong_reason_rejected
    runs = ROOT / "green-full-03/runs"

    def receipts(prefix):
        matches = [
            p
            for p in runs.glob(prefix + "-*.json")
            if re.fullmatch(re.escape(prefix) + r"-\d+\.json", p.name)
        ]
        assert matches, prefix
        return [json.loads(p.read_text()) for p in matches]

    frames = []
    sizes = {
        "oversized-header": 4,
        "short-header": 2,
        "short-body": 5,
        "trailing": 7,
        "overflow": 4 + 1024 * 1024 + 1,
        "zero-header": 4,
        "one-header": 4,
        "fragmented-oversized-header": 4,
    }
    declared = {"short-body": 100, "trailing": 2, "overflow": 1024 * 1024}
    for mode, size in sizes.items():
        items = receipts("worker-incremental-frame-" + mode)
        assert len(items) == 6
        for item in items:
            worker = item["worker"]
            assert "worker frame length" in item["error"] and item["residual_remedy"]
            assert worker["reaped"] and worker["unknown_helper_creation"] and item["pid"] is None
            assert worker["receipt_monotonic"] is None
            assert worker["signals"][0]["signal"] == "SIGTERM"
            assert worker["signals"][0]["monotonic"] < item["deadline_monotonic"] - 0.45
            assert worker["shutdown_deadline_monotonic"] == item["deadline_monotonic"]
            assert worker["query_deadline_monotonic"] == item["deadline_monotonic"] - 0.05
            assert item["finished_monotonic"] < item["deadline_monotonic"] - 0.45
        all_reads = receipts("frame-reads-" + mode)
        assert len(all_reads) == 6
        for reads in all_reads:
            result = [items for items in reads.values() if items[0]["requested"] == 4]
            assert len(result) == 1
            received = 0
            for read in result[0]:
                assert received < 4 or mode in declared
                cap = (
                    4 - received if received < 4 else min(32768, 4 + declared[mode] - received + 1)
                )
                assert read["requested"] <= cap
                received += read["received"]
            assert received == size
        frames.append({"mode": mode, "result_bytes": size, "independent_cases": len(items)})
    for exact_cap in (False, True):
        (fragmented,) = receipts(f"worker-fragmented-frame-{exact_cap}")
        assert (
            fragmented["error"] is None and fragmented["reaped"] and fragmented["returncode"] == 0
        )
        assert (
            fragmented["worker"]["reaped"]
            and not fragmented["worker"]["helper_obligation_retained"]
        )
    (held,) = receipts("worker-complete-frame-open-control")
    assert held["error"] and not held["reaped"] and held["worker"]["reaped"]
    assert (
        held["worker"]["helper_obligation_retained"] and held["worker"]["published_times"] is None
    )

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
        "frames": frames,
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
        "Only the six declared k80 conditions are included in this new census.",
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    assert before == after
    print(
        f"Audited {len(before)} subjects; {prior_count} prior files; {len(pids)} exact fixture "
        f"PIDs absent; {len(resources)} scratch directories; "
        "six conditions and failure reasons reconciled"
    )


if __name__ == "__main__":
    main()
