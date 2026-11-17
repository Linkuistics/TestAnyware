"""Controls for the k14 checker. Each test gives the checker one synthetic record that it
must accept or reject. A checker that accepts everything fails these tests."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import download_k14_check as check

Json = dict[str, Any]
OLD, NEW = b"old-version\n", b"new-version-longer\n"


def trace(advertised: int, emitted: bytes, hook: bool = True) -> list[Json]:
    lines: list[Json] = [
        {"request": 7, "kind": "request", "maxrss_kib": 100},
        {"request": 7, "kind": "open", "descriptor": {"ino": 1, "size": len(OLD)}},
        {"request": 7, "kind": "fstat", "descriptor": {"ino": 1, "size": advertised}},
        {"request": 7, "kind": "headers", "text": f"Content-Length: {advertised}\r\n\r\n"},
        {"request": 7, "kind": "read", "offset": 0, "length": len(emitted),
         "descriptor": {"ino": 1}, "path": {"ino": 2}},
        {"request": 7, "kind": "finish", "maxrss_kib": 100,
         "emitted": {"body_bytes": len(emitted), "sha256": check.sha256(emitted)}},
    ]  # fmt: skip
    if hook:
        lines.insert(3, {"request": 7, "kind": "action", "result": {"op": "replace"}})
    return lines


def raw_case(tmp: Path, body: bytes, end: str = "complete", **extra: Any) -> Path:
    case_dir = tmp / "case"
    case_dir.mkdir()
    for index, version in enumerate((OLD, NEW)):
        (case_dir / f"allowed-{index}.bin").write_bytes(version)
    (case_dir / "received.bin").write_bytes(body)
    record: Json = {
        "case": "replace-longer-after-fstat",
        "client": "raw",
        "event": "after-fstat",
        "allowed": [{"sha256": check.sha256(v)} for v in (OLD, NEW)],
        "result": {
            "status": 200,
            "content_length": len(body),
            "end": end,
            "body": {"bytes": len(body), "sha256": check.sha256(body), "kept": True},
        },
        "agent": {"trace": trace(len(body), body), "arm_unused": False},
    } | extra
    (case_dir / "record.json").write_text(json.dumps(record))
    return case_dir


def judge(case_dir: Path) -> Json:
    record: Json = json.loads((case_dir / "record.json").read_text())
    allowed = check.allowed_versions(case_dir, "replace-longer-after-fstat", record)
    assert allowed is not None
    verdict = check.classify_raw(case_dir, record, allowed, check.allowed_digests(record))
    return verdict


def test_oracle_controls_behave(tmp_path: Path) -> None:
    assert check.oracle_controls(tmp_path)["behaves"]


def test_matches_rejects_prefix_and_suffix() -> None:
    assert check.matches(OLD, [OLD, NEW])
    assert not check.matches(OLD[:-1], [OLD])
    assert not check.matches(OLD + b"x", [OLD])


def test_semantics_controls_reject_other_behaviour() -> None:
    good = {
        "rename-after-open": {"descriptor_reads_old": True, "length": 25},
        "truncate-after-fstat": {"fstat": 123, "length": 25},
        "append-after-fstat": {"fstat": 25, "length": 148},
        "overwrite-between-reads": {"torn": True},
    }
    assert check.semantics_controls(good)["behaves"]
    assert not check.semantics_controls(good | {"overwrite-between-reads": {"torn": False}})[
        "behaves"
    ]
    assert not check.semantics_controls({})["behaves"]


def test_raw_allowed_complete(tmp_path: Path) -> None:
    assert judge(raw_case(tmp_path, OLD))["class"] == "allowed-complete"


def test_raw_undetected_wrong(tmp_path: Path) -> None:
    assert judge(raw_case(tmp_path, NEW[: len(OLD)]))["class"] == "undetected-wrong"


def test_raw_framing_violation(tmp_path: Path) -> None:
    assert judge(raw_case(tmp_path, NEW, "complete-extra-bytes"))["class"] == "framing-violation"


def test_raw_premature_eof_is_detected(tmp_path: Path) -> None:
    assert judge(raw_case(tmp_path, OLD[:3], "eof-before-length"))["class"] == "detected-failure"


def test_raw_changed_received_file_is_unclassified(tmp_path: Path) -> None:
    case_dir = raw_case(tmp_path, OLD)
    (case_dir / "received.bin").write_bytes(NEW)
    assert judge(case_dir)["class"] == "unclassified"


def test_changed_allowed_file_is_rejected(tmp_path: Path) -> None:
    case_dir = raw_case(tmp_path, OLD)
    (case_dir / "allowed-0.bin").write_bytes(b"other")
    record: Json = json.loads((case_dir / "record.json").read_text())
    assert check.allowed_versions(case_dir, "replace-longer-after-fstat", record) is None


def cli_record(result: Json) -> Json:
    return {"allowed": [{"sha256": check.sha256(v)} for v in (OLD, NEW)], "result": result}


def test_cli_wrong_bytes_with_exit_zero(tmp_path: Path) -> None:
    local = tmp_path / "destination" / "file"
    local.parent.mkdir()
    local.write_bytes(NEW[:5])
    result: Json = {"exit": 0, "destination_sha256": check.sha256(NEW[:5]), "temporaries": []}
    verdict = check.classify_cli(tmp_path, cli_record(result), [OLD, NEW], set())
    assert verdict["class"] == "undetected-wrong"


def test_cli_failure_with_temporary_is_a_leak(tmp_path: Path) -> None:
    result = {"exit": 1, "destination_exists": False, "temporaries": [".f.testanyware-1.tmp"]}
    verdict = check.classify_cli(tmp_path, cli_record(result), [OLD, NEW], set())
    assert verdict["class"] == "detected-failure-leak"


def test_cli_interrupt_with_temporary_is_a_leak(tmp_path: Path) -> None:
    result = {"exit": -15, "ended_by": "SIGTERM", "temporaries": [".f.testanyware-1.tmp"]}
    record = cli_record(result) | {"interrupt": "abandon"}
    assert check.classify_cli(tmp_path, record, [OLD], set())["class"] == "interrupt-leak"


def test_unused_arm_makes_the_case_incomplete(tmp_path: Path) -> None:
    out = tmp_path / "out"
    case_dir = out / "cases" / "replace-longer-after-fstat" / "raw"
    case_dir.parent.mkdir(parents=True)
    made = raw_case(tmp_path, OLD)
    made.rename(case_dir)
    assert check.judge_case(out, "replace-longer-after-fstat", "raw")["complete"]
    record: Json = json.loads((case_dir / "record.json").read_text())
    record["agent"] = {"trace": trace(len(OLD), OLD, hook=False), "arm_unused": True}
    (case_dir / "record.json").write_text(json.dumps(record))
    assert not check.judge_case(out, "replace-longer-after-fstat", "raw")["complete"]


def test_emitted_bytes_must_equal_received_bytes(tmp_path: Path) -> None:
    out = tmp_path / "out"
    case_dir = out / "cases" / "replace-longer-after-fstat" / "raw"
    case_dir.parent.mkdir(parents=True)
    raw_case(tmp_path, OLD).rename(case_dir)
    record: Json = json.loads((case_dir / "record.json").read_text())
    record["agent"]["trace"] = trace(len(OLD), NEW)
    (case_dir / "record.json").write_text(json.dumps(record))
    verdict = check.judge_case(out, "replace-longer-after-fstat", "raw")
    assert verdict["agent_bytes_equal_received"] is False
    assert not verdict["complete"]


def test_stress_needs_a_stopped_writer() -> None:
    record: Json = {
        "reads": 2,
        "versions": {check.sha256(OLD): "old"},
        "outcomes": {
            "a": {"count": 2, "status": 200, "end": "complete", "sha256": check.sha256(OLD)}
        },
        "writer": {"stopped": True, "error": None, "writes": 9},
    }
    assert check.stress_summary(record)["complete"]
    record["writer"]["stopped"] = False
    assert not check.stress_summary(record)["complete"]


def test_stress_counts_wrong_bytes() -> None:
    record: Json = {
        "reads": 1,
        "versions": {check.sha256(OLD): "old"},
        "outcomes": {"a": {"count": 1, "status": 200, "end": "complete", "sha256": "x"}},
        "writer": {"stopped": True, "error": None, "writes": 9},
    }
    assert check.stress_summary(record)["classes"] == {"undetected-wrong": 1}


def test_publication_needs_every_record() -> None:
    record: Json = {"publications": 200, "outcomes": {"complete": 200, "400": 5}, "mismatches": []}
    assert check.publication_summary(record)["complete"]
    record["outcomes"]["complete"] = 199
    assert not check.publication_summary(record)["complete"]


def test_check_of_an_empty_directory_is_incomplete(tmp_path: Path) -> None:
    result = check.check(tmp_path)
    assert not result["complete"]
    assert len(result["incomplete"]) == len(check.CASES) * len(check.CLIENTS)
