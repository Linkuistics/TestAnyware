"""Item-by-item integrity verification of the delivered k23 condition and audits."""

import hashlib
import json
from pathlib import Path

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


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    inputs = {Path(__file__).resolve()}
    checks = []
    for name in CONDITIONS:
        manifest = EVIDENCE / name / "verdict.json"
        inputs.add(manifest)
        verdict = json.loads(manifest.read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for filename, expected in verdict["artifact_sha256"].items():
            path = Path(filename)
            inputs.add(path)
            assert digest(path) == expected, filename
            checks.append({"condition": name, "artifact": filename, "sha256": expected})
    final = json.loads((EVIDENCE / "k23-final-01/verdict.json").read_text())
    assert final["ok"] is True
    assert [item["name"] for item in final["checks"]] == ["lint", "format", "types", "pytest"]
    assert all(item["returncode"] == 0 and item["reaped"] for item in final["checks"])
    for filename, expected in final["subjects_after"].items():
        path = Path(filename)
        inputs.add(path)
        assert digest(path) == expected, filename
    for name in ("k20-audit-01", "k20-audit-02", "k20-audit-03", "k23-audit-03", "k23-audit-04"):
        manifest = EVIDENCE / name / "result.json"
        inputs.add(manifest)
        verdict = json.loads(manifest.read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for filename, expected in verdict["subjects_after"].items():
            path = Path(filename)
            inputs.add(path)
            assert digest(path) == expected, filename
            checks.append({"audit": name, "input": filename, "sha256": expected})
    audit = json.loads((EVIDENCE / "k23-audit-04/result.json").read_text())
    assert audit["ok"] is True and audit["process_identities_resolved"] is True
    assert not audit["uncertain_processes"]
    before = {str(path): digest(path) for path in sorted(inputs)}
    # Recompute every item rather than comparing only counts.
    after = {str(path): digest(path) for path in sorted(inputs)}
    assert before == after
    (ROOT / "result.json").write_text(
        json.dumps(
            {
                "ok": True,
                "checks": checks,
                "subjects_before": before,
                "subjects_after": after,
                "limits": "Integrity of retained items and current delivered subjects only. "
                "This does not repair open inspection/storage guarantees or establish acceptance.",
            },
            indent=2,
        )
        + "\n"
    )
    print(json.dumps({"ok": True, "item_checks": len(checks), "subjects": len(inputs)}))


if __name__ == "__main__":
    main()
