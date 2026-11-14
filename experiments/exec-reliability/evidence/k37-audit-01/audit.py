"""Independent later PID observations and scratch cleanup for k37 conditions."""

import hashlib
import json
import os
import re
import subprocess
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

ROOT = Path(__file__).resolve().parent
CONDITIONS = [ROOT.parent / name for name in ("k37-previous-01", "k37-final-01")]
FROZEN_INPUTS = ROOT.parent / "k37-inputs-01"
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
    *sorted(
        path
        for path in FROZEN_INPUTS.rglob("*")
        if path.is_file()
        and not {"__pycache__", ".pytest_cache", ".ruff_cache", ".venv"}.intersection(path.parts)
    ),
    *sorted(path for directory in CONDITIONS for path in directory.rglob("*") if path.is_file()),
]


def hashes():
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in INPUTS}


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
    manifest = json.loads((FROZEN_INPUTS / "manifest.json").read_text())
    assert manifest["subjects_before"] == manifest["subjects_after"]
    for name, digest in manifest["subjects_before"].items():
        assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
    for name, digest in manifest["snapshot_sha256"].items():
        assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
    changed = []
    for previous in (FROZEN_INPUTS / "previous").rglob("*"):
        if (
            previous.is_file()
            and previous.name
            in {
                "Taskfile.yml",
                "controller.py",
                "inspection.py",
                "peer.py",
                "preflight.py",
                "self_check.py",
                "test_controller.py",
                "test_inspection.py",
                "pyproject.toml",
                "uv.lock",
            }
            and not {".venv", "__pycache__", ".pytest_cache", ".ruff_cache"}.intersection(
                previous.parts
            )
        ):
            relative = previous.relative_to(FROZEN_INPUTS / "previous")
            if previous.read_bytes() != (PROJECT / relative).read_bytes():
                changed.append(relative.as_posix())
    assert changed == ["experiments/exec-reliability/controller.py"]
    outcomes = {}
    for condition in CONDITIONS:
        cases = ET.parse(condition / "pytest.xml").findall(".//testcase")
        assert len(cases) == 102
        assert all(case.find("error") is None and case.find("skipped") is None for case in cases)
        outcomes[condition.name] = {
            case.attrib["name"]: "failed" if case.find("failure") is not None else "passed"
            for case in cases
        }
        assert len(outcomes[condition.name]) == len(cases)
    previous = outcomes["k37-previous-01"]
    final = outcomes["k37-final-01"]
    assert previous.keys() == final.keys()
    assert all(outcome == "passed" for outcome in final.values())
    table_controls = [name for name in final if name.startswith("test_group_snapshot_")]
    assert len(table_controls) == 26
    failures = [name for name, outcome in previous.items() if outcome == "failed"]
    assert len(failures) == 16
    assert all(name in table_controls for name in failures)
    expected = [
        "duplicate process ID",
        "0 123 S",
        "-2 123 S",
        "+123 123 Z",
        "[123 123 Z--0-incomplete",
        "124 123--0-incomplete",
        "Z\\n\\n--0-",
        "Z extra",
        "124 123 ?",
        "Zgarbage",
        "\\xff",
        "stdout16--0-output",
        "stderr17-0-output",
        "stdout18--0-row",
        "[helper]",
        "[probe]",
    ]
    assert all(sum(token in name for name in failures) == 1 for token in expected)
    assert sum("accepts_valid_tables" in name for name in table_controls) == 5
    (ROOT / "comparison.json").write_text(json.dumps(outcomes, indent=2) + "\n")
    integrity = []
    for condition in CONDITIONS:
        verdict = json.loads((condition / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        for name, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
        if condition.name == "k37-final-01":
            for name, digest in verdict["subjects_before"].items():
                assert hashlib.sha256(Path(name).read_bytes()).hexdigest() == digest, name
        integrity.append(
            {"condition": str(condition), "artifacts": len(verdict["artifact_sha256"]), "ok": True}
        )
    pids = set()
    excluded = set()
    scratch = set()
    resources = []
    children = []

    def collect(value):
        if isinstance(value, dict):
            for key, item in value.items():
                if key in {"pid", "runner_pid", "child", "pgid"} and isinstance(item, int):
                    if item == 123456789:
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

    retained_argv_scratch = set(scratch)
    for session in {directory.parent for directory in scratch}:
        assert not session.is_symlink()
        if not session.exists():
            continue
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
            } or re.fullmatch(r"diag-[0-9a-f]{32}-(vm|second)", path.name), path
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
        "integrity": integrity,
        "subjects_before": before,
        "subjects_after": after,
        "processes": process_receipt,
        "oracle_controls": control_receipts,
        "resources": resources,
        "scratch_sources": {
            "retained_argv": [str(path) for path in sorted(retained_argv_scratch)],
            "session_enumeration_only": [
                str(path) for path in sorted(scratch - retained_argv_scratch)
            ],
        },
        "scratch": directories,
        "limits": "Later PID observations only; no escaped-child proof. ps creation and kill/wait "
        "remain conditionally bounded. No production scheduling or privilege claim.",
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
