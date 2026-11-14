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
    "red-01": (6, 5, [1], "inputs-red"),
    "green-controls-01": (6, 0, [0], "inputs-green"),
    "red-02": (7, 5, [1], "inputs-red-final"),
    "green-full-01": (357, 5, [0, 0, 1, 1], "inputs-green-final"),
    "green-full-02": (357, 0, [0, 0, 0, 0], "inputs-green-repaired"),
}
INPUTS = {
    Path(__file__),
    PRIOR,
    Path(sys.executable),
    Path("/bin/ps"),
    Path("/usr/bin/true"),
    *(
        p
        for p in ROOT.rglob("*")
        if p.is_file() and p.parent.name != "audit-01" and "__pycache__" not in p.parts
    ),
    *map(Path, json.loads((ROOT / "green-full-02/verdict.json").read_text())["subjects_after"]),
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
    output = ROOT / "audit-01"
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

    for name, (count, failures, codes, snapshot) in CONDITIONS.items():
        condition = ROOT / name
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        assert [c["returncode"] for c in verdict["checks"]] == codes
        assert all(c["reaped"] and not c["timed_out"] for c in verdict["checks"])
        assert verdict["ok"] == (failures == 0)
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(Path(path)) == expected, path
        for path, expected in verdict["subjects_after"].items():
            original = Path(path)
            if original.suffix == ".py" or original.name in {
                "pyproject.toml",
                "uv.lock",
                "Taskfile.yml",
            }:
                selected = snapshot
                if name == "red-02" and original.name == "worker_capture.py":
                    selected = "inputs-red"
                if name == "green-controls-01" and original.name == "run_controls.py":
                    selected = "inputs-red"
                frozen = ROOT / selected / original.name
                assert digest(frozen) == expected, str(frozen)
            else:
                assert digest(original) == expected, path
            if name == "green-full-02":
                assert digest(original) == expected, path
        cases = ET.parse(condition / "pytest.xml").findall(".//testcase")
        assert len(cases) == count
        assert not any(c.find("error") is not None or c.find("skipped") is not None for c in cases)
        results = {
            (c.attrib["classname"], c.attrib["name"]): c.find("failure") is None for c in cases
        }
        assert len(results) == count
        failed = {key[1] for key, passed in results.items() if not passed}
        expected = set()
        if name.startswith("red-"):
            expected = {
                f"test_result_framing_rejects_before_control_eof[{mode}]"
                for mode in (
                    "oversized-header",
                    "short-header",
                    "short-body",
                    "trailing",
                    "overflow",
                )
            }
        elif name == "green-full-01":
            expected = {
                f"test_review_fork_failure_requires_clean_channel_end[{mode}]"
                for mode in ("control-tail", "short-header", "short-frame")
            }
            expected |= {
                f"test_review_worker_rejects_hostile_channel[{mode}]"
                for mode in ("oversized-frame", "partial-frame")
            }
        assert failed == expected and len(failed) == failures
        outcomes[name] = results
        integrity.append(
            {"condition": name, "cases": count, "failures": sorted(failed), "codes": codes}
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
    green = outcomes["green-full-02"]
    for name, results in outcomes.items():
        assert results.keys() <= green.keys()
        for key, passed in results.items():
            assert green[key]
            if not passed:
                changed.append([name, *key])
    assert len(changed) == 15
    runs = ROOT / "green-full-02/runs"

    def receipt(prefix):
        matches = list(runs.glob(prefix + "-*.json"))
        assert len(matches) == 1, prefix
        return json.loads(matches[0].read_text())

    frames = []
    for mode, size in {
        "oversized-header": 4,
        "short-header": 2,
        "short-body": 5,
        "trailing": 7,
        "overflow": 4 + 1024 * 1024 + 1,
    }.items():
        item = receipt("worker-incremental-frame-" + mode)
        worker = item["worker"]
        assert "worker frame length" in item["error"] and item["residual_remedy"]
        assert worker["reaped"] and worker["unknown_helper_creation"] and item["pid"] is None
        assert worker["signals"][0]["monotonic"] < item["deadline_monotonic"] - 0.45
        assert worker["shutdown_deadline_monotonic"] == item["deadline_monotonic"]
        assert item["finished_monotonic"] <= item["deadline_monotonic"] + 0.015
        reads = receipt("frame-reads-" + mode)
        result = [items for items in reads.values() if items[0]["requested"] == 4]
        assert len(result) == 1 and sum(x["received"] for x in result[0]) == size
        frames.append({"mode": mode, "result_bytes": size})
    fragmented = receipt("worker-fragmented-frame")
    assert fragmented["error"] is None and fragmented["reaped"] and fragmented["returncode"] == 0
    assert fragmented["worker"]["reaped"] and not fragmented["worker"]["helper_obligation_retained"]
    held = receipt("worker-complete-frame-open-control")
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
        "Exploratory red.xml lacks retained fixture receipts and is outside these five conditions.",
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    assert before == after
    print(
        f"Audited {len(before)} subjects; {prior_count} prior files; {len(pids)} exact fixture "
        f"PIDs absent; {len(resources)} scratch directories; five conditions reconciled"
    )


if __name__ == "__main__":
    main()
