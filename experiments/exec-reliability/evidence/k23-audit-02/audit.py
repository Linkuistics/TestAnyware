"""Finite, read-only process audit and exact owned scratch cleanup for k23."""

import hashlib
import json
import os
import re
import subprocess
import sys
from pathlib import Path
from typing import cast

ROOT = Path(__file__).resolve().parent
EVIDENCE = ROOT.parent
CONDITIONS = (
    "k20-red-01",
    "k20-green-01",
    "k20-green-02",
    "k20-green-03",
    "k20-final-01",
    "k20-final-02",
    "k20-final-03",
    "k23-red-01",
    "k23-green-01",
    "k23-red-02",
    "k23-green-02",
    "k23-final-01",
)
INPUTS = [
    Path(__file__),
    Path("/bin/ps"),
    Path(sys.executable),
    EVIDENCE.parents[2] / "Taskfile.yml",
    EVIDENCE.parent / "pyproject.toml",
    EVIDENCE.parent / "uv.lock",
    ROOT / "k23-red-02-fork-failure.json",
    *sorted(path for name in CONDITIONS for path in (EVIDENCE / name).rglob("*") if path.is_file()),
    *sorted(
        path
        for name in ("k20-audit-01", "k20-audit-02", "k20-audit-03")
        for path in (EVIDENCE / name).rglob("*")
        if path.is_file()
    ),
]


def hashes() -> dict[str, str]:
    return {str(path): hashlib.sha256(path.read_bytes()).hexdigest() for path in INPUTS}


def absent(check: subprocess.CompletedProcess[bytes]) -> bool:
    return check.returncode == 1 and check.stdout == b"" and check.stderr == b""


def inspect_pids(pids: list[int | str]) -> subprocess.CompletedProcess[bytes]:
    return subprocess.run(
        [
            "/bin/ps",
            "-p",
            ",".join(map(str, pids)),
            "-o",
            "pid=,ppid=,pgid=,stat=,lstart=,command=",
        ],
        capture_output=True,
        timeout=5,
    )


def scratch_path(token: str) -> Path | None:
    # Only structured absolute argv tokens reach here. Joined ps command text
    # is never a path source. Reject rather than count the historical bad class.
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


def main() -> None:
    for name in CONDITIONS:
        assert (EVIDENCE / name / "verdict.json").is_file(), name
    before = hashes()
    (ROOT / "subjects-before.json").write_text(json.dumps(before, indent=2) + "\n")
    pids: set[int] = set()
    scratch: set[Path] = set()
    # This one failed fixture produced no plan argv. Its exact directory is
    # recorded by pytest's retained traceback in k23-red-02/pytest.stdout.
    scratch.add(
        Path(
            "/private/var/folders/z1/nw_wk3gj3hqchs4_73xrf2dw0000gn/T/pytest-of-antony/"
            "pytest-4016/test_review_fork_publishes_chi0"
        )
    )
    resources: list[dict[str, object]] = []
    known_children: list[dict[str, object]] = []
    excluded_simulated: set[int] = set()

    def collect(value: object) -> None:
        if isinstance(value, dict):
            item_dict = cast(dict[str, object], value)
            for key, item in item_dict.items():
                if key in {"pid", "runner_pid", "child"} and isinstance(item, int):
                    if item == 123456789:
                        excluded_simulated.add(item)
                    else:
                        pids.add(item)
                if key == "argv" and isinstance(item, list):
                    for token in cast(list[object], item):
                        if isinstance(token, str) and (directory := scratch_path(token)):
                            scratch.add(directory)
                collect(item)
        elif isinstance(value, list):
            for item in cast(list[object], value):
                collect(item)

    for path in INPUTS:
        if path.name in {"fork.stdout", "scope.stdout", "stubborn.stdout", "live-exit.stdout"}:
            content = path.read_bytes().strip()
            if content.isdigit():
                child = int(content)
                pids.add(child)
                known_children.append({"receipt": str(path), "pid": child})
        if path.suffix != ".json":
            continue
        value = json.loads(path.read_text())
        # Historical audit result arrays contain the malformed paths. Retain
        # them as inputs, but never use their scratch path assertions as evidence.
        collect(value)
        if not isinstance(value, dict) or path.name != "plan.json":
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
                marker.unlink()
            resources.append(
                {
                    "plan": str(path),
                    "identity": identity,
                    "path": str(marker),
                    "present_before": present,
                    "absent_after": not marker.exists(),
                }
            )

    # Use the SAME argv shape and absence predicate as the actual audit. They
    # must reject a live process, a mixed live/absent set, and a usage failure.
    live = inspect_pids([os.getpid()])
    mixed = inspect_pids([os.getpid(), 123456789])
    usage = inspect_pids(["invalid-selector"])
    missing = inspect_pids([123456789])
    controls = {"live": live, "mixed": mixed, "usage": usage, "missing": missing}
    (ROOT / "controls.json").write_text(
        json.dumps(
            {
                name: {
                    "returncode": check.returncode,
                    "stdout": check.stdout.decode(),
                    "stderr": check.stderr.decode(),
                    "absent": absent(check),
                }
                for name, check in controls.items()
            },
            indent=2,
        )
        + "\n"
    )
    assert not absent(live) and not absent(mixed) and not absent(usage) and absent(missing)
    assert usage.stderr and live.returncode == mixed.returncode == 0
    joined = (
        "/venv/bin/python /repo/peer.py scope-child /private/tmp/pytest-of-antony/pytest-1/test_x0"
    )
    assert scratch_path(joined) is None
    assert scratch_path("/private/tmp/pytest-of-antony/pytest-1/test_x0/release") == Path(
        "/private/tmp/pytest-of-antony/pytest-1/test_x0"
    )

    process_check = inspect_pids(sorted(pids))
    (ROOT / "processes.stdout").write_bytes(process_check.stdout)
    (ROOT / "processes.stderr").write_bytes(process_check.stderr)
    # Failed red R7 discarded child stdout before the old fixture could publish
    # it. Record a later group observation, explicitly keeping child identity unknown.
    old_fork = json.loads((ROOT / "k23-red-02-fork-failure.json").read_text())
    group = subprocess.run(
        ["/bin/ps", "-g", str(old_fork["pid"]), "-o", "pid=,pgid=,stat=,command="],
        capture_output=True,
        timeout=5,
    )
    # These session roots come from structured argv in this finite condition
    # list. Enumerate their fixture directories as well, including tests which
    # fail before a controller plan exists. Do not follow pytest's symlinks.
    for session_root in {directory.parent for directory in scratch}:
        if session_root.is_dir() and not session_root.is_symlink():
            scratch.update(
                path
                for path in session_root.iterdir()
                if path.name.startswith("test_") and path.is_dir() and not path.is_symlink()
            )
    archives = []
    directories = []
    for directory in sorted(scratch):
        assert not directory.is_symlink()
        files = []
        if directory.exists():
            checker = directory / "checker-timeout"
            if checker.is_dir() and not checker.is_symlink():
                target = ROOT / "scratch-checker-evidence" / directory.parent.name / directory.name
                target.mkdir(parents=True)
                for source in sorted(checker.iterdir()):
                    assert source.is_file() and not source.is_symlink()
                    assert source.name in {
                        "declaration.json",
                        "lint.stdout",
                        "lint.stderr",
                        "verdict.json",
                    }
                    digest = hashlib.sha256(source.read_bytes()).hexdigest()
                    destination = target / source.name
                    destination.write_bytes(source.read_bytes())
                    assert digest == hashlib.sha256(destination.read_bytes()).hexdigest()
                    assert digest == hashlib.sha256(source.read_bytes()).hexdigest()
                    archives.append(
                        {
                            "source": str(source),
                            "retained": str(destination),
                            "sha256_before_and_after": digest,
                        }
                    )
                    source.unlink()
                checker.rmdir()
            for path in sorted(directory.iterdir()):
                if (
                    path.is_file()
                    and not path.is_symlink()
                    and (
                        path.name
                        in {
                            "unrelated",
                            "input",
                            "not-executable",
                            "release",
                            "ready",
                            "ready.child",
                            "nested.py",
                            "fork-failure-audit.json",
                        }
                        or re.fullmatch(r"diag-[0-9a-f]{32}-(vm|second)", path.name)
                    )
                ):
                    files.append(
                        {"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest()}
                    )
                    path.unlink()
            if not any(directory.iterdir()):
                directory.rmdir()
        directories.append(
            {"path": str(directory), "removed_files": files, "absent_after": not directory.exists()}
        )
    historical = {}
    for name in ("k20-audit-01", "k20-audit-02", "k20-audit-03"):
        original = json.loads((EVIDENCE / name / "result.json").read_text())
        entries = [item["path"] for item in original["scratch_directories"]]
        historical[name] = {
            "valid_paths": [path for path in entries if scratch_path(path) is not None],
            "rejected_joined_commands": [path for path in entries if scratch_path(path) is None],
        }
    after = hashes()
    result = {
        "conditions": CONDITIONS,
        "subjects_before": before,
        "subjects_after": after,
        "recorded_pids": sorted(pids),
        "excluded_simulated_pids": sorted(excluded_simulated),
        "process_exit": process_check.returncode,
        "processes_absent": absent(process_check),
        "known_children": known_children,
        "resources": resources,
        "scratch_directories": directories,
        "archived_scratch_checker_evidence": archives,
        "historical_path_classification": historical,
        "absence_oracle_controls": {
            name: {
                "returncode": check.returncode,
                "stdout": check.stdout.decode(),
                "stderr": check.stderr.decode(),
                "absent": absent(check),
            }
            for name, check in controls.items()
        },
        "red_fork_group": {
            "pgid": old_fork["pid"],
            "child_pid": None,
            "returncode": group.returncode,
            "stdout": group.stdout.decode(),
            "stderr": group.stderr.decode(),
            "absent": absent(group),
        },
        "limits": "Later observations only; red R7 child PID unavailable. No escaped-child proof. "
        "ps creation/kill-wait can block; independent supervision remains open.",
        "ok": before == after
        and absent(process_check)
        and absent(group)
        and all(item["absent_after"] for item in resources)
        and all(item["absent_after"] for item in directories),
    }
    (ROOT / "result.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"ok": result["ok"], "pids": len(pids), "scratch": len(scratch)}))
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
