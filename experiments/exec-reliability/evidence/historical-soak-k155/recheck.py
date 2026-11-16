"""Recheck the retained k152 timing records with the k153 review corrections.

The frozen k152 checker, control and soak stay unchanged. This tool reads only
retained files: each run's timeline.json, its scope receipts and its JUnit
report. It runs no workload.

R1: identity_written is renamed identity_write_started. The fixture takes it
before witness I/O and before pipe delivery. Delivery lies between that stamp
and helper_end_bound, the upper bound after reap.
R2: the parent stamps child_spawned after a declaration write and a rename. It
has no order against child readiness. handler_installed precedes Popen in the
parent, so it orders before child readiness on the one host clock.
R3: KILL is required when the controller must have sent it. Group TERM always
escalates at the TERM deadline (controller.py, "mandatory group KILL"). A PID
TERM whose leader exit was observed before the deadline legitimately has none.
A leader return code of -9 also requires KILL. For other PID records the
evidence cannot decide, so a missing KILL is reported as undetermined.
Each weakening restores one frozen limit and must make the recheck fail.
R4: a JUnit failure or error that names /bin/ps and TimeoutExpired is a
fixture helper timeout, which the k152 helper predicate did not inspect.
"""

import argparse
import copy
import json
import sys
import xml.etree.ElementTree as ElementTree
from collections import Counter
from pathlib import Path
from typing import Any

Record = dict[str, Any]

SCOPE_ORDER = (
    ("spawn", "peer_start"),
    ("peer_start", "handler_installed"),
    ("handler_installed", "child_spawned"),
    ("handler_installed", "child_ready"),
    ("spawn", "term"),
    ("term", "kill"),
    ("term", "leader_exit"),
    ("helper_start", "helper_end"),
)
SCOPE_REQUIRED = (
    "spawn",
    "peer_start",
    "handler_installed",
    "child_spawned",
    "child_ready",
    "term",
    "leader_exit",
    "helper_start",
    "helper_end",
)
PREBUFFER_ORDER = (
    ("dispatch", "helper_start"),
    ("helper_start", "identity_write_started"),
    ("identity_write_started", "helper_end_bound"),
    ("helper_end_bound", "term"),
    ("wait_started", "wait_left"),
)
PREBUFFER_REQUIRED = (
    "dispatch",
    "helper_start",
    "identity_write_started",
    "helper_end_bound",
    "term",
    "wait_started",
    "wait_left",
)


def corrected(record: Record, run: Path) -> Record:
    """Return the retained record with the R1 name and the R3 receipt fields."""
    fixed = copy.deepcopy(record)
    if fixed["kind"] == "prebuffer":
        fixed["identity_write_started"] = fixed.pop("identity_written")
        return fixed
    # Receipts are retained under the run; the absolute path names the
    # checkout that wrote them, so only its last two parts are kept.
    receipt_path = run / "receipts" / Path(*Path(fixed["receipt"]).parts[-2:])
    receipt: Record = json.loads(receipt_path.read_text())
    fixed["signal_scope"] = receipt.get("signal_scope")
    return fixed


def kill_rule(record: Record) -> str:
    """Return "required", "permitted" or "undetermined" for an absent KILL."""
    if record.get("term") is None:
        return "permitted"
    if record.get("signal_scope") == "group" or record.get("returncode") == -9:
        return "required"
    leader, deadline = record.get("leader_exit"), record.get("term_deadline")
    if leader is not None and deadline is not None and leader < deadline:
        return "permitted"
    return "undetermined"


# Each weakening restores one frozen k152 limit. The recheck must fail under it.
WEAKENINGS = ("no-kill-rule", "child-spawned-pair")


def rules(
    record: Record, weakening: str | None = None
) -> tuple[tuple[str, ...], tuple[tuple[str, str], ...]]:
    if record["kind"] == "scope":
        required, order = SCOPE_REQUIRED, SCOPE_ORDER
        if weakening == "child-spawned-pair":
            order = (*order, ("child_spawned", "child_ready"))
    else:
        required, order = PREBUFFER_REQUIRED, PREBUFFER_ORDER
    if record.get("reason") == "exit":
        # The uninterrupted live-exit leader leaves on its own, without TERM.
        required = tuple(name for name in required if name != "term")
    return required, order


def check(record: Record, weakening: str | None = None) -> list[str]:
    """Return every missing field, undetermined KILL and reversed enforced pair."""
    required, order = rules(record, weakening)
    problems = [f"missing {name}" for name in required if record.get(name) is None]
    if record["kind"] == "scope" and record.get("kill") is None and weakening != "no-kill-rule":
        rule = kill_rule(record)
        if rule == "required":
            problems.append("missing kill")
        elif rule == "undetermined":
            problems.append("kill undetermined")
    for before, after in order:
        if record.get(before) is not None and record.get(after) is not None:
            if record[before] > record[after]:
                problems.append(f"{before} after {after}")
    return problems


def red(record: Record, weakening: str | None = None) -> list[Record]:
    """Damage each required field and each enforced pair, and expect detection."""
    required, order = rules(record)
    outcomes: list[Record] = []

    def expect(mutation: str, mutant: Record, problem: str) -> None:
        outcomes.append({"mutation": mutation, "detected": problem in check(mutant, weakening)})

    for name in required:
        expect(f"delete {name}", dict(record, **{name: None}), f"missing {name}")
    if record["kind"] == "scope" and record.get("kill") is not None:
        expect("delete kill", dict(record, kill=None), "missing kill")
    if record["kind"] == "scope" and record.get("signal_scope") == "pid":
        # A lost KILL behind a SIGKILL exit, the k145 historical shape.
        expect("rc -9 without kill", dict(record, kill=None, returncode=-9), "missing kill")
        late = dict(record, kill=None, returncode=None, leader_exit=None)
        expect("pid leader exit unobserved", late, "kill undetermined")
    for before, after in order:
        if record.get(before) is None or record.get(after) is None:
            continue
        mutant = dict(record, **{before: record[after] + 0.001, after: record[before]})
        expect(f"reverse {before}/{after}", mutant, f"{before} after {after}")
    return outcomes


def permitted(record: Record, weakening: str | None = None) -> list[Record]:
    """Apply sequences that the corrected rules allow, and expect no problem."""
    outcomes: list[Record] = []
    installed, spawned = record.get("handler_installed"), record.get("child_spawned")
    if record["kind"] == "scope" and installed is not None and spawned is not None:
        # R2: readiness may precede the parent's post-declaration stamp.
        mutant = dict(record, child_ready=(installed + spawned) / 2)
        outcomes.append(
            {"sequence": "child_ready before child_spawned", "clean": not check(mutant, weakening)}
        )
    return outcomes


def is_fixture_ps_timeout(text: str) -> bool:
    return "TimeoutExpired" in text and "/bin/ps" in text


def junit_failures(path: Path) -> list[Record]:
    found: list[Record] = []
    for case in ElementTree.parse(path).getroot().iter("testcase"):
        for element in list(case):
            if element.tag in {"failure", "error"}:
                text = (element.get("message") or "") + "\n" + (element.text or "")
                found.append(
                    {
                        "nodeid": f"{case.get('classname')}::{case.get('name')}",
                        "tag": element.tag,
                        "fixture_ps_timeout": is_fixture_ps_timeout(text),
                    }
                )
    return found


PREDICATE_RED = (
    "subprocess.TimeoutExpired: Command '['/bin/ps', '-p', '4100', '-o', "
    "'pid=,pgid=,stat=,lstart=,command=']' timed out after 1 seconds"
)


def recheck(run: Path) -> Record:
    checked: list[Record] = json.loads((run / "timeline.json").read_text())
    records = [corrected(entry["record"], run) for entry in checked]
    reds = [outcome for record in records for outcome in red(record)]
    allowed = [outcome for record in records for outcome in permitted(record)]
    junit = run / "pytest.xml"
    failures = junit_failures(junit) if junit.is_file() else []
    return {
        "run": run.name,
        "records": len(records),
        "kill_rules": [
            f"{r['signal_scope']}:{kill_rule(r)}:{'kill' if r.get('kill') else 'no-kill'}"
            for r in records
            if r["kind"] == "scope"
        ],
        "green_problems": [
            {"record": r.get("receipt") or r.get("witness"), "problems": check(r)}
            for r in records
            if check(r)
        ],
        "red_mutations": len(reds),
        "red_undetected": [outcome for outcome in reds if not outcome["detected"]],
        "permitted_sequences": len(allowed),
        "permitted_flagged": [outcome for outcome in allowed if not outcome["clean"]],
        "junit_failures": failures,
        "weakened": {
            weakening: {
                "red_undetected": sum(
                    not outcome["detected"] for r in records for outcome in red(r, weakening)
                ),
                "permitted_flagged": sum(
                    not outcome["clean"] for r in records for outcome in permitted(r, weakening)
                ),
            }
            for weakening in WEAKENINGS
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("evidence", type=Path, help="the k152 evidence directory")
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    runs: dict[str, list[Record]] = {}
    for name in ("control", "soak"):
        directory = args.evidence / name / "runs"
        runs[name] = [recheck(run) for run in sorted(p for p in directory.iterdir() if p.is_dir())]
    every = [result for results in runs.values() for result in results]
    timeouts: list[Record] = [
        {"run": result["run"], "nodeid": failure["nodeid"]}
        for result in every
        for failure in result["junit_failures"]
        if failure["fixture_ps_timeout"]
    ]
    predicate: Record = {
        "red_detected": is_fixture_ps_timeout(PREDICATE_RED),
        "fixture_ps_timeouts": timeouts,
    }
    weakened = {
        weakening: {
            key: sum(r["weakened"][weakening][key] for r in every)
            for key in ("red_undetected", "permitted_flagged")
        }
        for weakening in WEAKENINGS
    }
    passed = (
        bool(every)
        and predicate["red_detected"]
        and all(sum(counts.values()) > 0 for counts in weakened.values())
        and all(
            not r["green_problems"] and not r["red_undetected"] and not r["permitted_flagged"]
            for r in every
        )
    )
    summary = {
        "passed": passed,
        "runs": {name: len(results) for name, results in runs.items()},
        "records": sum(r["records"] for r in every),
        "red_mutations": sum(r["red_mutations"] for r in every),
        "permitted_sequences": sum(r["permitted_sequences"] for r in every),
        "kill_rules": dict(sorted(Counter(k for r in every for k in r["kill_rules"]).items())),
        "junit_failures": sum(len(r["junit_failures"]) for r in every),
        "fixture_ps_timeouts": len(timeouts),
        "weakened": weakened,
    }
    result = {"summary": summary, "predicate": predicate, "runs": runs}
    args.out.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps(summary))
    sys.exit(0 if passed else 1)


if __name__ == "__main__":
    main()
