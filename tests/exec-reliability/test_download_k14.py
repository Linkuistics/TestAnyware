"""Controls for the k14 checker. Each test gives the checker one synthetic record that it
must accept or reject. A checker that accepts everything fails these tests."""

from __future__ import annotations

import json
from collections.abc import Sequence
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
        action = {"request": 7, "kind": "action", "event": "after-fstat", "result": {"op": "x"}}
        lines.insert(3, action)
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


def placed_case(tmp_path: Path) -> tuple[Path, Path, Json]:
    """A complete raw case under OUT, and its record, ready to break one way."""
    out = tmp_path / "out"
    case_dir = out / "cases" / "replace-longer-after-fstat" / "raw"
    case_dir.parent.mkdir(parents=True)
    raw_case(tmp_path, OLD).rename(case_dir)
    assert check.judge_case(out, "replace-longer-after-fstat", "raw")["complete"]
    return out, case_dir, json.loads((case_dir / "record.json").read_text())


def rejudge(out: Path, case_dir: Path, record: Json) -> Json:
    (case_dir / "record.json").write_text(json.dumps(record))
    return check.judge_case(out, "replace-longer-after-fstat", "raw")


def test_hook_with_the_wrong_event_is_incomplete(tmp_path: Path) -> None:
    out, case_dir, record = placed_case(tmp_path)
    record["agent"]["trace"][3]["event"] = "after-open"
    assert not rejudge(out, case_dir, record)["complete"]


def test_hook_in_the_wrong_place_is_incomplete(tmp_path: Path) -> None:
    out, case_dir, record = placed_case(tmp_path)
    lines: list[Json] = record["agent"]["trace"]
    lines.insert(5, lines.pop(3))  # after the headers, not between fstat and headers
    assert not rejudge(out, case_dir, record)["complete"]


def test_two_hook_lines_are_incomplete(tmp_path: Path) -> None:
    out, case_dir, record = placed_case(tmp_path)
    lines: list[Json] = record["agent"]["trace"]
    lines.insert(3, dict(lines[3]))
    assert not rejudge(out, case_dir, record)["complete"]


def test_hook_in_an_unarmed_case_is_incomplete(tmp_path: Path) -> None:
    out, case_dir, record = placed_case(tmp_path)
    record["event"] = None
    assert not rejudge(out, case_dir, record)["complete"]
    record["agent"]["trace"].pop(3)
    assert rejudge(out, case_dir, record)["complete"]


def test_before_read_hook_needs_its_read_count() -> None:
    kinds = ["request", "open", "fstat", "headers", "read", "action", "read"]
    assert check.hook_placed(kinds, 5, "before-read-2")
    assert not check.hook_placed(kinds, 5, "before-read-3")
    assert not check.hook_placed(kinds, 5, "before-read-1")


def test_each_needed_boundary_is_required(tmp_path: Path) -> None:
    for kind in ("open", "fstat", "headers", "read", "finish"):
        out, case_dir, record = placed_case(tmp_path / kind)
        lines: list[Json] = record["agent"]["trace"]
        record["agent"]["trace"] = [x for x in lines if x["kind"] != kind]
        assert not rejudge(out, case_dir, record)["complete"], kind


def test_headers_without_content_length_are_incomplete(tmp_path: Path) -> None:
    out, case_dir, record = placed_case(tmp_path)
    record["agent"]["trace"][4]["text"] = "Content-Type: x\r\n\r\n"
    assert not rejudge(out, case_dir, record)["complete"]


def test_absent_agent_bytes_fail_the_wire_check(tmp_path: Path) -> None:
    out, case_dir, record = placed_case(tmp_path)
    record["agent"]["trace"][-1]["emitted"] = {}
    verdict = rejudge(out, case_dir, record)
    assert verdict["agent_bytes_equal_received"] is False
    assert not verdict["complete"]


def test_missing_file_trace_needs_no_open() -> None:
    kinds = ["request", "headers", "write", "finish"]
    assert check.trace_check(kinds, "missing", None)["ok"]
    assert not check.trace_check(kinds[:-1], "missing", None)["ok"]
    assert not check.trace_check(["request", "open", *kinds[1:]], "missing", None)["ok"]


def test_killed_trace_needs_the_pause_and_no_finish() -> None:
    kinds = ["request", "open", "fstat", "headers", "read", "write", "paused"]
    assert check.trace_check(kinds, "killed", 10)["ok"]
    assert not check.trace_check([*kinds, "finish"], "killed", 10)["ok"]
    assert not check.trace_check(kinds[:-1], "killed", 10)["ok"]


def test_killed_case_uses_the_kept_blob(tmp_path: Path) -> None:
    out = tmp_path / "out"
    case_dir = out / "cases" / "agent-killed-mid-body" / "raw"
    case_dir.parent.mkdir(parents=True)
    raw_case(tmp_path, OLD[:3], "eof-before-length").rename(case_dir)
    record: Json = json.loads((case_dir / "record.json").read_text())
    lines: list[Json] = record["agent"]["trace"][:-1]  # no finish
    lines.pop(3)
    lines.insert(5, {"request": 7, "kind": "paused", "event": "before-read-2"})
    lines[3]["text"] = f"Content-Length: {len(OLD)}\r\n\r\n"
    record |= {"event": "before-read-2", "interrupt": "kill-agent"}
    record["agent"]["trace"] = lines
    record["health_after"] = {"case": {"status": 200}}
    record["emitted"] = [{"request": 7, "bytes": 3, "sha256": check.sha256(OLD[:3])}]
    (case_dir / "record.json").write_text(json.dumps(record))
    verdict = check.judge_case(out, "agent-killed-mid-body", "raw")
    assert verdict["agent"]["emitted_from"] == "kept-blob"
    assert verdict["complete"]
    record["emitted"] = []
    (case_dir / "record.json").write_text(json.dumps(record))
    assert not check.judge_case(out, "agent-killed-mid-body", "raw")["complete"]


def receipts_dir(tmp_path: Path, name: str, items: Sequence[tuple[Json, bytes | None]]) -> Path:
    out = tmp_path / "out"
    (out / f"{name}-bodies").mkdir(parents=True)
    lines: list[str] = []
    for item, body in items:
        if body is not None:
            (out / f"{name}-bodies" / f"{check.sha256(body)}.bin").write_bytes(body)
        lines.append(json.dumps(item))
    (out / f"{name}.jsonl").write_text("".join(x + "\n" for x in lines))
    return out


def read_item(index: int, body: bytes, end: str = "complete", status: int = 200) -> Json:
    stamp = 1000 * (index + 1)
    return {"index": index, "status": status, "end": end, "sha256": check.sha256(body),
            "started_ns": stamp, "ended_ns": stamp + 10}  # fmt: skip


def stress_record(reads: int) -> Json:
    return {
        "reads": reads,
        "receipts": "stress-reads.jsonl",
        "versions": {check.sha256(OLD): "old", check.sha256(NEW): "new"},
        "writer": {"stopped": True, "error": None, "writes": 9},
        "error": None,
    }


def test_stress_is_judged_item_by_item(tmp_path: Path) -> None:
    items = [(read_item(0, OLD), OLD), (read_item(1, NEW), NEW), (read_item(2, NEW[:4]), NEW[:4])]
    out = receipts_dir(tmp_path, "stress-reads", items)
    summary = check.stress_summary(out, stress_record(3))
    assert summary["complete"]
    assert summary["classes"] == {"allowed-complete": 2, "undetected-wrong": 1}
    assert summary["undetected_wrong_indices"] == [2]


def test_stress_needs_a_stopped_writer(tmp_path: Path) -> None:
    out = receipts_dir(tmp_path, "stress-reads", [(read_item(0, OLD), OLD)])
    record = stress_record(1)
    record["writer"]["stopped"] = False
    assert not check.stress_summary(out, record)["complete"]


def test_stress_rejects_aggregates_and_lost_items(tmp_path: Path) -> None:
    out = receipts_dir(tmp_path, "stress-reads", [(read_item(0, OLD), OLD)])
    assert not check.stress_summary(out, stress_record(2))["complete"]
    assert not check.stress_summary(out, stress_record(1) | {"receipts": None})["complete"]
    (out / "stress-reads-bodies" / f"{check.sha256(OLD)}.bin").write_bytes(NEW)
    assert not check.stress_summary(out, stress_record(1))["complete"]


def test_stress_needs_monotonic_timings(tmp_path: Path) -> None:
    late, early = read_item(0, OLD), read_item(1, OLD)
    early["started_ns"] = late["started_ns"] - 1
    out = receipts_dir(tmp_path, "stress-reads", [(late, OLD), (early, OLD)])
    assert not check.stress_summary(out, stress_record(2))["complete"]


def publications(tmp_path: Path, drop: int | None = None, wrong: int | None = None) -> Path:
    items: list[tuple[Json, bytes | None]] = []
    stamp = 0
    for index in range(check.PUBLICATIONS):
        data = OLD + str(index).encode()
        stamp += 100
        publish: Json = {"kind": "publish", "publication": index}
        publish |= {
            "expected_sha256": check.sha256(data),
            "started_ns": stamp,
            "ended_ns": stamp + 1,
        }
        items.append((publish, None))
        polls = [(400, b"no"), (200, data if index != wrong else NEW)]
        for attempt, (status, body) in enumerate(polls):
            stamp += 10
            item = {"kind": "read", "publication": index, "attempt": attempt, "status": status,
                    "end": "complete", "sha256": check.sha256(body),
                    "started_ns": stamp, "ended_ns": stamp + 5}  # fmt: skip
            if index != drop or status != 200:
                items.append((item, body if status == 200 else None))
    return receipts_dir(tmp_path, "publication-reads", items)


def test_publication_is_judged_item_by_item(tmp_path: Path) -> None:
    record: Json = {"publications": 200, "receipts": "publication-reads.jsonl"}
    summary = check.publication_summary(publications(tmp_path / "a"), record)
    assert summary["complete"]
    assert summary["complete_reads"] == 200
    assert summary["not_yet_published_answers"] == {"400": 200}
    summary = check.publication_summary(publications(tmp_path / "b", wrong=7), record)
    assert summary["complete"]
    assert summary["mismatches"] == [7]
    summary = check.publication_summary(publications(tmp_path / "c", drop=9), record)
    assert not summary["complete"]
    assert summary["unanswered"] == [9]


def memory_controls(earlier: int, reset_gap: int, rise: int) -> Json:
    after = {"VmRSS": 13000, "VmHWM": 13000 + reset_gap}
    return {
        "peak-memory": {
            "before_reset": {"VmRSS": 13000, "VmHWM": 13000 + earlier},
            "reset_error": None,
            "after_reset": after,
            "after_32mib": {"VmRSS": 0, "VmHWM": after["VmHWM"] + rise},
        }
    }


def test_memory_control_needs_reset_and_rise() -> None:
    assert check.memory_control(memory_controls(98000, 300, 32700))["behaves"]
    assert not check.memory_control(memory_controls(1000, 300, 32700))["behaves"]
    assert not check.memory_control(memory_controls(98000, 98000, 32700))["behaves"]
    assert not check.memory_control(memory_controls(98000, 300, 100))["behaves"]
    assert not check.memory_control({})["behaves"]


def test_large_streaming_needs_the_reset_peak() -> None:
    def verdicts(reset: str | None, end: int) -> Json:
        peak = {"reset_error": reset, "start": {"VmHWM": 20000}, "end": {"VmHWM": end}}
        agent = {"peak": peak, "emitted_bytes": check.LARGE_BYTES, "maxrss_kib": [9, 9]}
        return {check.LARGE_CASE: {c: {"seconds": 1.0, "agent": agent} for c in check.CLIENTS}}

    assert check.large_streaming(verdicts(None, 22000))["behaves"]
    assert not check.large_streaming(verdicts(None, 20000 + 17 * 1024))["behaves"]
    assert not check.large_streaming(verdicts("PermissionError()", 22000))["behaves"]


def test_check_of_an_empty_directory_is_incomplete(tmp_path: Path) -> None:
    result = check.check(tmp_path)
    assert not result["complete"]
    assert len(result["incomplete"]) == len(check.CASES) * len(check.CLIENTS)
