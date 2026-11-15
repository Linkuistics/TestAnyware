"""Verify retained k115 outcomes, immutable inputs and named child cleanup receipts."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
EXPECTED = {
    "red-evidence": (29, 29),
    "red-corrected": (29, 1),
    "green-controls": (117, 0),
    "drain-loop": (3, 3),
    "wake-kill": (28, 4),
    "wake-end": (28, 8),
    "self-check": (826, 0),
    "review-controls": (238, 0),
    **{
        f"ordering-{label}-{sample:02}": (1, 0)
        for label in ("parent", "k110")
        for sample in range(1, 11)
    },
}


def main() -> None:
    subjects: dict[Path, str] = {}

    def read(path: Path) -> bytes:
        data = path.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        assert subjects.setdefault(path, digest) == digest, path
        return data

    read(Path(__file__).resolve())
    results: dict[str, object] = {}
    for name, expectation in EXPECTED.items():
        folder = HERE / name
        verdict = json.loads(read(folder / "verdict.json"))
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for key, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256(read(REPO / key)).hexdigest() == digest, key
        assert all(c["reaped"] and not c["timed_out"] for c in verdict["checks"]), name
        cases = list(ET.fromstring(read(folder / "pytest.xml")).iter("testcase"))
        assert all(c.find("error") is None and c.find("skipped") is None for c in cases)
        failures = [c for c in cases if c.find("failure") is not None]
        assert (len(cases), len(failures)) == expectation, name
        assert verdict["ok"] is (not failures), name
        if name == "red-corrected":
            assert failures[0].attrib["name"] == (
                "test_incremental_shutdown_evidence_retains_live_authority"
            )
            assert "Shutdown authority is closed" in failures[0].findtext("failure", "")
        if name == "drain-loop":
            assert all(
                "shutdown step slept inside caller turn" in c.findtext("failure", "")
                for c in failures
            )
        if name in {"wake-kill", "wake-end"}:
            assert all(
                "100.1245" in c.attrib["name"] or "100.3745" in c.attrib["name"] for c in failures
            )
        audits: list[str] = []
        for pattern in ("first-reap-*.json", "shutdown-*.json", "persistent-close-audit-*.json"):
            for path in sorted((folder / "runs").glob(pattern)):
                data = json.loads(read(path))
                assert data.get("audit", data)["reaped"] is True, path
                audits.append(str(path.relative_to(HERE)))
        if name in {"green-controls", "self-check"}:
            assert len(list((folder / "runs").glob("shutdown-incremental-*.json"))) == 42
            assert len(list((folder / "runs").glob("shutdown-blocking-*.json"))) == 42
        results[name] = {
            "tests": len(cases),
            "failures": [c.attrib["name"] for c in failures],
            "exact_child_audits": audits,
            "verified_artifacts": len(verdict["artifact_sha256"]),
        }
    green = HERE / "green-inputs"
    for path in [*sorted(green.glob("*.py")), green / "pyproject.toml", green / "uv.lock"]:
        assert read(path) == read(HERE.parents[1] / path.name), path
    declaration = json.loads(read(HERE / "ordering-declaration.json"))
    assert declaration["samples_per_revision"] == 10
    assert json.loads(read(HERE / "ordering-outcomes.json")) == {
        "parent": [0] * 10,
        "k110": [0] * 10,
    }
    for label, entry in declaration["inputs"].items():
        for filename, digest in entry["sha256"].items():
            data = read(HERE / f"ordering-{label}-inputs" / filename)
            assert hashlib.sha256(data).hexdigest() == digest
    prior = json.loads(read(HERE / "prior-after.json"))
    assert prior["ok"] and not prior["changed"]
    assert prior["snapshot"] == json.loads(read(HERE / "prior-before.json"))
    final = {p: hashlib.sha256(p.read_bytes()).hexdigest() for p in subjects}
    assert subjects == final
    result = {
        "conditions": results,
        "prior_paths": prior["count"],
        "subjects_before": {str(p.relative_to(REPO)): h for p, h in subjects.items()},
        "subjects_after": {str(p.relative_to(REPO)): h for p, h in final.items()},
    }
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump(result, stream, indent=2)
        stream.write("\n")
    print(f"Verified {len(results)} conditions; {prior['count']} prior paths unchanged")


if __name__ == "__main__":
    main()
