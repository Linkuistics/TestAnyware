"""Controls for the k7 download controller and checker. No agent and no VM is needed."""

from __future__ import annotations

import json
import socket
import subprocess
import sys
import threading
import time
from pathlib import Path
from typing import Any

import download_k7
import download_k7_check as check


def serve_once(response: bytes) -> int:
    """Answer one connection with fixed bytes, then close it."""
    server = socket.socket()
    server.bind(("127.0.0.1", 0))
    server.listen(1)
    port = int(server.getsockname()[1])

    def run() -> None:
        conn, _ = server.accept()
        with conn:
            conn.recv(65536)
            conn.sendall(response)
        server.close()

    threading.Thread(target=run, daemon=True).start()
    return port


def head(length: int) -> bytes:
    return f"HTTP/1.1 200 OK\r\nContent-Length: {length}\r\n\r\n".encode()


def test_raw_client_reports_a_short_body() -> None:
    port = serve_once(head(10) + b"abc")
    result, body = download_k7.raw_download(port, Path("/x"), timeout=5)
    assert (result["end"], result["content_length"], body) == ("eof-before-length", 10, b"abc")


def test_raw_client_reports_extra_bytes() -> None:
    port = serve_once(head(3) + b"abcdef")
    result, body = download_k7.raw_download(port, Path("/x"), timeout=5)
    assert result["end"] == "complete-extra-bytes"
    assert body == b"abcdef"


def test_raw_client_reports_an_exact_body() -> None:
    port = serve_once(head(3) + b"abc")
    result, body = download_k7.raw_download(port, Path("/x"), timeout=5)
    assert (result["end"], body) == ("complete", b"abc")


def test_matches_rejects_prefixes_and_suffixes() -> None:
    assert check.matches(b"abc", [b"abc"])
    assert not check.matches(b"ab", [b"abc"])
    assert not check.matches(b"abc\n", [b"abc"])


def test_oracle_controls_behave(tmp_path: Path) -> None:
    result = check.oracle_controls(tmp_path)
    assert result["behaves"] is True
    assert result["exact"]["sha256"] != result["flipped"]["sha256"]


def test_oracle_constants_agree() -> None:
    assert download_k7.ORACLE.hex() == check.ORACLE_HEX
    assert download_k7.STRESS_PUBLICATIONS == check.STRESS_PUBLICATIONS
    assert download_k7.HISTORICAL_SHA256 == check.HISTORICAL_SHA256


def test_pattern_versions_differ_in_every_chunk() -> None:
    a, b = download_k7.pattern(1, download_k7.BIG), download_k7.pattern(2, download_k7.BIG)
    for start in range(0, download_k7.BIG, download_k7.CHUNK):
        assert a[start : start + 32] != b[start : start + 32]


def raw_case(tmp_path: Path, body: bytes, end: str, allowed: list[bytes]) -> dict[str, Any]:
    for index, version in enumerate(allowed):
        (tmp_path / f"allowed-{index}.bin").write_bytes(version)
    (tmp_path / "received.bin").write_bytes(body)
    record = {"result": {"status": 200, "end": end, "sha256": check.sha256(body)}}
    return check.classify_raw(tmp_path, record, check.allowed_versions(tmp_path, "c"))


def test_raw_wrong_complete_body_is_undetected_wrong(tmp_path: Path) -> None:
    row = raw_case(tmp_path, b"new-prefix", "complete", [b"old", b"new-prefix-and-more"])
    assert row["class"] == "undetected-wrong"


def test_raw_short_body_is_detected(tmp_path: Path) -> None:
    assert raw_case(tmp_path, b"ol", "eof-before-length", [b"old"])["class"] == "detected-failure"


def test_raw_allowed_body_is_allowed(tmp_path: Path) -> None:
    assert raw_case(tmp_path, b"old", "complete", [b"old", b"new"])["class"] == "allowed-complete"


def test_raw_record_that_disagrees_with_bytes_is_unclassified(tmp_path: Path) -> None:
    (tmp_path / "received.bin").write_bytes(b"x")
    record = {"result": {"status": 200, "end": "complete", "sha256": "0" * 64}}
    assert check.classify_raw(tmp_path, record, [b"x"])["class"] == "unclassified"


def cli_case(tmp_path: Path, result: dict[str, Any], local: bytes | None) -> dict[str, Any]:
    (tmp_path / "local").mkdir()
    if local is not None:
        (tmp_path / "local" / "c.bin").write_bytes(local)
    return check.classify_cli(tmp_path, {"case": "c", "result": result}, [b"old"])


def test_cli_success_with_wrong_bytes_is_undetected_wrong(tmp_path: Path) -> None:
    row = cli_case(tmp_path, {"exit": 0, "stdout": json.dumps({"bytes": 2})}, b"ne")
    assert (row["class"], row["receipt_bytes"]) == ("undetected-wrong", 2)


def test_cli_failure_with_temporary_is_a_leak(tmp_path: Path) -> None:
    row = cli_case(tmp_path, {"exit": 1, "temporaries": [".c.bin.testanyware-1.0.tmp"]}, None)
    assert row["class"] == "detected-failure-leak"


def test_cli_clean_failure_is_detected(tmp_path: Path) -> None:
    assert cli_case(tmp_path, {"exit": 1, "temporaries": []}, None)["class"] == "detected-failure"


def test_interrupted_cli_with_temporary_is_a_leak(tmp_path: Path) -> None:
    record = {"case": "c", "cli_killed": {"exit": -15, "temporaries_after": [".t"]}}
    assert check.classify_cli(tmp_path, record, [])["class"] == "interrupt-leak"


def test_stress_summary_counts_unknown_complete_bodies() -> None:
    record = {
        "reads": 3,
        "swaps": 9,
        "versions": {"aa": "pending"},
        "outcomes": {
            "1": {"count": 1, "status": 200, "end": "complete", "sha256": "aa"},
            "2": {"count": 1, "status": 200, "end": "complete", "sha256": check.FAILURE_D_SHA256},
            "3": {"count": 1, "status": 200, "end": "eof-before-length", "sha256": "bb"},
        },
    }
    summary = check.stress_summary(record)
    assert summary["classes"] == {
        "allowed-complete": 1,
        "undetected-wrong": 1,
        "detected-failure": 1,
    }
    assert summary["failure_d_bytes"] == 1


def test_raw_client_bounds_a_trickling_peer() -> None:
    """A peer that sends one byte before each receive timeout cannot extend the download."""
    server = socket.socket()
    server.bind(("127.0.0.1", 0))
    server.listen(1)
    port = int(server.getsockname()[1])
    done = threading.Event()

    def run() -> None:
        conn, _ = server.accept()
        with conn:
            conn.recv(65536)
            conn.sendall(head(1000))
            while not done.wait(0.05):
                try:
                    conn.sendall(b"x")
                except OSError:
                    break
        server.close()

    threading.Thread(target=run, daemon=True).start()
    started = time.monotonic()
    result, _ = download_k7.raw_download(port, Path("/x"), timeout=0.5)
    done.set()
    assert result["end"] == "timeout-before-length"
    assert time.monotonic() - started < 2


def test_end_process_kills_a_child_that_ignores_term() -> None:
    child = subprocess.Popen(
        [
            sys.executable,
            "-c",
            "import signal, time; signal.signal(signal.SIGTERM, "
            "signal.SIG_IGN); print('ready', flush=True); time.sleep(60)",
        ],
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    assert child.stdout is not None
    child.stdout.readline()
    ended, _, _ = download_k7.end_process(child)
    assert (ended, child.returncode) == ("SIGKILL", -9)
    assert download_k7.pid_alive(child.pid) is False


def test_mutable_stress_stops_its_writer_when_a_read_fails(tmp_path: Path) -> None:
    """A refused connection ends the loop. The writer stops and the record keeps the error."""
    closed = socket.socket()
    closed.bind(("127.0.0.1", 0))
    port = int(closed.getsockname()[1])
    closed.close()
    agent = download_k7.Agent("t", Path("/x"), Path("/x"), tmp_path, tmp_path / "pids")
    agent.target.mkdir()
    agent.port = port
    record = download_k7.stress_mutable(agent, 5)
    assert "error" in record and record["writer_alive"] is False
    assert not check.stress_summary(record)["complete"]


def immutable(publications: int, complete: int, mismatches: int = 0) -> dict[str, Any]:
    outcomes = {"complete": complete} | ({"mismatch": mismatches} if mismatches else {})
    return {"publications": publications, "outcomes": outcomes, "mismatches": [{}] * mismatches}


def test_immutable_stress_needs_every_planned_read() -> None:
    assert check.stress_summary(immutable(200, 200))["complete"]
    assert check.stress_summary(immutable(200, 199, 1))["complete"]
    assert not check.stress_summary(immutable(200, 0))["complete"]
    assert not check.stress_summary(immutable(0, 0))["complete"]
    assert not check.stress_summary(immutable(200, 150) | {"error": "x"})["complete"]


def mutable(reads: int, outcomes: dict[str, Any]) -> dict[str, Any]:
    return {"reads": reads, "swaps": 1, "versions": {"aa": "pending"}, "outcomes": outcomes}


def test_mutable_stress_needs_classified_reads_that_reconcile() -> None:
    good = {"1": {"count": 2, "status": 200, "end": "complete", "sha256": "aa"}}
    odd = {"1": {"count": 2, "status": 200, "end": "eof", "sha256": "aa"}}
    assert check.stress_summary(mutable(2, good))["complete"]
    assert not check.stress_summary(mutable(0, {}))["complete"]
    assert not check.stress_summary(mutable(3, good))["complete"]
    assert not check.stress_summary(mutable(2, odd))["complete"]
    assert not check.stress_summary(mutable(2, good) | {"writer_alive": True})["complete"]


def test_immutable_binary_is_judged_by_the_checker_oracle(tmp_path: Path) -> None:
    """A wrong allowed file from the controller cannot make wrong bytes look allowed."""
    (tmp_path / "allowed-0.bin").write_bytes(b"wrong")
    (tmp_path / "received.bin").write_bytes(b"wrong")
    record = {"result": {"status": 200, "end": "complete", "sha256": check.sha256(b"wrong")}}
    allowed = check.allowed_versions(tmp_path, "immutable-binary")
    assert check.classify_raw(tmp_path, record, allowed)["class"] == "undetected-wrong"


def test_unreaped_cli_interrupt_is_unclassified(tmp_path: Path) -> None:
    killed: dict[str, Any] = {"ended_by": "unreaped", "temporaries_after": []}
    record: dict[str, Any] = {"case": "c", "cli_killed": killed}
    assert check.classify_cli(tmp_path, record, [])["class"] == "unclassified"


def test_executable_digests_must_match_after_the_probe() -> None:
    meta: dict[str, Any] = {
        "cli": {"sha256": "c"},
        "shim": {"sha256": "s"},
        "agents": {"a": {"sha256": "x"}},
    }
    assert check.executables_unchanged(meta) is None
    after = {"cli_sha256": "c", "shim_sha256": "s", "agents_sha256": {"a": "x"}}
    assert check.executables_unchanged(meta | {"after": after}) is True
    changed = after | {"agents_sha256": {"a": "y"}}
    assert check.executables_unchanged(meta | {"after": changed}) is False
