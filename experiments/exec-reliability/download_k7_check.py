"""Judge one retained download_k7 OUT directory.

Usage: download_k7_check.py OUT [RESULT]

Each raw and Host CLI download gets one class:

- allowed-complete: the transfer reports success and the bytes equal a version that the
  file held during the case.
- detected-failure: the transfer reports failure and leaves no destination and no
  temporary file.
- detected-failure-leak: the transfer reports failure but leaves a temporary file.
- undetected-wrong: the transfer reports success but the bytes equal no allowed version.
- framing-violation: the raw client received bytes after Content-Length.
- unclassified: the evidence is missing or does not fit one of the classes above.

An interrupted Host CLI is clean-interrupt or interrupt-leak. An undetected-wrong result
is a finding, not a check failure. The checker exits 0 only when the evidence is complete:
every planned record exists and is classified, the oracle controls behave, every agent
listened on loopback only and stopped, and the historical failure D files keep their
digests. The checker never repairs, retries or normalises bytes.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

Json = dict[str, Any]

# Independent copies of the constants in download_k7.py. They are not imported, so that an
# error in the controller cannot also change the oracle.
ORACLE_HEX = "00ff80410a"
HISTORICAL = Path(
    "/Users/antony/Development/AgenticOrchestration/Ravel-Strategies.gh-issue-1/"
    "experiments/tutorial-vm-reliability/evidence/macos-candidate-02"
)
HISTORICAL_SHA256 = {
    "after-abrupt-03-initial.json": (
        "b95c9261fb8ff0ca68a9d8b06ebdcb4102a72f60b670c2d748cdf4eca454e7a4"
    ),
    "after-abrupt-03-state-002.json": (
        "ca41852314b967a327b0a4faf0240cac2f2e304387439e5518cf380da4b9a56e"
    ),
    "after-abrupt-03-collect-002.json": (
        "6645b2e4b55f2b89a7e635bfa6bd17074f538c2978606e01852ce049cc3740a3"
    ),
    "after-abrupt-03-launch.json": (
        "eb9c539fccdae4d746627b159d754845a725782efe37d548b61f57de555338b7"
    ),
}
FAILURE_D_SHA256 = HISTORICAL_SHA256["after-abrupt-03-state-002.json"]
CASES = [
    "replace-longer",
    "replace-shorter",
    "truncate-during-read",
    "overwrite-during-read",
    "append-during-read",
    "immutable-binary",
    "agent-killed-mid-body",
]
CLI_ONLY = ["cli-killed-mid-body"]


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def matches(body: bytes, allowed: list[bytes]) -> bool:
    """Exact equality with one allowed version. No prefix, suffix or trim is accepted."""
    return any(body == version for version in allowed)


def oracle_controls(out: Path) -> Json:
    """Write the oracle and two corrupted copies, then read each back from disk. The
    comparator must accept the exact copy and reject the flipped and truncated copies."""
    oracle = bytes.fromhex(ORACLE_HEX)
    flipped = bytearray(oracle)
    flipped[2] ^= 0x01
    copies = {"exact": oracle, "flipped": bytes(flipped), "truncated": oracle[:4]}
    folder = out / "oracle-controls"
    folder.mkdir(exist_ok=True)
    result: Json = {}
    for name, data in copies.items():
        path = folder / f"{name}.bin"
        path.write_bytes(data)
        read = path.read_bytes()
        result[name] = {"sha256": sha256(read), "accepted": matches(read, [oracle])}
    result["behaves"] = (
        result["exact"]["accepted"]
        and not result["flipped"]["accepted"]
        and not result["truncated"]["accepted"]
    )
    return result


def allowed_versions(case_dir: Path) -> list[bytes]:
    return [p.read_bytes() for p in sorted(case_dir.glob("allowed-*.bin"))]


def classify_raw(case_dir: Path, record: Json, allowed: list[bytes]) -> Json:
    result: Json = record.get("result") or {}
    received = case_dir / "received.bin"
    if "error" in record or "end" not in result or not received.exists():
        return {"class": "unclassified", "reason": record.get("error") or "no raw result"}
    body = received.read_bytes()
    if sha256(body) != result.get("sha256"):
        return {"class": "unclassified", "reason": "received.bin differs from the record"}
    out: Json = {
        "status": result.get("status"),
        "content_length": result.get("content_length"),
        "received": len(body),
        "end": result["end"],
        "sha256": sha256(body),
        "failure_d_bytes": sha256(body) == FAILURE_D_SHA256,
    }
    end = result["end"]
    if end == "complete-extra-bytes":
        return out | {"class": "framing-violation"}
    if end in ("complete", "complete-open") and result.get("status") == 200:
        allowed_match = matches(body, allowed)
        return out | {"class": "allowed-complete" if allowed_match else "undetected-wrong"}
    if end in ("eof-before-length", "timeout-before-length"):
        return out | {"class": "detected-failure"}
    return out | {"class": "unclassified", "reason": f"end {end}"}


def classify_cli(case_dir: Path, record: Json, allowed: list[bytes]) -> Json:
    if "cli_killed" in record:
        killed: Json = record["cli_killed"]
        clean = not killed.get("destination_exists") and not killed.get("temporaries_after")
        return {
            "class": "clean-interrupt" if clean else "interrupt-leak",
            "exit": killed.get("exit"),
            "temporaries_while_paused": killed.get("temporaries_while_paused"),
            "temporaries_after": killed.get("temporaries_after"),
        }
    result: Json = record.get("result") or {}
    if "error" in record or "exit" not in result:
        return {"class": "unclassified", "reason": record.get("error") or "no CLI result"}
    out: Json = {
        "exit": result["exit"],
        "killed_at_bound": result.get("killed_at_bound"),
        "temporaries": result.get("temporaries"),
        "stderr": str(result.get("stderr", ""))[:300],
    }
    local = case_dir / "local" / f"{record['case']}.bin"
    if result["exit"] == 0:
        if not local.exists():
            return out | {"class": "unclassified", "reason": "exit 0 without a destination"}
        body = local.read_bytes()
        try:
            receipt: Json = json.loads(str(result.get("stdout", "")))
        except json.JSONDecodeError:
            receipt = {}
        out |= {
            "destination_bytes": len(body),
            "sha256": sha256(body),
            "receipt_bytes": receipt.get("bytes"),
            "failure_d_bytes": sha256(body) == FAILURE_D_SHA256,
        }
        return out | {"class": "allowed-complete" if matches(body, allowed) else "undetected-wrong"}
    if result.get("killed_at_bound"):
        return out | {"class": "unclassified", "reason": "killed at the CLI bound"}
    if local.exists():
        return out | {"class": "unclassified", "reason": "failure left a destination"}
    if result.get("temporaries"):
        return out | {"class": "detected-failure-leak"}
    return out | {"class": "detected-failure"}


def agent_ok(record: Json) -> bool:
    agent: Json = record.get("agent") or {}
    return bool(agent.get("loopback_only")) and agent.get("alive_after_stop") is False


def stress_summary(record: Json) -> Json:
    if "outcomes" not in record:
        return {"complete": False}
    if "publications" in record:
        outcomes: Json = record["outcomes"]
        return {
            "complete": True,
            "publications": record["publications"],
            "complete_reads": outcomes.get("complete", 0),
            "mismatches": len(record.get("mismatches", [])),
        }
    if "versions" not in record:
        return {"complete": False}
    versions: Json = record["versions"]
    classes: dict[str, int] = {}
    wrong: dict[str, int] = {}
    for entry in record["outcomes"].values():
        end = entry["end"]
        if end in ("complete", "complete-open") and entry["status"] == 200:
            if entry["sha256"] in versions:
                key = "allowed-complete"
            else:
                key = "undetected-wrong"
                wrong[entry["sha256"]] = wrong.get(entry["sha256"], 0) + entry["count"]
        elif end in ("eof-before-length", "timeout-before-length"):
            key = "detected-failure"
        else:
            key = "unclassified"
        classes[key] = classes.get(key, 0) + entry["count"]
    return {
        "complete": True,
        "reads": record["reads"],
        "swaps": record["swaps"],
        "classes": classes,
        "undetected_wrong_sha256": wrong,
        "failure_d_bytes": wrong.get(FAILURE_D_SHA256, 0),
    }


def check(out: Path) -> Json:
    meta: Json = json.loads((out / "meta.json").read_text())
    historical_now = {
        name: sha256((HISTORICAL / name).read_bytes()) if (HISTORICAL / name).exists() else None
        for name in HISTORICAL_SHA256
    }
    historical_ok = (
        historical_now == HISTORICAL_SHA256
        and meta.get("historical_before") == HISTORICAL_SHA256
        and meta.get("historical_after") == HISTORICAL_SHA256
    )
    oracle = oracle_controls(out)
    problems: list[str] = []
    if not historical_ok:
        problems.append("historical failure D digests changed or are missing")
    if not oracle["behaves"]:
        problems.append("oracle controls did not behave")
    agents: Json = {}
    for label in meta["agents"]:
        root = out / label
        rows: Json = {}
        calibration = root / "calibration" / "calibration.json"
        if not calibration.exists():
            problems.append(f"{label}: no calibration")
        for case in CASES + CLI_ONLY:
            clients = ["cli"] if case in CLI_ONLY else ["raw", "cli"]
            for client in clients:
                case_dir = root / f"{case}-{client}"
                path = case_dir / "record.json"
                if not path.exists():
                    rows[f"{case}/{client}"] = {"class": "unclassified", "reason": "missing"}
                    problems.append(f"{label}: {case}/{client} missing")
                    continue
                record: Json = json.loads(path.read_text())
                allowed = allowed_versions(case_dir)
                if client == "raw":
                    row = classify_raw(case_dir, record, allowed)
                else:
                    row = classify_cli(case_dir, record, allowed)
                row["agent_ok"] = agent_ok(record)
                if row["class"] == "unclassified":
                    problems.append(f"{label}: {case}/{client} unclassified")
                if not row["agent_ok"]:
                    problems.append(f"{label}: {case}/{client} agent not loopback or not stopped")
                rows[f"{case}/{client}"] = row
        for name in ("stress-mutable", "stress-immutable"):
            path = root / name / "record.json"
            if not path.exists():
                problems.append(f"{label}: {name} missing")
                continue
            record = json.loads(path.read_text())
            summary = stress_summary(record) | {"agent_ok": agent_ok(record)}
            if not summary["complete"] or not summary["agent_ok"]:
                problems.append(f"{label}: {name} incomplete")
            rows[name] = summary
        agents[label] = rows
    return {
        "complete": not problems,
        "problems": problems,
        "historical": {"now": historical_now, "unchanged": historical_ok},
        "oracle_controls": oracle,
        "agents": agents,
    }


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        print(__doc__, file=sys.stderr)
        return 64
    out = Path(argv[1])
    result = check(out)
    target = out / (argv[2] if len(argv) == 3 else "check.json")
    if target.exists():
        print(f"refusing to overwrite {target}", file=sys.stderr)
        return 2
    target.write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({"complete": result["complete"], "problems": result["problems"]}))
    return 0 if result["complete"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
