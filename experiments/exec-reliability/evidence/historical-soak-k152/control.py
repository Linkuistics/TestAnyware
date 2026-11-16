"""Show that a frozen control run carries every timing field in order.

Green: each record of the control soak passes timeline.check, and the run
holds the expected record kinds. Red: for every record, deleting each required
field, and reversing each ordered pair, must make timeline.check report that
exact problem. A checker that accepts any red copy fails this control.
"""

import argparse
import copy
import json
import sys
from pathlib import Path
from typing import Any

import timeline

Record = dict[str, Any]
EXPECTED = {"scope": 4, "prebuffer": 2}


def rules(record: Record) -> tuple[tuple[str, ...], tuple[tuple[str, str], ...]]:
    if record["kind"] == "scope":
        required, order = timeline.SCOPE_REQUIRED, timeline.SCOPE_ORDER
    else:
        required, order = timeline.PREBUFFER_REQUIRED, timeline.PREBUFFER_ORDER
    if record.get("reason") == "exit":
        required = tuple(name for name in required if name != "term")
    return required, order


def red(record: Record) -> list[Record]:
    required, order = rules(record)
    outcomes: list[Record] = []
    for name in required:
        mutant = copy.deepcopy(record)
        mutant[name] = None
        detected = f"missing {name}" in timeline.check(mutant)
        outcomes.append({"mutation": f"delete {name}", "detected": detected})
    for before, after in order:
        if record.get(before) is None or record.get(after) is None:
            continue
        mutant = copy.deepcopy(record)
        mutant[before], mutant[after] = record[after] + 0.001, record[before]
        detected = f"{before} after {after}" in timeline.check(mutant)
        outcomes.append({"mutation": f"reverse {before}/{after}", "detected": detected})
    return outcomes


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("soak", type=Path)
    parser.add_argument("out", type=Path)
    args = parser.parse_args()
    summary: Record = json.loads((args.soak / "soak.json").read_text())
    results: list[Record] = []
    for run in summary["runs"]:
        checked: list[Record] = json.loads((Path(run["run"]) / "timeline.json").read_text())
        kinds = {kind: 0 for kind in EXPECTED}
        for entry in checked:
            kinds[entry["record"]["kind"]] += 1
        reds = [outcome for entry in checked for outcome in red(entry["record"])]
        results.append(
            {
                "run": run["run"],
                "returncode": run["returncode"],
                "kinds": kinds,
                "kinds_ok": kinds == EXPECTED,
                "green_problems": [entry["problems"] for entry in checked if entry["problems"]],
                "red_mutations": len(reds),
                "red_undetected": [outcome for outcome in reds if not outcome["detected"]],
            }
        )
    passed = bool(results) and all(
        result["kinds_ok"] and not result["green_problems"] and not result["red_undetected"]
        for result in results
    )
    args.out.write_text(json.dumps({"passed": passed, "runs": results}, indent=2) + "\n")
    print(json.dumps({"passed": passed, "red_mutations": sum(r["red_mutations"] for r in results)}))
    sys.exit(0 if passed else 1)


if __name__ == "__main__":
    main()
