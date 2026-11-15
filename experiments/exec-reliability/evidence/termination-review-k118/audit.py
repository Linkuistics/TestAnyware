"""Check retained k114 identities/counts and k118 regression/cleanup evidence."""

import hashlib
import json
import xml.etree.ElementTree as ET
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[3]
OLD = HERE.with_name("termination-turns-k114")


def failed_names(folder: Path) -> tuple[list[str], list[str]]:
    verdict = json.loads((folder / "verdict.json").read_text())
    assert verdict["subjects_before"] == verdict["subjects_after"], folder
    for key, digest in verdict["artifact_sha256"].items():
        assert hashlib.sha256((REPO / key).read_bytes()).hexdigest() == digest, key
    cases = list(ET.parse(folder / "pytest.xml").getroot().iter("testcase"))
    assert not any(c.find("error") is not None or c.find("skipped") is not None for c in cases)
    return (
        [c.attrib["name"] for c in cases],
        [c.attrib["name"] for c in cases if c.find("failure") is not None],
    )


def child_count(folder: Path, expected: int) -> None:
    audits = sorted((folder / "runs").glob("termination-turn-*.json"))
    assert len(audits) == expected, folder
    for path in audits:
        receipt = json.loads(path.read_text())
        assert receipt["after"]["state"] == "absent", path
        assert receipt["returncode"] is not None, path


def main() -> None:
    deadlines = "test_termination_turn_deadlines_and_wakes"
    starved = "test_termination_turn_starved_first_turn"
    overrun = "test_termination_turn_observer_overrun"
    real = "test_termination_turn_real_child"
    historical = {
        "renew-term": {
            f"{deadlines}[False-False]",
            f"{deadlines}[False-True]",
            f"{deadlines}[True-False]",
            *(
                f"{starved}[{instant}-{pid}]"
                for instant in ("11.0", "11.89", "11.9", "12.1")
                for pid in ("False", "True")
            ),
            f"{real}[False]",
            f"{real}[True]",
        },
        "early-group": {
            f"{deadlines}[True-False]",
            f"{overrun}[0.5-False]",
            f"{overrun}[1.2-False]",
        },
        "late-wake": {f"{deadlines}[False-False]", f"{deadlines}[False-True]"},
    }
    assert [len(historical[key]) for key in ("renew-term", "early-group", "late-wake")] == [
        13,
        3,
        2,
    ]
    for name, expected in historical.items():
        cases, failed = failed_names(OLD / name)
        assert len(cases) == 20 and set(failed) == expected, name
    for name in (
        "red",
        "green-controls",
        "green-full",
        "green-full-2",
        "green-full-3",
        "renew-term",
        "early-group",
        "late-wake",
    ):
        child_count(OLD / name, 2)
    controller_hashes = {
        "baseline": "783c2fbe8d5bbffc7b722c95022509d809a1d78d2706da375495cf968afd7448",
        "candidate": "fbf6184d6d9463096f52133c87599732edc6877cfdddb5d7540fa5eb25f7ee2d",
    }
    for index in range(1, 6):
        pair = {}
        for side in ("baseline", "candidate"):
            folder = OLD / f"{side}-lost-{index}"
            cases, failed = failed_names(folder)
            assert len(cases) == 4 and not failed
            child_count(folder, 0)
            subjects = json.loads((folder / "verdict.json").read_text())["subjects_before"]
            inputs = {Path(key).name: value for key, value in subjects.items()}
            assert inputs["controller.py"] == controller_hashes[side]
            pair[side] = inputs
        for name in ("inspection.py", "worker_capture.py"):
            assert pair["baseline"][name] == pair["candidate"][name]
    results = {}
    for name, total, failures, children in (
        ("old-double", 20, 8, 2),
        ("red-double", 4, 2, 0),
        ("old-repeat", 20, 0, 2),
        ("red-repeat", 4, 2, 0),
        ("red-cutoff-errors", 6, 6, 0),
        ("green-controls", 26, 0, 2),
        ("double-observe", 26, 6, 2),
        ("full", 852, 0, 2),
    ):
        folder = HERE / name
        cases, failed = failed_names(folder)
        assert len(cases) == total and len(failed) == failures, (name, cases, failed)
        if name in {"red-double", "red-repeat", "double-observe"}:
            expected = {f"{deadlines}[False-False]", f"{deadlines}[False-True]"}
            if name == "double-observe":
                expected.update(
                    f"test_termination_turn_live_leader_cutoff_is_not_observer_failure[{cutoff}-{pid}]"
                    for cutoff in ("11.9", "12.1")
                    for pid in ("False", "True")
                )
            assert set(failed) == expected
        child_count(folder, children)
        verdict = json.loads((folder / "verdict.json").read_text())
        assert all(c["reaped"] and not c["timed_out"] for c in verdict["checks"])
        assert verdict["ok"] is (failures == 0)
        if name in {"green-controls", "double-observe", "full"}:
            fixtures = verdict["checks"][0]["fixtures"]
            assert len(fixtures) == 2 and all(f["absent"] for f in fixtures)
        results[name] = {"cases": len(cases), "failures": failed, "children": children}
    first_timeout = json.loads((HERE / "fixture-timeout" / "verdict.json").read_text())
    assert first_timeout["subjects_before"] == first_timeout["subjects_after"]
    assert first_timeout["checks"][0]["timed_out"]
    assert first_timeout["checks"][0]["reaped"]
    assert first_timeout["checks"][0]["fixtures"] == []
    timeout = json.loads((HERE / "fixture-timeout-2" / "verdict.json").read_text())
    assert timeout["subjects_before"] == timeout["subjects_after"]
    check = timeout["checks"][0]
    assert not timeout["ok"] and check["timed_out"] and check["reaped"]
    assert len(check["fixtures"]) == 1
    assert check["fixtures"][0]["signaled"] and check["fixtures"][0]["absent"]
    prior = json.loads((HERE / "prior-after.json").read_text())
    assert prior["ok"] and not prior["changed"]
    with (HERE / "condition-audit.json").open("x") as stream:
        json.dump({"conditions": results, "prior_paths": prior["count"]}, stream, indent=2)
    print(json.dumps(results))


if __name__ == "__main__":
    main()
