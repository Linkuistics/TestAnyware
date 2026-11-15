"""Verify retained digests, per-case mutation failures, and exact-child bootstrap audits."""

import hashlib
import json
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    root = Path(__file__).resolve().parent
    repo = root.parents[3]
    before = digest(Path(__file__))
    summary: dict[str, object] = {}
    checkers: set[int] = set()
    children: set[int] = set()
    verified = 0
    green_cases = {
        case.attrib["name"]
        for case in ET.parse(root / "attempt-2" / "green" / "pytest.xml").findall(".//testcase")
    }
    verdict_paths = sorted(
        [
            *root.glob("red-*/verdict.json"),
            *root.glob("full-*/verdict.json"),
            *(root / "attempt-2").glob("*/verdict.json"),
        ]
    )
    assert len(verdict_paths) == 24
    for verdict_path in verdict_paths:
        folder = verdict_path.parent
        verdict = json.loads(verdict_path.read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"]
        for key, expected in {**verdict["subjects_before"], **verdict["artifact_sha256"]}.items():
            path = Path(key) if Path(key).is_absolute() else repo / key
            candidates = [path]
            if path.parent == root or path == repo / "Taskfile.yml":
                candidates.extend(
                    root / name / path.name
                    for name in ("tooling-attempt-1", "tooling-attempt-2", "tooling-attempt-3")
                )
            if path.name == "diagnostic-process-inspection.md":
                candidates.extend(
                    root / name / "spec.md"
                    for name in ("tooling-attempt-1", "tooling-attempt-2", "tooling-attempt-3")
                )
            assert any(p.is_file() and digest(p) == expected for p in candidates), key
            verified += 1
        checks = verdict["checks"]
        assert all(check["reaped"] and not check["timed_out"] for check in checks)
        checkers.update(check["pid"] for check in checks)
        cases = ET.parse(folder / "pytest.xml").findall(".//testcase")
        failures = [
            c.attrib["name"]
            for c in cases
            if c.find("failure") is not None or c.find("error") is not None
        ]
        if folder.name.startswith("red-"):
            assert failures and checks[0]["returncode"] == 1
            assert set(failures) <= green_cases
        elif folder.name in {"green", "full-final-4"}:
            assert not failures and verdict["ok"]
        summary[str(folder.relative_to(root))] = {
            "cases": len(cases),
            "failures": failures,
            "checks": [{"name": c["name"], "exit": c["returncode"]} for c in checks],
        }
        receipts = sorted((folder / "runs").glob("bootstrap-*.json"))
        assert len(receipts) == (8 if "test_declared_bootstrap_no_child_receipt" in failures else 9)
        for receipt in receipts:
            retained = json.loads(receipt.read_text())
            audit = retained["audit"]
            if audit.get("reaped") is True:
                pid = audit.get("pid", (retained["runtime"].get("owner") or {}).get("pid"))
                assert type(pid) is int
                children.add(pid)
            else:
                assert audit["created"] is False
    recorded_pids: set[int] = set()

    def collect(value: object) -> None:
        if isinstance(value, dict):
            for key, item in value.items():
                if key in {"pid", "worker_pid", "helper_pid"} and type(item) is int and item > 1:
                    recorded_pids.add(item)
                collect(item)
        elif isinstance(value, list):
            for item in value:
                collect(item)

    for verdict_path in verdict_paths:
        for receipt in (verdict_path.parent / "runs").glob("*/result.json"):
            collect(json.loads(receipt.read_text()))
    base = root / "attempt-2" / "inputs"
    mutated: dict[str, list[str]] = {}
    for subject in sorted((root / "attempt-2").glob("inputs-*")):
        changed = [
            p.name for p in base.glob("*.py") if p.read_bytes() != (subject / p.name).read_bytes()
        ]
        assert changed == ["worker_capture.py"]
        mutated[subject.name] = changed
    assert len(mutated) == 17
    assert all((base / p.name).read_bytes() == p.read_bytes() for p in root.parents[1].glob("*.py"))
    # Each faulted test's finally block uses the unfaulted exact-child waitpid.
    # Supplement those reap proofs with a current all-process inventory.
    inventory = subprocess.run(
        ["/bin/ps", "-axo", "pid=,ppid=,pgid=,comm=,args="],
        capture_output=True,
        text=True,
        check=True,
    )
    (root / "process-inventory.stdout").write_text(inventory.stdout)
    (root / "process-inventory.stderr").write_text(inventory.stderr)
    present = {int(line.split()[0]) for line in inventory.stdout.splitlines() if line.strip()}
    residual = sorted((checkers | children | recorded_pids) & present)
    assert not residual, residual
    assert digest(Path(__file__)) == before
    (root / "condition-audit.json").write_text(
        json.dumps(
            {
                "conditions": summary,
                "verified_digests": verified,
                "mutations": mutated,
                "reaped_checker_pids": sorted(checkers),
                "independently_reaped_bootstrap_pids": sorted(children),
                "matching_pids_in_final_inventory": residual,
                "additional_receipt_pids_checked": sorted(recorded_pids),
                "audit_source_before": before,
                "audit_source_after": digest(Path(__file__)),
                "current_runtime_and_tests_match_frozen": True,
                "ok": True,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"{len(summary)} conditions, {verified} digests, {len(mutated)} rejected mutants")


if __name__ == "__main__":
    main()
