"""Independent exact-resource and immutable-receipt audit for k57 receipt consistency."""

import hashlib
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CONDITIONS = [
    ROOT.parent / name for name in ("k57-order-red-01", "k57-coupling-red-01", "k57-fixed-01")
]
PROJECT = ROOT.parents[3]
INPUTS = [
    Path(__file__),
    Path(sys.executable),
    Path("/bin/ps"),
    Path("/usr/bin/true"),
    PROJECT / "Taskfile.yml",
    *sorted((PROJECT / "experiments/exec-reliability").glob("*.py")),
    PROJECT / "experiments/exec-reliability/pyproject.toml",
    PROJECT / "experiments/exec-reliability/uv.lock",
    *sorted(path for path in (ROOT.parent / "k57-inputs-01").rglob("*") if path.is_file()),
    *sorted(path for directory in CONDITIONS for path in directory.rglob("*") if path.is_file()),
]


PRIOR = json.loads((ROOT.parent / "k57-inputs-01/prior-evidence.json").read_text())


def hashes():
    return {
        str(path): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in [*INPUTS, *map(Path, PRIOR)]
    }


def absent(check):
    return check.returncode == 1 and check.stdout == b"" and check.stderr == b""


def inspect_pids(pids):
    return subprocess.run(
        ["/bin/ps", "-p", ",".join(map(str, pids)), "-o", "pid=,ppid=,pgid=,stat=,command="],
        capture_output=True,
        timeout=5,
    )


def scratch_path(token):
    if not token.startswith("/") or any(character.isspace() for character in token):
        return None
    parts = Path(token).parts
    for index, part in enumerate(parts):
        if part == "pytest-of-antony" and index + 2 < len(parts):
            if re.fullmatch(r"pytest-\d+", parts[index + 1]) and parts[index + 2].startswith(
                "test_"
            ):
                return Path(*parts[: index + 3])
    return None


def main():
    before = hashes()
    (ROOT / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
    assert all(before[name] == digest for name, digest in PRIOR.items())
    order_faults = {"early-created", "future-created", "kill-before-create", "reap-before-kill"}
    coupling_faults = {
        "reaped-without-stamp",
        "stamp-without-reaped",
        "pid-without-created",
        "created-without-pid",
        "kill-errno-without-kill",
        "wrong-cutoff",
    }
    timing_faults = (
        order_faults
        | coupling_faults
        | {
            "predispatch-start",
            "reversed-query",
            "future-finish",
            "null-start",
            "null-finish",
            "null-deadline",
            "null-cutoff",
            "early-cutoff",
            "late-cutoff",
            "kill-without-create",
            "nan-finish",
            "infinite-finish",
        }
    )
    old_faults = {
        "nested-result",
        "nested-readiness",
        "malformed",
        "duplicate-key",
        "bad-token",
        "bad-timestamp",
        "bad-field-type",
        "oversized-frame",
        "oversized-channel",
        "partial-frame",
        "exit-before-intent",
        "duplicate-readiness",
    }
    faults = timing_faults | old_faults
    prefix = "test_review_worker_rejects_hostile_channel"
    accepted_faults = {
        "k57-order-red-01": order_faults | coupling_faults,
        "k57-coupling-red-01": coupling_faults,
        "k57-fixed-01": set(),
    }
    expected_failures = {
        name: {f"{prefix}[{fault}]" for fault in failures}
        for name, failures in accepted_faults.items()
    }
    reasons = {
        **dict.fromkeys(order_faults, "worker helper timestamps out of order"),
        "reaped-without-stamp": "worker reap state/timestamp mismatch",
        "stamp-without-reaped": "worker reap state/timestamp mismatch",
        "pid-without-created": "worker pid/creation timestamp mismatch",
        "created-without-pid": "worker pid/creation timestamp mismatch",
        "kill-errno-without-kill": "worker kill errno without kill timestamp",
        "wrong-cutoff": "worker capture cutoff mismatch",
    }
    integrity = []
    worker_controls = []
    case_sets = {}
    for condition in CONDITIONS:
        cases = ET.parse(condition / "pytest.xml").findall(".//testcase")
        failures = {case.attrib["name"] for case in cases if case.find("failure") is not None}
        assert failures == expected_failures[condition.name]
        assert len(cases) == 187
        assert not any(
            case.find("error") is not None or case.find("skipped") is not None for case in cases
        )
        case_sets[condition.name] = {case.attrib["name"] for case in cases}
        assert len(case_sets[condition.name]) == len(cases)
        codes = [0, 0, 0, 0 if condition.name == "k57-fixed-01" else 1]
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        assert [item["returncode"] for item in verdict["checks"]] == codes
        assert all(item["reaped"] and not item["timed_out"] for item in verdict["checks"])
        for name, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
        label = {
            "k57-order-red-01": "order-red",
            "k57-coupling-red-01": "coupling-red",
            "k57-fixed-01": "final",
        }[condition.name]
        for name, digest in verdict["subjects_before"].items():
            original = Path(name)
            frozen = ROOT.parent / "k57-inputs-01" / label / original.name
            if (
                original.parent == PROJECT / "experiments/exec-reliability"
                or original.name == "Taskfile.yml"
            ):
                assert hashlib.sha256(frozen.read_bytes()).hexdigest() == digest, frozen
            if condition.name == "k57-fixed-01":
                assert hashlib.sha256(original.read_bytes()).hexdigest() == digest, original
        modes = set()
        for receipt_path in sorted((condition / "runs").glob("worker-*.json")):
            receipt = json.loads(receipt_path.read_text())
            mode = re.fullmatch(r"worker-(.*)-[0-9]+.json", receipt_path.name).group(1)
            if mode.startswith("publication-"):
                assert receipt["fault"] in faults
                assert bytes.fromhex(receipt["payload_hex"])
                assert bytes.fromhex(receipt["frame_hex"])
                if receipt["fault"] in order_faults:
                    body = json.loads(bytes.fromhex(receipt["payload_hex"]))
                    (outcome_path,) = (condition / "runs").glob(
                        f"worker-hostile-{receipt['fault']}-protocol-adapter-*.json"
                    )
                    outcome = json.loads(outcome_path.read_text())["worker"]
                    assert (
                        outcome["dispatch_monotonic"]
                        <= body["started_monotonic"]
                        <= body["finished_monotonic"]
                        <= outcome["receipt_monotonic"]
                        <= body["deadline_monotonic"]
                    )
                    assert body["reaped"] == (body["reaped_monotonic"] is not None)
                    assert (body["pid"] is not None) == (body["created_monotonic"] is not None)
                    assert body["capture_deadline_monotonic"] == body["deadline_monotonic"] - min(
                        0.05, (body["deadline_monotonic"] - body["started_monotonic"]) / 4
                    )
                    if receipt["fault"] == "early-created":
                        assert body["created_monotonic"] < body["started_monotonic"]
                    elif receipt["fault"] == "future-created":
                        assert body["finished_monotonic"] < body["created_monotonic"]
                    elif receipt["fault"] == "kill-before-create":
                        assert body["kill_monotonic"] < body["created_monotonic"]
                    else:
                        assert body["reaped_monotonic"] < body["kill_monotonic"]
                continue
            assert mode not in modes, mode
            modes.add(mode)
            if mode == "group-snapshot":
                assert receipt["state"] == "live" and receipt["error"] is None
                assert any(item["pid"] == receipt["fixture_pid"] for item in receipt["members"])
                receipt = receipt["helper"]
            worker = receipt["worker"]
            assert receipt["finished_monotonic"] <= receipt["deadline_monotonic"]
            if mode == "admission-rejected":
                assert worker is None and receipt["admission"]["ok"] is False
            elif mode == "fork-errno-adapter":
                assert worker is None and receipt["error_errno"] == 35
            else:
                assert worker["reaped"] and not worker["ownership_lost"]
                assert worker["shutdown_deadline_monotonic"] == receipt["deadline_monotonic"]
                assert worker["reserve_seconds"] <= 0.05
                hostile = mode.startswith("hostile-")
                accepted_red = mode in {
                    f"hostile-{fault}-protocol-adapter" for fault in accepted_faults[condition.name]
                }
                failed_unknown = mode in {"delayed-creation", "startup-delay"} or (
                    hostile and not accepted_red
                )
                assert worker["unknown_helper_creation"] == failed_unknown
                if failed_unknown:
                    assert receipt["error"] and receipt["residual_remedy"]
                    assert receipt["stdout"] == receipt["stderr"] == ""
                    fault = mode.removeprefix("hostile-").removesuffix("-protocol-adapter")
                    if fault in reasons:
                        assert receipt["error"] == f"worker capture failed: {reasons[fault]}"
                elif mode == "spawn-failure":
                    assert receipt["error_errno"] == 2 and receipt["pid"] is None
                    assert not receipt["reaped"] and receipt["created_monotonic"] is None
                elif mode == "early-exit":
                    assert receipt["returncode"] == 1 and receipt["reaped"]
                    assert receipt["error"] is None
                elif mode == "real-helper-timeout":
                    assert receipt["error"] and receipt["reaped"]
                    assert receipt["kill_monotonic"] is not None
                elif mode == "channel-close-error-adapter":
                    assert receipt["close_errors"] and receipt["error"]
                else:
                    assert receipt["error"] is None
                if not accepted_red:
                    assert worker["dispatch_monotonic"] <= receipt["finished_monotonic"]
                    times = worker["published_times"]
                    if failed_unknown:
                        assert times is None
                    else:
                        assert (
                            worker["dispatch_monotonic"]
                            <= times["started_monotonic"]
                            <= times["finished_monotonic"]
                            <= worker["receipt_monotonic"]
                            <= times["deadline_monotonic"]
                            == worker["query_deadline_monotonic"]
                        )
                        previous = times["started_monotonic"]
                        for key in ("created_monotonic", "kill_monotonic", "reaped_monotonic"):
                            if times[key] is not None:
                                assert previous <= times[key] <= times["finished_monotonic"]
                                previous = times[key]
                        if condition.name == "k57-fixed-01":
                            assert receipt["reaped"] == (times["reaped_monotonic"] is not None)
                            assert (receipt["pid"] is not None) == (
                                times["created_monotonic"] is not None
                            )
                            assert (
                                receipt["kill_errno"] is None or times["kill_monotonic"] is not None
                            )
                            assert times["capture_deadline_monotonic"] == times[
                                "deadline_monotonic"
                            ] - min(
                                0.05, (times["deadline_monotonic"] - times["started_monotonic"]) / 4
                            )
            worker_controls.append({"receipt": str(receipt_path), "mode": mode})
        expected_modes = {
            "admission-rejected",
            "fork-errno-adapter",
            "delayed-creation",
            "startup-delay",
            "reset",
            "exit-hook",
            "channel-close-error-adapter",
            "group-snapshot",
            "early-exit",
            "spawn-failure",
            "real-helper-timeout",
            *(f"closed-stdio-{suffix}" for suffix in ("0", "1", "2", "01", "02", "12", "012")),
            *(f"hostile-{fault}-protocol-adapter" for fault in faults),
        }
        assert modes == expected_modes, (modes - expected_modes, expected_modes - modes)
        assert not list((condition / "runs").glob("escaped-*.json"))
        for fault in timing_faults:
            assert len(list((condition / "runs").glob(f"worker-publication-{fault}-*.json"))) == 1
        integrity.append(
            {
                "condition": str(condition),
                "tests": len(cases),
                "expected_failures": sorted(failures),
                "checker_codes": codes,
                "ok": True,
            }
        )
    assert len({frozenset(cases) for cases in case_sets.values()}) == 1
    pids = set()
    excluded = set()
    scratch = set()
    resources = []
    children = []

    def collect(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if key in {
                    "pid",
                    "runner_pid",
                    "child",
                    "pgid",
                    "residual_leader_pid",
                    "fixture_pid",
                    "controller_pid",
                } and isinstance(item, int):
                    if item in {123456789, 987654321}:
                        excluded.add(item)  # Explicit simulated unreapable-checker PID.
                    else:
                        pids.add(item)
                if key == "argv" and isinstance(item, list):
                    for token in item:
                        if isinstance(token, str) and (directory := scratch_path(token)):
                            scratch.add(directory)
                collect(item)
        elif isinstance(value, list):
            for item in value:
                collect(item)

    for path in INPUTS:
        if path.suffix == ".stdout":
            raw = path.read_bytes().strip()
            if raw.isdigit():
                pids.add(int(raw))
                children.append({"receipt": str(path), "pid": int(raw)})
        if path.suffix != ".json":
            continue
        value = json.loads(path.read_text())
        collect(value)
        if path.name != "plan.json":
            continue
        for resource in value.get("resources", []):
            argv = resource["acquire"]["argv"]
            identity = resource["identity"]
            assert value["run_id"] in identity
            assert Path(argv[1]).name == "peer.py" and argv[2:4] == ["acquire", identity]
            marker = Path(argv[4]) / identity
            assert scratch_path(str(marker)) and not marker.is_symlink()
            present = marker.exists()
            if present:
                assert marker.read_bytes() == b"owned"
            resources.append({"path": str(marker), "present_before": present})

    # The retained failing JUnit names fixture paths even when no argv receipt exists.
    junit_scratch = set()
    for condition in CONDITIONS:
        raw = (condition / "pytest.xml").read_text()
        for token in re.findall(
            r"/[^\s\"'<>]+/pytest-of-antony/pytest-[0-9]+/test_[^\s\"'<>/)]+", raw
        ):
            if directory := scratch_path(token):
                junit_scratch.add(directory)
    scratch.update(junit_scratch)
    retained_argv_scratch = set(scratch)
    # Enumerate only pytest sessions named by retained argv, excluding symlinks.
    for session in {directory.parent for directory in scratch}:
        if session.is_dir() and not session.is_symlink():
            scratch.update(
                path
                for path in session.iterdir()
                if path.name.startswith("test_") and path.is_dir() and not path.is_symlink()
            )
    for directory in sorted(scratch):
        ready = directory / "ready"
        if ready.is_file() and not ready.is_symlink():
            raw = ready.read_bytes().strip()
            if raw.isdigit():  # fork publishes identity here; ready.child says "ready".
                pids.add(int(raw))
                children.append({"receipt": str(ready), "pid": int(raw)})

    ended = subprocess.Popen(("/usr/bin/true",))
    assert ended.wait(timeout=1) == 0
    controls = {
        "live": inspect_pids([os.getpid()]),
        "mixed": inspect_pids([os.getpid(), ended.pid]),
        "invalid": inspect_pids(["invalid-selector"]),
        "missing": inspect_pids([ended.pid]),
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
    assert [absent(check) for check in controls.values()] == [False, False, False, True]
    assert controls["invalid"].stderr
    assert scratch_path("/python /peer.py /tmp/pytest-of-antony/pytest-1/test_x0") is None
    assert scratch_path("/tmp/pytest-of-antony/pytest-1/test_x0/ready") is not None
    check = inspect_pids(sorted(pids))
    (ROOT / "processes.stdout").write_bytes(check.stdout)
    (ROOT / "processes.stderr").write_bytes(check.stderr)
    # No reuse inference or signaling: any returned row remains unresolved.
    process_receipt = {
        "pids": sorted(pids),
        "excluded_simulated": sorted(excluded),
        "known_children": children,
        "returncode": check.returncode,
        "absent": absent(check),
    }
    (ROOT / "processes.json").write_text(json.dumps(process_receipt, indent=2) + "\n")
    assert absent(check), "Retained PID still present or inspection unknown; no scratch cleanup"

    directories = []
    for directory in sorted(scratch):
        assert not directory.is_symlink()
        removed = []
        if not directory.exists():
            directories.append(
                {"path": str(directory), "removed": [], "absent": True, "already_absent": True}
            )
            continue
        preflight = directory / "preflight"
        if preflight.is_dir() and not preflight.is_symlink():
            target = (
                ROOT / "scratch-evidence" / directory.parent.name / directory.name / "preflight"
            )
            target.mkdir(parents=True)
            for source in sorted(preflight.iterdir()):
                assert source.name in {"inventory.json", "paths.json"}
                assert source.is_file() and not source.is_symlink()
                raw = source.read_bytes()
                destination = target / source.name
                destination.write_bytes(raw)
                assert destination.read_bytes() == source.read_bytes() == raw
                source.unlink()
            preflight.rmdir()
        for path in sorted(directory.iterdir()):
            assert path.is_file() and not path.is_symlink()
            assert path.name in {
                "unrelated",
                "input",
                "not-executable",
                "ready",
                "release",
                "ready.child",
                "stdout",
                "stderr",
                "creation.json",
                "child.json",
                "parent-evidence",
                "before-ready",
            } or re.fullmatch(r"diag-[0-9a-f]{32}-(vm(?:-(?:first|second))?|second)", path.name), (
                path
            )
            if path.name in {"creation.json", "child.json", "parent-evidence", "before-ready"}:
                target = ROOT / "scratch-evidence" / directory.parent.name / directory.name
                target.mkdir(parents=True, exist_ok=True)
                (target / path.name).write_bytes(path.read_bytes())
            removed.append(
                {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
            )
            path.unlink()
        directory.rmdir()
        directories.append(
            {"path": str(directory), "removed": removed, "absent": not directory.exists()}
        )
    for resource in resources:
        resource["absent_after"] = not Path(resource["path"]).exists()
    after = hashes()
    result = {
        "conditions": [str(path) for path in CONDITIONS],
        "worker_controls": worker_controls,
        "integrity": integrity,
        "subjects_before": before,
        "subjects_after": after,
        "processes": process_receipt,
        "oracle_controls": control_receipts,
        "resources": resources,
        "scratch_sources": {
            "retained_argv_or_junit": [str(path) for path in sorted(retained_argv_scratch)],
            "session_enumeration_only": [
                str(path) for path in sorted(scratch - retained_argv_scratch)
            ],
        },
        "scratch": directories,
        "limits": "Later exact-PID observations only, no unrecorded escaped-child proof. "
        "The earlier k51 close-error RED has no retained worker identity; it stays unknown. "
        "Audit process creation/waits remain conditional. Interrupted helper creation remains "
        "unknown in original receipts; these later observations never repair decisions. "
        "Per-query isolation is not the persistent worker, gated publication or shared scheduler. "
        "No true UID-denial, setuid-ps, guest or acceptance claim.",
        "ok": before == after
        and absent(check)
        and all(item["absent_after"] for item in resources)
        and all(item["absent"] for item in directories),
    }
    (ROOT / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"ok": result["ok"], "pids": len(pids), "scratch": len(scratch)}))
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
