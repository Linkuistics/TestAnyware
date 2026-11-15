"""Verify frozen comparisons, per-case outcomes and independent overlap receipts."""

import argparse
import hashlib
import itertools
import json
import xml.etree.ElementTree as ET
from pathlib import Path
from typing import Any, cast

HERE = Path(__file__).resolve().parent
CONTROL = "test_production_worker_retirement_overlaps_owned_teardown"


def load(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise ValueError(f"expected object: {path}")
    return cast(dict[str, Any], value)


def require(condition: bool, reason: str) -> None:
    if not condition:
        raise ValueError(reason)


def cases(directory: Path) -> dict[str, str]:
    items: dict[str, str] = {}
    for item in ET.parse(directory / "pytest.xml").iter("testcase"):
        key = item.attrib["classname"] + "::" + item.attrib["name"]
        require(key not in items, f"duplicate case: {key}")
        items[key] = next(
            (tag for tag in ("failure", "error", "skipped") if item.find(tag) is not None),
            "passed",
        )
    require(bool(items), f"no cases: {directory}")
    return items


def condition(name: str, accepted: bool) -> dict[str, str]:
    directory = HERE / name
    verdict = load(directory / "verdict.json")
    require(verdict["ok"] is accepted, f"unexpected verdict: {name}")
    require(verdict["subjects_before"] == verdict["subjects_after"], f"subject drift: {name}")
    require(bool(verdict["artifact_sha256"]), f"missing artifacts: {name}")
    for filename, expected in verdict["artifact_sha256"].items():
        require(
            hashlib.sha256(Path(filename).read_bytes()).hexdigest() == expected,
            f"artifact drift: {filename}",
        )
    for check in verdict["checks"]:
        require(check["reaped"] is True and check["timed_out"] is False, f"checker: {name}")
        expected = 0 if accepted or check["name"] != "pytest" else 1
        require(check["returncode"] == expected, f"checker return: {name}/{check['name']}")
    return cases(directory)


def overlap(directory: Path) -> list[dict[str, str]]:
    observed: dict[tuple[str, str, str], dict[str, str]] = {}
    for path in sorted((directory / "runs").glob("*/production-overlap-audit.json")):
        audit = load(path)
        key = audit["fault"], audit["failed_call"], audit["held_call"]
        require(key not in observed, f"duplicate overlap condition: {key}")
        result = load(path.with_name("result.json"))
        lifecycle, held = audit["lifecycle"], audit["held"]
        owner = lifecycle["owner"]
        require(lifecycle == result["inspection_worker"], f"lifecycle mismatch: {path}")
        require(held in result["calls"], f"held call mismatch: {path}")
        require(result["ok"] is False, f"failure lost: {path}")
        require(result["inspection_failure"]["call"] == key[1], f"failure identity: {path}")
        require(
            [item["name"] for item in result["calls"][-3:]] == ["cleanup", "dispose", "inspect"],
            f"safety calls missing: {path}",
        )
        require(
            result["unreached"]
            == (["workload", "unreached"] if key[1] == "acquire" else ["unreached"]),
            f"work continued after failure: {path}",
        )
        for item in [audit["worker"], *audit["leaders"], *audit["helpers"]]:
            require(
                item["state"] == "absent"
                and item["returncode"] == 1
                and item["stdout"] == ""
                and item["stderr"] == "",
                f"independent absence unresolved: {path}",
            )
        require(audit["worker"]["pid"] == owner["pid"], f"worker identity: {path}")
        require(
            {item["pid"] for item in audit["leaders"]} == {item["pid"] for item in result["calls"]},
            f"leader identity: {path}",
        )
        require(owner["reaped"] is True and owner["returncode"] == -9, f"worker reap: {path}")
        signals = owner["signals"]
        require([item["signal"] for item in signals] == ["SIGTERM", "SIGKILL"], f"signals: {path}")
        cleanup = next(item for item in result["calls"] if item["name"] == "cleanup")
        require(
            cleanup["started_monotonic"]
            < signals[1]["monotonic"]
            < lifecycle["shutdown_finished_monotonic"]
            < held["leader_exit_monotonic"],
            f"shutdown did not overlap teardown: {path}",
        )
        if key[0] == "table" or key[2] == "cleanup":
            require(held["started_monotonic"] < signals[1]["monotonic"], f"held overlap: {path}")
        if key[0] == "transport":
            failed = result["inspection_failure"]["observation"]["helper"]["worker"]
            cutoff = failed["shutdown_deadline_monotonic"]
            require(
                owner["shutdown_deadline_monotonic"] <= cutoff
                and lifecycle["shutdown"]["kill_deadline_monotonic"] <= cutoff
                and all(item["monotonic"] < cutoff for item in signals),
                f"request cutoff renewed: {path}",
            )
            require(failed["helper_obligation_retained"] is True, f"helper obligation lost: {path}")
        observed[key] = {"fault": key[0], "failed_call": key[1], "held_call": key[2]}
    require(
        set(observed)
        == set(
            itertools.product(
                ("table", "transport"), ("acquire", "workload"), ("cleanup", "dispose")
            )
        ),
        f"missing overlap receipts: {directory}",
    )
    return list(observed.values())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    records: dict[str, dict[str, str]] = {}
    for name in ("parent-full-1", "candidate-full-1", "parent-full-2", "candidate-full-2"):
        records[name] = condition(name, name.startswith("candidate"))
    reference = records["candidate-full-1"]
    require(len(reference) == 949, "unexpected full case set")
    for name, items in records.items():
        require(set(items) == set(reference), f"different case identities: {name}")
        for key, outcome in items.items():
            expected = "failure" if name.startswith("parent") and CONTROL in key else "passed"
            require(outcome == expected, f"unexpected case outcome: {name}/{key}")
    for name, fault in (
        ("skip-companion-v2", None),
        ("skip-failure-retirement-v2", "table"),
        ("synchronous-retirement-v2", "transport"),
    ):
        items = condition(name, False)
        require(len(items) == 8, f"mutant case set: {name}")
        for key, outcome in items.items():
            expected = "failure" if fault is None or f"[{fault}-" in key else "passed"
            require(outcome == expected, f"mutant outcome: {name}/{key}")
        mutation = load(HERE / name.replace("-v2", "-v2-inputs") / "preparation.json")["mutation"]
        # Keep the oracle independent of the preparation module and its import path.
        before, after = {
            "synchronous-retirement": (
                "defer_retirement=self.retirements.append,",
                "defer_retirement=None,",
            ),
            "skip-companion": ("self.worker.advance_close()", "None"),
            "skip-failure-retirement": (
                "self.worker.failure = self.worker.failure or "
                'f"local inspection failed after {call}"',
                "pass",
            ),
        }[mutation]
        candidate = (HERE / "full-candidate-inputs/controller.py").read_text()
        actual = (HERE / f"{name}-inputs/controller.py").read_text()
        require(candidate.count(before) == 1, f"mutation site: {name}")
        require(actual == candidate.replace(before, after), f"mutant source mismatch: {name}")
    audits = {name: overlap(HERE / name) for name in ("candidate-full-1", "candidate-full-2")}
    with args.output.open("x") as stream:
        json.dump({"ok": True, "cases": records, "overlap": audits}, stream, indent=2)
        stream.write("\n")
    print(
        "949 identical case identities; expected parent/mutant failures; 16 overlap audits verified"
    )


if __name__ == "__main__":
    main()
