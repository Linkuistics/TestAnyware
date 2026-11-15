"""Audit retained termination-turn conditions and exact-child reconciliation."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]


def main() -> None:
    results: dict[str, dict[str, object]] = {}
    for name in (
        "red",
        "green-controls",
        "green-full",
        "green-full-2",
        "green-full-3",
        "renew-term",
        "early-group",
        "late-wake",
        *(f"{side}-lost-{i}" for i in range(1, 6) for side in ("baseline", "candidate")),
    ):
        folder = HERE / name
        verdict = json.loads((folder / "verdict.json").read_text())
        assert verdict["subjects_before"] == verdict["subjects_after"], name
        for key, digest in verdict["artifact_sha256"].items():
            assert hashlib.sha256((REPO / key).read_bytes()).hexdigest() == digest, key
        assert all(item["reaped"] and not item["timed_out"] for item in verdict["checks"]), name
        cases = list(ET.fromstring((folder / "pytest.xml").read_bytes()).iter("testcase"))
        failed = [item for item in cases if item.find("failure") is not None]
        assert not any(
            item.find("error") is not None or item.find("skipped") is not None for item in cases
        )
        if name == "red":
            assert len(cases) == len(failed) == 20
            assert all(
                "has no attribute '_Termination'" in (item.findtext("failure") or "")
                for item in failed
            )
        elif name == "green-full":
            assert len(failed) == 2 and not verdict["ok"]
            assert {item.attrib["name"] for item in failed} == {
                "test_review_late_observation_cannot_extend_reap_deadline[True]",
                "test_review_worker_fork_failure_evidence[lost-result]",
            }
        elif name == "green-full-2":
            assert not failed and not verdict["ok"]
            assert [(i["name"], i["returncode"]) for i in verdict["checks"]] == [
                ("pytest", 0),
                ("ruff", 1),
                ("format", 0),
                ("pyright", 0),
            ]
        elif "-lost-" in name:
            assert len(cases) == 4 and not failed and verdict["ok"], name
        elif name.startswith("green"):
            assert not failed and verdict["ok"], name
        else:
            assert failed and not verdict["ok"], name
            assert all("termination_turn" in item.attrib["name"] for item in failed)
        audits: list[str] = []
        for path in sorted((folder / "runs").glob("termination-turn-*.json")):
            data = json.loads(path.read_bytes())
            assert data["after"]["state"] == "absent", path
            assert data["returncode"] is not None, path
            audits.append(path.name)
        results[name] = {
            "cases": len(cases),
            "failures": [i.attrib["name"] for i in failed],
            "child_audits": audits,
        }
    prior = json.loads((HERE / "prior-after.json").read_bytes())
    assert prior["ok"] and not prior["changed"]
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump({"conditions": results, "prior_paths": prior["count"]}, stream, indent=2)
    print(json.dumps(results))


if __name__ == "__main__":
    main()
