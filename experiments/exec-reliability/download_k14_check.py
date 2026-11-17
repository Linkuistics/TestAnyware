"""Judge one retained download_k14 OUT directory.

Usage: download_k14_check.py OUT [RESULT]

Each raw and Host CLI download gets one class, with the k7 vocabulary:

- allowed-complete: the transfer reports success and the bytes equal a version that the
  file held during the case.
- detected-failure: the transfer reports failure and leaves no destination and no
  temporary file.
- detected-failure-leak: the transfer reports failure but leaves a temporary file.
- undetected-wrong: the transfer reports success but the bytes equal no allowed version.
- framing-violation: the raw client received bytes after Content-Length.
- client-abandoned: the raw client closed the connection on purpose during the body.
- clean-interrupt or interrupt-leak: the Host CLI received SIGTERM during the body.
- unclassified: the evidence is missing or does not fit one of the classes above.

Each case also gets its agent evidence from the seam trace: the size from fstat, the
advertised Content-Length, the bytes that the agent wrote to the socket, and whether
those bytes equal the bytes that the raw client received.

An undetected-wrong result is a finding, not a check failure. The checker exits 0 only
when the evidence is complete: every planned record exists and is classified, each armed
hook ran at its event, the file semantics controls and the oracle controls behave, the
stress and publication controls are complete, the ports were bound to loopback only and
the container is gone. The checker never repairs, retries or normalises bytes.
"""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from typing import Any

Json = dict[str, Any]

# Independent copies of the constants in download_k14.py. They are not imported, so that an
# error in the controller cannot also change the oracle.
ORACLE_HEX = "00ff80410a"
PENDING = (
    Path(__file__).resolve().parent
    / "evidence/download-consistency-k7/historical/after-abrupt-03-initial.json"
)
PENDING_SHA256 = "b95c9261fb8ff0ca68a9d8b06ebdcb4102a72f60b670c2d748cdf4eca454e7a4"
CASES = [
    "replace-longer-after-open",
    "replace-longer-after-fstat",
    "replace-shorter-after-fstat",
    "replace-between-reads",
    "truncate-after-open",
    "truncate-after-fstat",
    "truncate-between-reads",
    "append-after-fstat",
    "overwrite-longer-after-fstat",
    "overwrite-between-reads",
    "immutable-binary",
    "missing-file",
    "agent-killed-mid-body",
    "client-abandons-mid-body",
    "large-immutable",
]
CLIENTS = ["raw", "cli"]
ORACLE_CASE = "immutable-binary"
LARGE_CASE = "large-immutable"
LARGE_BYTES = 64 * 1024 * 1024
PUBLICATIONS = 200
TRANSFER_BOUND = 150.0
# A streaming agent holds one 1 MiB chunk. A 16 MiB rise in peak RSS during the 64 MiB
# transfer would mean that the agent holds a large part of the file.
MEMORY_RISE_LIMIT_KIB = 16 * 1024


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


def semantics_controls(controls: Json) -> Json:
    """The file semantics controls without HTTP. Each must show the Linux behaviour that
    the forced cases depend on, or the forced cases cannot be read."""
    expected = {
        "rename-after-open": controls.get("rename-after-open")
        == {"descriptor_reads_old": True, "length": 25},
        "truncate-after-fstat": controls.get("truncate-after-fstat")
        == {"fstat": 123, "length": 25},
        "append-after-fstat": controls.get("append-after-fstat") == {"fstat": 25, "length": 148},
        "overwrite-between-reads": controls.get("overwrite-between-reads") == {"torn": True},
    }
    return {"checks": expected, "behaves": all(expected.values())}


def allowed_versions(case_dir: Path, case: str, record: Json) -> list[bytes] | None:
    """The allowed versions as files. A version too large to keep is absent, and the
    record digest stands for it. None means that a kept file differs from the record."""
    if case == ORACLE_CASE:
        return [bytes.fromhex(ORACLE_HEX)]
    kept = [p.read_bytes() for p in sorted(case_dir.glob("allowed-*.bin"))]
    planned = [entry["sha256"] for entry in record.get("allowed", [])]
    if any(sha256(v) not in planned for v in kept):
        return None
    return kept


def allowed_digests(record: Json) -> set[str]:
    return {entry["sha256"] for entry in record.get("allowed", [])}


def download_request(trace: list[Json]) -> int | None:
    """The request that opened the case file. Health probes open nothing."""
    opened = [line["request"] for line in trace if line["kind"] == "open"]
    return opened[0] if opened else None


def field(value: Any, name: str) -> Any:
    """One field of a JSON object that may be absent."""
    if not isinstance(value, dict):
        return None
    mapping: Json = value  # pyright: ignore[reportUnknownVariableType]
    return mapping.get(name)


def agent_evidence(record: Json) -> Json:
    agent: Json = record.get("agent") or {}
    trace: list[Json] = agent.get("trace") or []
    number = download_request(trace)
    lines = [line for line in trace if line["request"] == number] if number else []
    kinds = [line["kind"] for line in lines]
    fstat = next((x["descriptor"]["size"] for x in lines if x["kind"] == "fstat"), None)
    header = next((x["text"] for x in lines if x["kind"] == "headers"), "")
    advertised = None
    for text in header.split("\r\n"):
        name, _, value = text.partition(":")
        if name.strip().lower() == "content-length":
            advertised = int(value.strip())
    finish = next((x for x in lines if x["kind"] == "finish"), None)
    emitted: Json = finish["emitted"] if finish else {}
    reads = [x for x in lines if x["kind"] == "read"]
    rss = [x["maxrss_kib"] for x in lines if "maxrss_kib" in x]
    return {
        "request": number,
        "fstat_size": fstat,
        "content_length": advertised,
        "emitted_bytes": emitted.get("body_bytes"),
        "emitted_sha256": emitted.get("sha256"),
        "reads": [{"offset": x["offset"], "length": x["length"]} for x in reads],
        "descriptor_inode": reads[0]["descriptor"]["ino"] if reads else None,
        "path_inode_at_last_read": field(reads[-1].get("path"), "ino") if reads else None,
        "action": next((x.get("result") for x in lines if x["kind"] == "action"), None),
        "action_error": next((x.get("error") for x in lines if x["kind"] == "action-error"), None),
        "write_error": next((x.get("error") for x in lines if x["kind"] == "write-error"), None),
        "hook_ran": "action" in kinds or "paused" in kinds,
        "maxrss_kib": [min(rss), max(rss)] if rss else None,
    }


def classify_raw(case_dir: Path, record: Json, allowed: list[bytes], digests: set[str]) -> Json:
    if record.get("interrupt") == "abandon":
        abandoned: Json | None = record.get("abandoned")
        if "error" in record or not abandoned or not field(record.get("paused"), "paused"):
            return {"class": "unclassified", "reason": record.get("error") or "no abandon"}
        return {"class": "client-abandoned", "received": abandoned.get("received")}
    result: Json = record.get("result") or {}
    if "error" in record or "end" not in result:
        return {"class": "unclassified", "reason": record.get("error") or "no raw result"}
    body: Json = result.get("body") or {}
    received = case_dir / "received.bin"
    if body.get("kept"):
        if not received.exists() or sha256(received.read_bytes()) != body.get("sha256"):
            return {"class": "unclassified", "reason": "received.bin differs from the record"}
        good = matches(received.read_bytes(), allowed)
    else:
        good = body.get("sha256") in digests
    out: Json = {
        "status": result.get("status"),
        "content_length": result.get("content_length"),
        "received": body.get("bytes"),
        "end": result["end"],
        "sha256": body.get("sha256"),
        "seconds": result.get("seconds"),
    }
    end = result["end"]
    if end == "complete-extra-bytes":
        return out | {"class": "framing-violation"}
    if end in ("complete", "complete-open") and result.get("status") == 200:
        return out | {"class": "allowed-complete" if good else "undetected-wrong"}
    if end in ("complete", "complete-open") and not allowed:
        return out | {"class": "detected-failure"}  # a non-200 answer for a missing file
    if end in ("eof-before-length", "timeout-before-length"):
        return out | {"class": "detected-failure"}
    return out | {"class": "unclassified", "reason": f"end {end}"}


def classify_cli(case_dir: Path, record: Json, allowed: list[bytes], digests: set[str]) -> Json:
    result: Json = record.get("result") or {}
    if "error" in record or "exit" not in result:
        return {"class": "unclassified", "reason": record.get("error") or "no CLI result"}
    out: Json = {
        "exit": result["exit"],
        "temporaries": result.get("temporaries"),
        "seconds": result.get("seconds"),
        "stdout": str(result.get("stdout", ""))[:300],
    }
    if record.get("interrupt") == "abandon":
        if result.get("ended_by") not in ("SIGTERM", "SIGKILL"):
            return out | {"class": "unclassified", "reason": "the CLI was not interrupted"}
        clean = not result.get("destination_exists") and not result.get("temporaries")
        return out | {
            "class": "clean-interrupt" if clean else "interrupt-leak",
            "ended_by": result.get("ended_by"),
            "temporaries_while_paused": record.get("temporaries_while_paused"),
        }
    if result.get("killed_at_bound") or result.get("ended_by") == "SIGKILL":
        return out | {"class": "unclassified", "reason": "killed at the CLI bound"}
    local = case_dir / "destination" / "file"
    if result["exit"] == 0:
        digest = result.get("destination_sha256")
        if local.exists():
            body = local.read_bytes()
            if sha256(body) != digest:
                return out | {"class": "unclassified", "reason": "destination differs"}
            good = matches(body, allowed)
        elif result.get("destination_removed_after_digest"):
            good = digest in digests
        else:
            return out | {"class": "unclassified", "reason": "exit 0 without a destination"}
        out |= {"destination_bytes": result.get("destination_bytes"), "sha256": digest}
        return out | {"class": "allowed-complete" if good else "undetected-wrong"}
    if result.get("destination_exists"):
        return out | {"class": "unclassified", "reason": "failure left a destination"}
    if result.get("temporaries"):
        return out | {"class": "detected-failure-leak"}
    return out | {"class": "detected-failure"}


def judge_case(out: Path, case: str, client: str) -> Json:
    case_dir = out / "cases" / case / client
    path = case_dir / "record.json"
    if not path.exists():
        return {"class": "unclassified", "reason": "no record", "complete": False}
    record: Json = json.loads(path.read_text())
    allowed = allowed_versions(case_dir, case, record)
    if allowed is None:
        return {"class": "unclassified", "reason": "an allowed file changed", "complete": False}
    digests = allowed_digests(record)
    classify = classify_raw if client == "raw" else classify_cli
    verdict = classify(case_dir, record, allowed, digests)
    agent = agent_evidence(record)
    verdict["agent"] = agent
    armed = record.get("event") is not None
    hook_ok = not armed or (agent["hook_ran"] and not field(record.get("agent"), "arm_unused"))
    wire_ok: bool | None = None
    if client == "raw" and verdict.get("sha256") and agent["emitted_sha256"]:
        # The bytes that the agent wrote equal the bytes that the raw client received.
        wire_ok = verdict["sha256"] == agent["emitted_sha256"]
    verdict["agent_bytes_equal_received"] = wire_ok
    if record.get("interrupt"):
        health: Json = record.get("health_after") or {}
        verdict["agents_healthy_after"] = all(v.get("status") == 200 for v in health.values())
    verdict["complete"] = (
        verdict["class"] != "unclassified"
        and hook_ok
        and wire_ok is not False
        and agent["action_error"] is None
        and verdict.get("agents_healthy_after", True)
    )
    if not hook_ok:
        verdict["reason"] = "the armed hook did not run at its event"
    return verdict


def stress_summary(record: Json) -> Json:
    """Complete only when every read is classified and the writer stopped."""
    if "outcomes" not in record or record.get("error"):
        return {"complete": False, "reason": record.get("error") or "no outcomes"}
    versions: Json = record.get("versions") or {}
    classes: dict[str, int] = {}
    for entry in record["outcomes"].values():
        end = entry["end"]
        if end in ("complete", "complete-open") and entry["status"] == 200:
            key = "allowed-complete" if entry["sha256"] in versions else "undetected-wrong"
        elif end in ("eof-before-length", "timeout-before-length"):
            key = "detected-failure"
        elif end == "complete-extra-bytes":
            key = "framing-violation"
        else:
            key = "unclassified"
        classes[key] = classes.get(key, 0) + entry["count"]
    writer: Json = record.get("writer") or {}
    return {
        "complete": record["reads"] > 0
        and sum(classes.values()) == record["reads"]
        and "unclassified" not in classes
        and writer.get("stopped") is True
        and writer.get("error") is None
        and int(writer.get("writes", 0)) > 0,
        "reads": record["reads"],
        "writes": writer.get("writes"),
        "classes": classes,
    }


def publication_summary(record: Json) -> Json:
    """Complete only when every publication got one classified 200 read."""
    if "outcomes" not in record or record.get("error"):
        return {"complete": False, "reason": record.get("error") or "no outcomes"}
    outcomes: Json = record["outcomes"]
    good = int(outcomes.get("complete", 0))
    bad = len(record.get("mismatches", []))
    return {
        "complete": record.get("publications") == PUBLICATIONS
        and int(outcomes.get("mismatch", 0)) == bad
        and good + bad == PUBLICATIONS
        and "unanswered" not in outcomes,
        "publications": record.get("publications"),
        "complete_reads": good,
        "mismatches": bad,
        "not_yet_published_answers": {
            k: v for k, v in outcomes.items() if k not in ("complete", "mismatch")
        },
    }


def loopback_only(out: Path) -> bool:
    path = out / "container-inspect.json"
    if not path.exists():
        return False
    inspect: list[Json] = json.loads(path.read_text() or "[]")
    if not inspect:
        return False
    bindings: dict[str, list[Json] | None] = inspect[0]["HostConfig"]["PortBindings"] or {}
    hosts: list[str] = [b["HostIp"] for entries in bindings.values() for b in entries or []]
    return bool(hosts) and all(h == "127.0.0.1" for h in hosts)


def large_streaming(verdicts: Json) -> Json:
    """The 64 MiB transfer stayed within the bound and the agent held little of it."""
    result: Json = {}
    for client in CLIENTS:
        verdict: Json = verdicts[LARGE_CASE][client]
        agent: Json = verdict.get("agent") or {}
        rss = agent.get("maxrss_kib")
        rise = rss[1] - rss[0] if rss else None
        seconds = verdict.get("seconds")
        result[client] = {
            "seconds": seconds,
            "maxrss_rise_kib": rise,
            "within_bound": seconds is not None and seconds <= TRANSFER_BOUND,
            "fixed_memory": rise is not None and rise <= MEMORY_RISE_LIMIT_KIB,
            "emitted_bytes": agent.get("emitted_bytes"),
        }
    result["behaves"] = all(
        result[c]["within_bound"]
        and result[c]["fixed_memory"]
        and result[c]["emitted_bytes"] == LARGE_BYTES
        for c in CLIENTS
    )
    return result


def check(out: Path) -> Json:
    pending_ok = PENDING.exists() and sha256(PENDING.read_bytes()) == PENDING_SHA256
    controls_path = out / "controls.json"
    controls: Json = json.loads(controls_path.read_text()) if controls_path.exists() else {}
    semantics = semantics_controls(controls)
    oracle = oracle_controls(out)
    verdicts: Json = {case: {c: judge_case(out, case, c) for c in CLIENTS} for case in CASES}
    stress_path, publication_path = out / "stress.json", out / "immutable.json"
    stress = stress_summary(json.loads(stress_path.read_text())) if stress_path.exists() else {}
    publication = (
        publication_summary(json.loads(publication_path.read_text()))
        if publication_path.exists()
        else {}
    )
    summary_path = out / "summary.json"
    summary: Json = json.loads(summary_path.read_text()) if summary_path.exists() else {}
    disposal: Json = summary.get("disposal") or {}
    large = large_streaming(verdicts)
    incomplete = [
        f"{case}/{c}" for case in CASES for c in CLIENTS if not verdicts[case][c]["complete"]
    ]
    classes = {f"{case}/{c}": verdicts[case][c]["class"] for case in CASES for c in CLIENTS}
    named = ("undetected-wrong", "framing-violation", "detected-failure-leak", "interrupt-leak")
    findings = {name: [k for k, v in classes.items() if v == name] for name in named}
    gates = {
        "pending_record_unchanged": pending_ok,
        "semantics_controls": semantics["behaves"],
        "oracle_controls": oracle["behaves"],
        "every_case_complete": not incomplete,
        "stress_complete": bool(stress.get("complete")),
        "publication_complete": bool(publication.get("complete")),
        "large_streaming": large["behaves"],
        "loopback_only": loopback_only(out),
        "container_removed": disposal.get("removed") is True,
        "controller_error": summary.get("error") is None,
    }
    return {
        "complete": all(gates.values()),
        "gates": gates,
        "incomplete": incomplete,
        "findings": findings,
        "semantics_controls": semantics,
        "oracle_controls": oracle,
        "large_streaming": large,
        "stress": stress,
        "publication": publication,
        "cases": verdicts,
    }


def main(argv: list[str]) -> int:
    if len(argv) not in (2, 3):
        raise SystemExit(__doc__)
    out = Path(argv[1])
    result = check(out)
    # A relative RESULT is relative to OUT, as in the k7 checker. An absolute RESULT stays absolute.
    target = out / (argv[2] if len(argv) == 3 else "check.json")
    target.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(f"complete {result['complete']}")
    for name, items in result["findings"].items():
        print(f"{name}: {', '.join(items) or 'none'}")
    if result["incomplete"]:
        print(f"incomplete: {', '.join(result['incomplete'])}")
    print("gates: " + ", ".join(f"{k}={v}" for k, v in result["gates"].items()))
    return 0 if result["complete"] else 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
