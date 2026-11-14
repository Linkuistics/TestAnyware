"""Reconcile retained worker conditions, exact-child controls and prior jj evidence."""

import hashlib
import json
import subprocess
import xml.etree.ElementTree as ET
from pathlib import Path

BASE = Path(__file__).resolve().parent
ROOT = BASE.parents[3]
LIVE = ROOT / "experiments/exec-reliability"


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def resolve(path: str) -> Path:
    value = Path(path)
    return value if value.is_absolute() else ROOT / value


def cases(path: Path) -> dict[str, str]:
    result = {}
    for case in ET.parse(path).iter("testcase"):
        key = f"{case.attrib['classname']}::{case.attrib['name']}"
        assert key not in result, key
        result[key] = (
            "failed"
            if case.find("failure") is not None or case.find("error") is not None
            else "skipped"
            if case.find("skipped") is not None
            else "passed"
        )
    return result


def main() -> None:
    output = BASE / "audit-1"
    output.mkdir(exist_ok=False)
    verdict_paths = sorted(BASE.glob("*/verdict.json"))
    verdicts = {path.parent.name: json.loads(path.read_text()) for path in verdict_paths}
    subjects = set(verdict_paths) | {Path(__file__), BASE / "run_condition.py"}
    for verdict in verdicts.values():
        subjects.update(resolve(path) for path in verdict["artifact_sha256"])
    subjects.update(LIVE.glob("*.py"))
    subjects.update(path for path in BASE.glob("inputs-*/*.*") if path.is_file())
    before = {str(path): digest(path) for path in sorted(subjects)}
    declaration = {"subjects_before": before, "subprocess_timeout_seconds": 30}
    (output / "declaration.json").write_text(json.dumps(declaration, indent=2))
    counts = {}
    for name, verdict in verdicts.items():
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for path, expected in verdict["artifact_sha256"].items():
            assert digest(resolve(path)) == expected, path
        assert all(item["reaped"] and not item["timed_out"] for item in verdict["checks"])
        expected_ok = name.startswith("green-") or name == "full-3"
        assert verdict["ok"] is expected_ok, name
        statuses = list(cases(BASE / name / "pytest.xml").values())
        counts[name] = {value: statuses.count(value) for value in sorted(set(statuses))}
    red = cases(BASE / "red-final/pytest.xml")
    green = cases(BASE / "green-final/pytest.xml")
    full = cases(BASE / "full-3/pytest.xml")
    assert len(red) == len(green) == 16 and red.keys() == green.keys()
    assert set(red.values()) == {"failed"} and set(green.values()) == {"passed"}
    assert len(full) == 497 and set(full.values()) == {"passed"}
    assert all(full[key] == value for key, value in green.items())
    for name in ("red-final", "green-final", "full-3"):
        for path, expected in verdicts[name]["subjects_after"].items():
            assert digest(resolve(path)) == expected, (name, path)
    for path in (BASE / "inputs-final").iterdir():
        if path.is_file():
            current = ROOT / "Taskfile.yml" if path.name == "Taskfile.yml" else LIVE / path.name
            assert digest(path) == digest(current), path
    for path in (BASE / "inputs-expired-red").iterdir():
        if path.is_file():
            live = ROOT / "Taskfile.yml" if path.name == "Taskfile.yml" else LIVE / path.name
            key = live.relative_to(ROOT).as_posix()
            assert digest(path) == verdicts["red-expired-1"]["subjects_after"][key], path
    independent = {}
    for name in ("green-final", "full-3"):
        paths = list((BASE / name / "runs").glob("*/persistent-worker-independent-audit.json"))
        assert len(paths) == 1, name
        data = json.loads(paths[0].read_text())
        assert len(data) == 11 and len({item["pid"] for item in data}) == 11
        assert all(item["state"] == "absent" for item in data)
        independent[name] = str(paths[0])
        reconciliations = list((BASE / name / "runs").glob("persistent-expired-independent-*.json"))
        assert len(reconciliations) == 1
        audit = json.loads(reconciliations[0].read_text())
        assert audit["owner"]["reaped"] and audit["owner"]["returncode"] == 0
        assert audit["failure"] and audit["closed"]
    commands = [
        ("baseline", ["jj", "log", "-r", "@-", "--no-graph", "-T", "commit_id"]),
        (
            "prior-paths",
            ["jj", "file", "list", "-r", "@-", "experiments/exec-reliability/evidence"],
        ),
        (
            "evidence-diff",
            [
                "jj",
                "--config",
                "snapshot.max-new-file-size=25000000",
                "diff",
                "--summary",
                "experiments/exec-reliability/evidence",
            ],
        ),
        (
            "old-workers",
            [
                "/bin/ps",
                "-p",
                "21105,21109,21113,21116",
                "-o",
                "pid=,ppid=,pgid=,lstart=,stat=,command=",
            ],
        ),
    ]
    receipts = []
    for name, argv in commands:
        result = subprocess.run(argv, cwd=ROOT, capture_output=True, timeout=30)
        (output / f"{name}.stdout").write_bytes(result.stdout)
        (output / f"{name}.stderr").write_bytes(result.stderr)
        assert result.returncode == (1 if name == "old-workers" else 0), name
        if name == "old-workers":
            assert not result.stdout and not result.stderr
        receipts.append({"name": name, "argv": argv, "returncode": result.returncode})
    prior = (output / "prior-paths.stdout").read_text().splitlines()
    differences = (output / "evidence-diff.stdout").read_text().splitlines()
    prefix = "A experiments/exec-reliability/evidence/persistent-worker-k94/"
    assert differences and all(line.startswith(prefix) for line in differences)
    after = {str(path): digest(path) for path in sorted(subjects)}
    result = {
        "ok": before == after,
        "subjects_before": before,
        "subjects_after": after,
        "case_counts": counts,
        "independent_process_absence": independent,
        "independent_child_evidence": "WNOHANG/ECHILD assertions in all lifecycle tests; expired "
        "request reconciles exact child outside the runtime ownership deadline",
        "prior_tracked_evidence_paths": len(prior),
        "prior_sorted_inventory_sha256": hashlib.sha256(
            "\n".join(sorted(prior)).encode()
        ).hexdigest(),
        "prior_changes": [],
        "new_tracked_evidence_paths_at_audit": len(differences),
        "commands": receipts,
        "historical_source_limit": "Earlier draft samples retain before/after subject hashes and "
        "raw failures. Final RED/GREEN, expired-request RED and full-3 have frozen source bytes.",
    }
    (output / "verdict.json").write_text(json.dumps(result, indent=2) + "\n")
    print(f"audit ok={result['ok']}; {len(prior)} prior tracked evidence paths unchanged")
    raise SystemExit(0 if result["ok"] else 1)


if __name__ == "__main__":
    main()
