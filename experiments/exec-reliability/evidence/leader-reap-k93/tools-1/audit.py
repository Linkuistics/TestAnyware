"""Audit exact tracked history and identity-bound first-reap evidence portably."""

import argparse
import errno
import hashlib
import json
import os
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any, cast

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parents[3]
OLD = ROOT.with_name("leader-reap-k88")
PREFIX = "/Users/antony/Development/AbstractingComputers/TestAnyware.gh-issue-14/"


def load(path: Path) -> dict[str, Any]:
    return cast(dict[str, Any], json.loads(path.read_bytes()))


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def resolve(name: str) -> Path:
    """Only the original repository prefix is relocated; host tools stay absolute."""
    if name.startswith(PREFIX):
        return REPO / name.removeprefix(PREFIX)
    path = Path(name)
    return path if path.is_absolute() else REPO / path


def historical_subject(name: str, sample: str) -> Path:
    path = resolve(name)
    if path == REPO / "Taskfile.yml":
        return OLD / ("Taskfile.green-2.yml" if sample == "green-2" else "red-inputs/Taskfile.yml")
    if path == OLD / "run_condition.py" and sample in {"red-1", "green-1"}:
        return OLD / "run_condition_original.py"
    if sample == "full-1" and path.parent == REPO / "experiments/exec-reliability":
        return OLD / "green-inputs" / path.name
    return path


def check_subjects(verdict: dict[str, Any], sample: str | None = None) -> None:
    assert verdict["subjects_before"] == verdict["subjects_after"], "moving subjects"
    for name, sha in verdict["subjects_before"].items():
        path = historical_subject(name, sample) if sample else resolve(name)
        assert digest(path) == sha, f"subject identity: {name} -> {path}"


def inventory_names(inventory: dict[str, Any]) -> set[str]:
    result = subprocess.run(
        [
            "jj",
            "--ignore-working-copy",
            "file",
            "list",
            "-r",
            inventory["revision"],
            "experiments/exec-reliability/evidence",
        ],
        cwd=REPO,
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    return set(result.stdout.splitlines())


def check_inventory(inventory: dict[str, Any], expected: set[str]) -> None:
    assert set(inventory["paths"]) == expected, "tracked inventory equality"
    for name in inventory["paths"]:
        assert not Path(name).is_absolute() and ".." not in Path(name).parts


def inventory_hashes(inventory: dict[str, Any]) -> dict[str, Any]:
    result = {}
    for name in inventory["paths"]:
        path = REPO / name
        link = path.is_symlink()
        data = os.readlink(path).encode() if link else path.read_bytes()
        result[name] = {
            "kind": "symlink" if link else "file",
            "sha256": hashlib.sha256(data).hexdigest(),
        }
    assert result == inventory["paths"], "historical bytes or link targets changed"
    return result


def cases(directory: Path) -> tuple[list[tuple[str, str]], set[str]]:
    rows = ET.parse(directory / "pytest.xml").findall(".//testcase")
    identities = [(row.attrib["classname"], row.attrib["name"]) for row in rows]
    assert len(set(identities)) == len(rows), "duplicate case identity"
    assert all(row.find("skipped") is None for row in rows), "skipped control"
    failures = {
        row.attrib["name"]
        for row in rows
        if row.find("failure") is not None or row.find("error") is not None
    }
    return identities, failures


def check_checks(verdict: dict[str, Any], red: bool, full: bool) -> None:
    assert verdict["ok"] is (not red), "sample verdict"
    checks = verdict["checks"]
    assert [item["name"] for item in checks] == (
        ["lint", "format", "types", "pytest"] if full else ["pytest"]
    ), "checker inventory"
    for item in checks:
        assert item["timed_out"] is False and item["reaped"] is True
        assert item["returncode"] == (1 if red and item["name"] == "pytest" else 0)


def check_artifacts(verdict: dict[str, Any]) -> None:
    for name, sha in verdict["artifact_sha256"].items():
        assert digest(resolve(name)) == sha, f"artifact: {name}"


def check_child(item: dict[str, Any], red: bool, historical: bool) -> None:
    assert item["before"]["state"] == "zombie" and item["after"]["state"] == "absent"
    receipt, pid = item["receipt"], item["pid"]
    assert receipt["authority_closed_monotonic"] == 100.0
    assert receipt["reap_deadline_monotonic"] == item["deadline"]
    assert item["attempts"] == ([] if historical and red else [[pid, os.WNOHANG]])
    ready = item["outcome"] == "ready" and not (historical and red)
    exact = ready and not historical and receipt.get("reaped_monotonic") == item["deadline"]
    assert receipt["within_process_bounds"] is exact
    assert receipt["reaped"] is ready
    if not historical and not red:
        assert receipt["reap_attempts"] == 1
        assert (
            receipt["reap_last_result"]
            == {"ready": "reaped", "withheld": "not_ready", "missing": "error"}[item["outcome"]]
        )
    if ready:
        assert receipt["returncode"] == 0
        assert receipt["reaped_monotonic"] >= item["deadline"]
        assert "residual_leader_pid" not in receipt
    else:
        assert receipt["residual_leader_pid"] == pid
        if item["outcome"] == "missing" and not (historical and red):
            assert receipt["reap_errno"] == errno.ECHILD


def child_matrix(directory: Path, red: bool, historical: bool) -> None:
    children = [load(path) for path in sorted((directory / "runs").glob("leader-reap-*.json"))]
    assert len(children) == (6 if historical else 9), "child inventory"
    assert len({item["pid"] for item in children}) == len(children), "duplicate child"
    for item in children:
        check_child(item, red, historical)
    from collections import Counter

    assert Counter((item["deadline"], item["outcome"]) for item in children) == Counter(
        {
            (deadline, outcome): (2 if deadline == 100.0 and not historical else 1)
            for deadline in (99.0, 100.0)
            for outcome in ("ready", "withheld", "missing")
        }
    ), "boundary matrix"
    if not historical:
        for outcome in ("ready", "withheld", "missing"):
            equality = [
                item
                for item in children
                if item["deadline"] == 100.0 and item["outcome"] == outcome
            ]
            if outcome == "ready":
                assert {item["receipt"]["reaped_monotonic"] for item in equality} == {
                    100.0,
                    100.001,
                }
        callers = [load(path) for path in (directory / "runs").glob("*/caller-reap-audit.json")]
        assert len(callers) == 6, "caller inventory"
        assert {(item["late"], item["simulation"]) for item in callers} == {
            (late, outcome)
            for late in (False, True)
            for outcome in ("ready", "withheld", "missing")
        }
        for item in callers:
            receipt = item["receipt"]
            assert all(child["state"] == "absent" for child in item["after"])
            assert all(len(event) == 2 or event[2] is False for event in item["events"])
            assert receipt["ok"] is (item["simulation"] == "ready" and not item["late"])
            if not red:
                assert receipt["reap_attempts"] == 1


def audit_history() -> dict[str, Any]:
    results = {}
    identities = None
    failures_expected = {
        f"test_leader_reap_first_attempt_at_deadline[{outcome}-{boundary}]"
        for outcome in ("ready", "withheld", "missing")
        for boundary in ("equality", "expired")
    } | {"test_review_nonblocking_reap_retains_failed_owner[expired]"}
    for sample in ("red-1", "green-1", "full-1", "green-2"):
        directory = OLD / sample
        verdict = load(directory / "verdict.json")
        check_subjects(verdict, sample)
        check_artifacts(verdict)
        # The historical full runner used pytest first, then ruff/format/pyright.
        assert [item["name"] for item in verdict["checks"]] == (
            ["pytest", "ruff", "format", "pyright"] if sample == "full-1" else ["pytest"]
        )
        assert verdict["ok"] is (sample != "red-1")
        for item in verdict["checks"]:
            assert item["timed_out"] is False and item["reaped"] is True
            assert item["returncode"] == (1 if sample == "red-1" else 0)
        current, failures = cases(directory)
        if identities is None:
            identities = current
        elif sample == "full-1":
            assert set(identities) <= set(current)
        else:
            assert current == identities, "historical case order/identity"
        assert len(current) == (460 if sample == "full-1" else 16)
        assert failures == (failures_expected if sample == "red-1" else set())
        child_matrix(directory, sample == "red-1", True)
        results[sample] = {"cases": len(current), "failures": sorted(failures)}
    return results


def audit_current() -> dict[str, Any]:
    results = {}
    identities = None
    red_failures = {
        f"test_leader_reap_first_attempt_at_deadline[{outcome}-{boundary}]"
        for outcome in ("ready", "withheld", "missing")
        for boundary in ("equality", "expired", "exact-success")
    } | {
        f"test_leader_reap_caller_closes_target_authority[{boundary}-{outcome}]"
        for boundary in ("exact", "late")
        for outcome in ("ready", "withheld", "missing")
    }
    for sample in ("red-3", "exclusive-2", "green-2", "full-1"):
        directory = ROOT / sample
        verdict = load(directory / "verdict.json")
        red = sample in {"red-3", "exclusive-2"}
        check_subjects(verdict)
        check_artifacts(verdict)
        check_checks(verdict, red, sample == "full-1")
        current, failures = cases(directory)
        if identities is None:
            identities = current
        elif sample == "full-1":
            assert set(identities) <= set(current)
        else:
            assert current == identities, "current case order/identity"
        assert len(current) == (469 if sample == "full-1" else 25)
        expected = (
            red_failures
            if sample == "red-3"
            else {
                "test_leader_reap_first_attempt_at_deadline[ready-exact-success]",
                "test_leader_reap_caller_closes_target_authority[exact-ready]",
            }
            if sample == "exclusive-2"
            else set()
        )
        assert failures == expected, "failure identities"
        # The exclusive mutant deliberately damages success at K; its test failures
        # are the oracle. Do not credit its damaged receipts as conforming samples.
        if sample != "exclusive-2":
            child_matrix(directory, sample == "red-3", False)
        results[sample] = {"cases": len(current), "failures": sorted(failures)}
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    inventory = load(ROOT / "prior-tracked.json")
    check_inventory(inventory, inventory_names(inventory))
    before = inventory_hashes(inventory)
    tool_paths = [Path(__file__), ROOT / "prior-tracked.json", ROOT / "run_condition.py"]
    for sample in ("red-3", "exclusive-2", "green-2", "full-1"):
        verdict_path = ROOT / sample / "verdict.json"
        tool_paths.append(verdict_path)
        verdict = load(verdict_path)
        tool_paths.extend(resolve(name) for name in verdict["artifact_sha256"])
        tool_paths.extend(resolve(name) for name in verdict["subjects_before"])
    tools_before = {str(path): digest(path) for path in tool_paths}
    history, current = audit_history(), audit_current()
    interim = {}
    for sample in ("red-2", "green-1", "exclusive-1"):
        verdict = load(ROOT / sample / "verdict.json")
        check_artifacts(verdict)
        identities, failures = cases(ROOT / sample)
        interim[sample] = {
            "cases": len(identities),
            "failures": sorted(failures),
            "subject_identity_certified": False,
            "reason": "Runner/Taskfile changed after these interim runs without a byte archive; "
            "retained as observations only. Final samples use frozen subjects.",
        }
    assert before == inventory_hashes(inventory)
    assert tools_before == {str(path): digest(path) for path in tool_paths}
    with args.output.open("x") as stream:
        json.dump(
            {
                "ok": True,
                "history": history,
                "current": current,
                "interim": interim,
                "prior_tracked_entries": len(before),
                "subjects": {
                    str(Path(name).relative_to(REPO))
                    if Path(name).is_relative_to(REPO)
                    else name: sha
                    for name, sha in tools_before.items()
                },
                "inventory_before_after_equal": True,
                "limitations": "Recorded local controls; host tool hashes require the same tools. "
                "Withheld/ECHILD are simulated. Aggregate scheduler remains UNMET (k91).",
            },
            stream,
            indent=2,
        )
        stream.write("\n")


if __name__ == "__main__":
    main()
